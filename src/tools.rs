//! Offline tools over the gate's JSONL logs.
//!   gui-gate train LOGS... -o weights.json [--holdout 0.3] [--seed 0] [--l2 0.05]
//!       fit P(direct succeeds) from executed-direct actions, Platt-calibrate it, report held-out metrics
//!       (split by session, so a model is never scored on the run it was trained on);
//!   gui-gate compare LOGS... [--json]
//!       summarise logs per policy (baseline comparisons).
use crate::decide::{prior_weights, sigmoid};
use crate::features::FEATURE_NAMES;
use crate::util::*;
use indexmap::IndexMap;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

fn files_of(paths: &[String]) -> Vec<String> {
    let mut out = vec![];
    for p in paths {
        if std::path::Path::new(p).is_dir() {
            let mut v: Vec<String> = glob::glob(&format!("{p}/*.jsonl")).map(|g| g.flatten().map(|x| x.to_string_lossy().to_string()).collect())
                .unwrap_or_default();
            v.retain(|f| !f.ends_with(".fixture.jsonl"));
            v.sort();
            out.extend(v);
        } else {
            out.push(p.clone());
        }
    }
    out
}

fn events(path: &str) -> Vec<Value> {
    std::fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

type Row = (String, HashMap<String, f64>, f64);

/// Environment outcomes from a fixture log are training labels (never features).
fn outcomes_from_fixture(evs: &[Value], fixture: &str) -> HashMap<i64, bool> {
    let fx: Vec<Value> = events(fixture).into_iter().filter(|e| matches!(s(e, "ev"), "click" | "ignored")).collect();
    let mut out = HashMap::new();
    let mut prev = 0.0;
    for e in evs {
        if s(e, "ev") != "action" || !truthy(e.get("executed")) {
            continue;
        }
        let ts = e["ts"].as_f64().unwrap_or(0.0);
        let hits: Vec<&Value> = fx.iter().filter(|f| { let t = f["t"].as_f64().unwrap_or(0.0); prev < t && t <= ts + 0.05 }).collect();
        prev = ts + 0.05;
        if !matches!(s(e, "tool"), "click" | "double_click") {
            continue;
        }
        let want = e.get("target").and_then(|t| t.get("label")).cloned();
        let ok = hits.iter().any(|f| s(f, "ev") == "click" && f.get("caption") == want.as_ref());
        if let Some(aid) = as_i64(e.get("aid")) {
            out.insert(aid, ok);
        }
    }
    out
}

fn load(paths: &[String]) -> Vec<Row> {
    let mut rows = vec![];
    let mut retro: HashSet<(String, i64)> = HashSet::new();
    let mut env: HashMap<(String, i64), bool> = HashMap::new();
    for f in files_of(paths) {
        let evs = events(&f);
        let fxp = format!("{}.fixture.jsonl", f.trim_end_matches(".jsonl"));
        if std::path::Path::new(&fxp).exists() {
            let sess = evs.first().map(|e| s(e, "session").to_string()).unwrap_or_default();
            for (aid, ok) in outcomes_from_fixture(&evs, &fxp) {
                env.insert((sess.clone(), aid), ok);
            }
        }
        // session history as the live gate saw it before each action (logs older than v2 lack these features)
        let mut prev: Vec<(String, bool, bool)> = vec![];
        let mut last: Option<(String, f64, bool)> = None;
        for mut e in evs {
            if s(&e, "ev") == "retro_repeat" {
                if let Some(a) = as_i64(e.get("aid")) {
                    retro.insert((s(&e, "session").to_string(), a));
                }
            } else if s(&e, "ev") == "action" && truthy(e.get("executed")) {
                let sig = s(&e, "sig").to_string();
                if e.get("features").and_then(|f| f.get("sess_fail")).is_none() {
                    let role = e.get("target").map(|t| s(t, "role").to_string()).unwrap_or_default();
                    if let Some(fm) = e.get_mut("features").and_then(|f| f.as_object_mut()) {
                        for (k, v) in crate::features::history_feats(&prev, &sig).into_iter().chain(crate::features::role_feats(&role)) {
                            fm.insert(k.to_string(), json!(v));
                        }
                    }
                }
                let ts = e["ts"].as_f64().unwrap_or(0.0);
                let err = truthy(e.get("is_error"));
                // the live gate flags the previous action as a repeat-failure when this one repeats it
                if let (Some((lsig, lts, lerr)), Some(p)) = (&last, prev.last_mut()) {
                    if *lsig == sig && !*lerr && ts - *lts < 60.0 {
                        p.1 = true;
                    }
                }
                let none = s(&e, "effect") == "none";
                prev.push((sig.clone(), err || truthy(e.get("stale_error")) || none, none));
                last = Some((sig, ts, err));
                if s(&e, "decision") == "direct" {
                    rows.push(e);
                }
            }
        }
    }
    rows.into_iter().map(|e| {
        let key = (s(&e, "session").to_string(), as_i64(e.get("aid")).unwrap_or(-1));
        let ok = match env.get(&key) {
            Some(v) => *v && !truthy(e.get("is_error")),
            None => !truthy(e.get("is_error")) && !truthy(e.get("stale_error")) && !(retro.contains(&key) && !crate::decide::nav_signature(s(&e, "sig"))),
        };
        let mut f: HashMap<String, f64> = e.get("features").and_then(|f| f.as_object())
            .map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_f64().unwrap_or(0.0))).collect()).unwrap_or_default();
        if let Some(sc) = e.get("shadow_check").and_then(|v| v.as_bool()) {
            if f.get("check_pass").copied().unwrap_or(0.0) == 0.0 && f.get("check_fail").copied().unwrap_or(0.0) == 0.0 {
                f.insert("check_pass".into(), if sc { 1.0 } else { 0.0 });
                f.insert("check_fail".into(), if sc { 0.0 } else { 1.0 });
            }
        }
        (key.0, f, if ok { 1.0 } else { 0.0 })
    }).collect()
}

fn logit(w: &IndexMap<String, f64>, f: &HashMap<String, f64>) -> f64 {
    FEATURE_NAMES.iter().map(|k| w.get(*k).copied().unwrap_or(0.0) * f.get(*k).copied().unwrap_or(0.0)).sum()
}

fn fit(data: &[&Row], l2: f64, init: &IndexMap<String, f64>) -> IndexMap<String, f64> {
    let mut w: IndexMap<String, f64> = FEATURE_NAMES.iter().map(|k| (k.to_string(), init.get(*k).copied().unwrap_or(0.0))).collect();
    let n = data.len().max(1) as f64;
    for _ in 0..3000 {
        let mut g: HashMap<&str, f64> = HashMap::new();
        for (_, f, y) in data {
            let p = sigmoid(logit(&w, f));
            for k in FEATURE_NAMES {
                let v = f.get(*k).copied().unwrap_or(0.0);
                if v != 0.0 {
                    *g.entry(k).or_default() += (p - y) * v;
                }
            }
        }
        for k in FEATURE_NAMES {
            let reg = if *k == "bias" { 0.0 } else { l2 * (w[*k] - init.get(*k).copied().unwrap_or(0.0)) };
            w[*k] -= 0.5 * (g.get(k).copied().unwrap_or(0.0) / n + reg);
        }
    }
    w
}

fn platt(zs: &[f64], ys: &[f64]) -> (f64, f64) {
    let (mut a, mut b) = (1.0, 0.0);
    let n = zs.len().max(1) as f64;
    for _ in 0..500 {
        let (mut ga, mut gb) = (0.0, 0.0);
        for (z, y) in zs.iter().zip(ys) {
            let p = sigmoid(a * z + b);
            ga += (p - y) * z;
            gb += p - y;
        }
        a -= 0.05 * ga / n;
        b -= 0.05 * gb / n;
    }
    (a, b)
}

fn metrics(ps: &[f64], ys: &[f64]) -> Value {
    let n = ys.len();
    if n == 0 {
        return json!({});
    }
    let nf = n as f64;
    let brier = ps.iter().zip(ys).map(|(p, y)| (p - y).powi(2)).sum::<f64>() / nf;
    let nll = -ps.iter().zip(ys).map(|(p, y)| y * p.max(1e-6).ln() + (1.0 - y) * (1.0 - p).max(1e-6).ln()).sum::<f64>() / nf;
    let (mut ece, mut table) = (0.0, vec![]);
    for i in 0..10 {
        let (lo, hi) = (i as f64 / 10.0, (i + 1) as f64 / 10.0);
        let idx: Vec<usize> = ps.iter().enumerate().filter(|(_, p)| (lo <= **p && **p < hi) || (i == 9 && **p == 1.0)).map(|(j, _)| j).collect();
        if idx.is_empty() {
            continue;
        }
        let conf = idx.iter().map(|j| ps[*j]).sum::<f64>() / idx.len() as f64;
        let acc = idx.iter().map(|j| ys[*j]).sum::<f64>() / idx.len() as f64;
        ece += idx.len() as f64 / nf * (conf - acc).abs();
        table.push(json!({"bin": format!("{lo:.1}-{hi:.1}"), "n": idx.len(), "mean_p": round_to(conf, 3), "success": round_to(acc, 3)}));
    }
    let pos: Vec<f64> = ps.iter().zip(ys).filter(|(_, y)| **y == 1.0).map(|(p, _)| *p).collect();
    let neg: Vec<f64> = ps.iter().zip(ys).filter(|(_, y)| **y == 0.0).map(|(p, _)| *p).collect();
    let auc = if !pos.is_empty() && !neg.is_empty() {
        let s: f64 = pos.iter().map(|a| neg.iter().map(|b| if a > b { 1.0 } else if a == b { 0.5 } else { 0.0 }).sum::<f64>()).sum();
        json!(round_to(s / (pos.len() * neg.len()) as f64, 3))
    } else {
        Value::Null
    };
    json!({"n": n, "base_rate": round_to(ys.iter().sum::<f64>() / nf, 3), "brier": round_to(brier, 4), "nll": round_to(nll, 4),
           "ece": round_to(ece, 4), "auc": auc, "reliability": table})
}

/// Small deterministic PRNG (splitmix64) for the session shuffle.
fn shuffle<T>(v: &mut [T], seed: u64) {
    let mut x = seed.wrapping_add(0x9E3779B97F4A7C15);
    for i in (1..v.len()).rev() {
        x = x.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^= z >> 31;
        v.swap(i, (z % (i as u64 + 1)) as usize);
    }
}

pub fn train_main(argv: &[String]) -> i32 {
    let (mut paths, mut out, mut holdout, mut seed, mut l2) = (vec![], "weights.json".to_string(), 0.3, 0u64, 0.05);
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "-o" | "--out" => { out = argv.get(i + 1).cloned().unwrap_or(out); i += 1; }
            "--holdout" => { holdout = argv.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(holdout); i += 1; }
            "--seed" => { seed = argv.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(seed); i += 1; }
            "--l2" => { l2 = argv.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(l2); i += 1; }
            "-h" | "--help" => { println!("usage: gui-gate train LOGS... [-o weights.json] [--holdout 0.3] [--seed 0] [--l2 0.05]"); return 0; }
            p => paths.push(p.to_string()),
        }
        i += 1;
    }
    let data = load(&paths);
    let mut sessions: Vec<String> = data.iter().map(|d| d.0.clone()).collect::<HashSet<_>>().into_iter().collect();
    sessions.sort();
    if data.len() < 20 || sessions.len() < 2 {
        println!("{}", json!({"error": "need at least 20 direct-executed actions from 2+ sessions", "rows": data.len(), "sessions": sessions.len()}));
        return 1;
    }
    shuffle(&mut sessions, seed);
    let k = ((sessions.len() as f64 * holdout).round() as usize).max(1);
    let test_s: HashSet<&String> = sessions[..k].iter().collect();
    let train: Vec<&Row> = data.iter().filter(|d| !test_s.contains(&d.0)).collect();
    let test: Vec<&Row> = data.iter().filter(|d| test_s.contains(&d.0)).collect();
    let mut tr_s: Vec<&String> = train.iter().map(|d| &d.0).collect::<HashSet<_>>().into_iter().collect();
    tr_s.sort();
    let cal_s: HashSet<&String> = if tr_s.len() > 2 { tr_s[..(tr_s.len() / 4).max(1)].iter().cloned().collect() } else { HashSet::new() };
    let mut fit_rows: Vec<&Row> = train.iter().filter(|d| !cal_s.contains(&d.0)).cloned().collect();
    if fit_rows.is_empty() { fit_rows = train.clone(); }
    let mut cal_rows: Vec<&Row> = train.iter().filter(|d| cal_s.contains(&d.0)).cloned().collect();
    if cal_rows.is_empty() { cal_rows = train.clone(); }
    let prior = prior_weights();
    let w = fit(&fit_rows, l2, &prior);
    let (a, b) = platt(&cal_rows.iter().map(|d| logit(&w, &d.1)).collect::<Vec<_>>(), &cal_rows.iter().map(|d| d.2).collect::<Vec<_>>());
    let ys: Vec<f64> = test.iter().map(|d| d.2).collect();
    let res = json!({
        "rows": data.len(), "sessions": sessions.len(), "test_sessions": test_s.len(),
        "prior_on_test": metrics(&test.iter().map(|d| sigmoid(logit(&prior, &d.1))).collect::<Vec<_>>(), &ys),
        "fitted_on_test": metrics(&test.iter().map(|d| sigmoid(logit(&w, &d.1))).collect::<Vec<_>>(), &ys),
        "fitted_calibrated_on_test": metrics(&test.iter().map(|d| sigmoid(a * logit(&w, &d.1) + b)).collect::<Vec<_>>(), &ys),
    });
    let name = crate::sidefx::basename(&out);
    let name = name.rsplit_once('.').map(|x| x.0.to_string()).unwrap_or(name);
    let wj: serde_json::Map<String, Value> = w.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
    let doc = json!({"name": name, "weights": wj, "calib": {"a": a, "b": b}, "eval": res});
    if let Err(e) = std::fs::write(&out, serde_json::to_string_pretty(&doc).unwrap()) {
        eprintln!("cannot write {out}: {e}");
        return 1;
    }
    println!("{}", serde_json::to_string_pretty(&res).unwrap());
    0
}

pub fn compare_main(argv: &[String]) -> i32 {
    let as_json = argv.iter().any(|a| a == "--json");
    let paths: Vec<String> = argv.iter().filter(|a| *a != "--json").cloned().collect();
    let mut evs = vec![];
    let mut sess_policy: IndexMap<String, String> = IndexMap::new();
    let mut retro: HashSet<(String, i64)> = HashSet::new();
    for f in files_of(&paths) {
        for e in events(&f) {
            if s(&e, "ev") == "action" {
                sess_policy.entry(s(&e, "session").to_string()).or_insert_with(|| s(&e, "policy").to_string());
            }
            if s(&e, "ev") == "retro_repeat" {
                retro.insert((s(&e, "session").to_string(), as_i64(e.get("aid")).unwrap_or(-1)));
            }
            evs.push(e);
        }
    }
    let mut agg: IndexMap<String, IndexMap<String, f64>> = IndexMap::new();
    for e in &evs {
        let pol = sess_policy.get(s(e, "session")).cloned().unwrap_or_else(|| "?".into());
        let g = agg.entry(pol).or_default();
        let mut add = |k: &str, v: f64| *g.entry(k.to_string()).or_default() += v;
        match s(e, "ev") {
            "obs" => {
                add("observations", 1.0);
                add("screenshots", if truthy(e.get("screenshot")) { 1.0 } else { 0.0 });
                add("image_tokens", as_f64(e.get("px")).unwrap_or(0.0) / 750.0);
                add("driver_obs_ms", as_f64(e.get("ms")).unwrap_or(0.0));
            }
            "action" => {
                add("actions", 1.0);
                add("gate_ms", as_f64(e.get("gate_ms")).unwrap_or(0.0));
                if truthy(e.get("executed")) {
                    add("executed", 1.0);
                    add("exec_ms", as_f64(e.get("exec_ms")).unwrap_or(0.0));
                    let key = (s(e, "session").to_string(), as_i64(e.get("aid")).unwrap_or(-1));
                    let bad = truthy(e.get("is_error")) || truthy(e.get("stale_error")) || (retro.contains(&key) && !crate::decide::nav_signature(s(e, "sig")));
                    add("failed", if bad { 1.0 } else { 0.0 });
                } else {
                    add("interventions", 1.0);
                    let d = s(e, "decision");
                    add(&format!("int_{}", if d.is_empty() { "?" } else { d }), 1.0);
                }
            }
            _ => {}
        }
    }
    let mut out = serde_json::Map::new();
    for (pol, g) in &agg {
        let mut d: serde_json::Map<String, Value> = g.iter().map(|(k, v)| (k.clone(), json!(round_to(*v, 1)))).collect();
        d.insert("sessions".into(), json!(sess_policy.values().filter(|p| *p == pol).count()));
        let gv = |k: &str| g.get(k).copied().unwrap_or(0.0);
        if gv("executed") > 0.0 {
            d.insert("fail_rate_executed".into(), json!(round_to(gv("failed") / gv("executed"), 3)));
        }
        if gv("actions") > 0.0 {
            d.insert("gate_ms_per_action".into(), json!(round_to(gv("gate_ms") / gv("actions"), 1)));
        }
        out.insert(pol.clone(), Value::Object(d));
    }
    if as_json {
        println!("{}", serde_json::to_string_pretty(&Value::Object(out)).unwrap());
        return 0;
    }
    for (pol, d) in &out {
        println!("== policy {pol}");
        let mut keys: Vec<&String> = d.as_object().unwrap().keys().collect();
        keys.sort();
        for k in keys {
            println!("   {k}: {}", d[k]);
        }
    }
    0
}

/// gui-gate replay LOGS... [--weights FILE|prior_v1] [--human deny|auto] [--reviewer] [--lambda X] [--fail-exp E]
/// Re-decide every executed action in old logs with the current formula and predictor (features rebuilt the way
/// the live gate computes them) and score the decisions against what actually happened to those actions.
pub fn replay_main(argv: &[String]) -> i32 {
    let mut cfg = crate::decide::GateConfig::default();
    cfg.human_channel = "deny".into();
    let mut paths = vec![];
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--weights" => {
                let w = argv.get(i + 1).cloned().unwrap_or_default();
                if w == "prior_v1" { cfg.weights = crate::decide::prior_v1_weights(); cfg.calib = None; cfg.model_name = w; }
                else if let Err(e) = cfg.load_weights(&w) { eprintln!("{e}"); return 1; }
                i += 1;
            }
            "--human" => { cfg.human_channel = argv.get(i + 1).cloned().unwrap_or_default(); i += 1; }
            "--lambda" => { cfg.lam = argv.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(1.0); i += 1; }
            "--fail-exp" => { cfg.fail_exp = argv.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(1.0); i += 1; }
            "--reviewer" => cfg.reviewer = true,
            "-h" | "--help" => { println!("usage: gui-gate replay LOGS... [--weights FILE|prior_v1] [--human deny|auto] [--reviewer] [--lambda X] [--fail-exp E]"); return 0; }
            p => paths.push(p.to_string()),
        }
        i += 1;
    }
    let mut by: IndexMap<String, (f64, f64)> = IndexMap::new(); // option -> (n, n that actually failed)
    let (mut n, mut fails): (f64, f64) = (0.0, 0.0);
    for f in files_of(&paths) {
        let evs = events(&f);
        let retro: HashSet<i64> = evs.iter().filter(|e| s(e, "ev") == "retro_repeat").filter_map(|e| as_i64(e.get("aid"))).collect();
        let mut prev: Vec<(String, bool, bool)> = vec![];
        let mut last: Option<(String, f64, bool)> = None;
        for e in evs.iter().filter(|e| s(e, "ev") == "action" && truthy(e.get("executed"))) {
            let sig = s(e, "sig").to_string();
            let tg = e.get("target").cloned().unwrap_or(json!({}));
            let mut feats: crate::features::Feats = FEATURE_NAMES.iter().map(|k| (*k, as_f64(e.get("features").and_then(|f| f.get(*k))).unwrap_or(0.0))).collect();
            for (k, v) in crate::features::history_feats(&prev, &sig).into_iter().chain(crate::features::role_feats(s(&tg, "role"))) {
                feats[k] = v;
            }
            let keys = sig.rsplit('|').next().unwrap_or("").to_string();
            let ctx = crate::features::ActionCtx {
                tool: s(e, "tool").to_string(), args: json!({"keys": keys}),
                target: crate::features::Target { kind: s(&tg, "kind").to_string(), role: s(&tg, "role").to_string(), label: s(&tg, "label").to_string(), ..Default::default() },
                signature: sig.clone(), risk: None, steps: vec![], risk_level: e.get("risk_level").and_then(|v| v.as_str()).map(String::from), handoff: None,
            };
            let ca = matches!(ctx.target.kind.as_str(), "token" | "index" | "name");
            let d = crate::decide::hard_rules(&ctx, &feats, &cfg, true, true).unwrap_or_else(|| crate::decide::choose(&ctx, &feats, &cfg, ca, false));
            let aid = as_i64(e.get("aid")).unwrap_or(-1);
            // pressing the same arrow/Tab key again is how lists are walked, not a sign the first press failed
            let failed = truthy(e.get("is_error")) || truthy(e.get("stale_error")) || (retro.contains(&aid) && !crate::decide::nav_signature(&sig));
            n += 1.0;
            if failed { fails += 1.0; }
            let ent = by.entry(d.option.clone()).or_default();
            ent.0 += 1.0;
            if failed { ent.1 += 1.0; }
            // update history exactly as the live gate would
            let ts = e["ts"].as_f64().unwrap_or(0.0);
            let err = truthy(e.get("is_error"));
            if let (Some((lsig, lts, lerr)), Some(p)) = (&last, prev.last_mut()) {
                if *lsig == sig && !*lerr && ts - *lts < 60.0 { p.1 = true; }
            }
            let none = s(e, "effect") == "none";
            prev.push((sig.clone(), err || truthy(e.get("stale_error")) || none, none));
            last = Some((sig, ts, err));
        }
    }
    let inter: f64 = by.iter().filter(|(k, _)| k.as_str() != "direct").map(|(_, v)| v.0).sum();
    let caught: f64 = by.iter().filter(|(k, _)| k.as_str() != "direct").map(|(_, v)| v.1).sum();
    let internal: f64 = by.iter().filter(|(k, _)| matches!(k.as_str(), "check" | "heal" | "wait")).map(|(_, v)| v.0).sum();
    let opts: serde_json::Map<String, Value> = by.iter().map(|(k, v)| (k.clone(), json!({"n": v.0, "actually_failed": v.1}))).collect();
    println!("{}", serde_json::to_string_pretty(&json!({
        "model": cfg.model_name, "human": cfg.human_channel, "reviewer": cfg.reviewer, "lambda": cfg.lam, "actions": n, "failed": fails,
        "intervene_rate": round_to(inter / n.max(1.0), 3), "gate_side_rate": round_to(internal / n.max(1.0), 3),
        "failures_caught": round_to(caught / fails.max(1.0), 3), "precision": round_to(caught / inter.max(1.0), 3),
        "by_option": opts})).unwrap());
    0
}
