//! Offline training and evaluation for the value model (value.rs), with label hygiene.
//!
//!   gui-gate train --model treatment LOGS... -o value.json [--split run+family|run|family|task|session]
//!       [--holdout 0.3] [--seed 0] [--l2 0.01] [--l2-main 0.01] [--l2-int 0.05] [--boot 30] [--level 0.9] [--min-rows 30]
//!       [--label step|task --task-labels runs_scored.jsonl] [--families map.json] [--prior zero|v1|builtin]
//!       [--report] [--md report.md] [--fit-all] [--test LOGS...]
//!   gui-gate train --report LOGS...        (the direct predictor, same metrics; see tools::train_main)
//!   gui-gate costs LOGS... -o costs.json   measured option costs for GUI_GATE_COSTS
//!
//! Label hygiene (what the logs can and cannot say):
//! * Rows are labelled with the option ACTUALLY TAKEN. An executed step whose trail shows a gate-side wait / check /
//!   heal / reviewer allow is that option's row, not a "direct" row (0.9.x mixed them into the direct model).
//! * A step the gate handed back (look_light, look_full, reason, review-deny, ask_help) is labelled with the outcome
//!   of the next executed action of the same session (within 180 s): the step after the treatment. 0.9.x had no
//!   label at all for these states, so the direct model only saw states the gate let through.
//! * States the gate blocked still have NO outcome under direct (and executed states none under the options not
//!   taken). That gap is not fixable from logs: options were chosen by the logged model (confounded by p_direct),
//!   so q_o is an observational estimate until bench/counterfactual/ branches the same checkpoint under every option.
//! * Safety outcomes (ask_human, handoff, deny, risk_review, stop_loop) and every risky step are excluded: the value
//!   model is about safe states only.
//! * The regularisation centre defaults to zero (treatment) / the hand-set v1 prior (direct predictor), never the
//!   built-in osworld_v3 weights, which were fitted on these same logs (`--prior builtin` is allowed and the report
//!   then says prior_leak_risk=true).
//! * Held-out evaluation keeps runs AND task families apart (default --split run+family): a test row's run and task
//!   family are both absent from training; rows that cross are discarded and counted.
use crate::decide::sigmoid;
use crate::features::FEATURE_NAMES;
use crate::util::*;
use crate::value::{Coefs, TreatmentModel, VALUE_OPTIONS};
use indexmap::IndexMap;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

const NF: usize = 31;

fn fidx(k: &str) -> Option<usize> {
    FEATURE_NAMES.iter().position(|x| *x == k)
}

#[derive(Clone, Debug)]
pub struct TRow {
    pub session: String,
    pub run: String,
    pub task: String,
    pub family: String,
    pub f: Vec<f64>,
    pub option: String,
    pub y: f64,
    pub tier: String,
    pub check: Option<bool>,
}

#[derive(Clone, Debug, Default)]
pub struct LoadStats {
    pub files: usize,
    pub executed_rows: usize,
    pub intervention_rows: usize,
    pub interventions_unlabelled: usize,
    pub safety_excluded: usize,
    pub risky_excluded: usize,
    pub env_labels: usize,
    pub files_without_task_label: usize,
    pub by_option: IndexMap<String, usize>,
    pub by_tier: IndexMap<String, usize>,
}

impl LoadStats {
    fn json(&self) -> Value {
        json!({"files": self.files, "executed_rows": self.executed_rows, "intervention_rows": self.intervention_rows,
               "interventions_unlabelled_dropped": self.interventions_unlabelled, "safety_outcomes_excluded": self.safety_excluded,
               "risky_steps_excluded": self.risky_excluded, "environment_labels": self.env_labels,
               "files_without_task_label": self.files_without_task_label,
               "by_option": Value::Object(self.by_option.iter().map(|(k, n)| (k.clone(), json!(n))).collect()),
               "by_tier": Value::Object(self.by_tier.iter().map(|(k, n)| (k.clone(), json!(n))).collect())})
    }
}

/// (run, task, family) of a gate log from its path: results/<run>/gate/<task>-<cond>-s<seed>[-try<n>].jsonl
pub fn file_groups(path: &str, families: &HashMap<String, String>) -> (String, String, String) {
    let p = std::path::Path::new(path);
    let stem = p.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    let parent = p.parent();
    let pname = parent.and_then(|d| d.file_name()).map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    let run = if pname == "gate" {
        parent.and_then(|d| d.parent()).and_then(|d| d.file_name()).map(|x| x.to_string_lossy().to_string()).unwrap_or(pname)
    } else {
        pname
    };
    let re = regex::Regex::new(r"^(.*)-[A-Za-z0-9_]+-s\d+(-try\d+)?$").unwrap();
    let task = re.captures(&stem).map(|c| c[1].to_string()).unwrap_or_else(|| stem.clone());
    let family = families.get(&task).cloned().or_else(|| families.iter().find(|(k, _)| task.starts_with(k.as_str())).map(|(_, v)| v.clone()))
        .unwrap_or_else(|| {
            let toks: Vec<&str> = task.split('-').collect();
            let ow = regex::Regex::new(r"^o[wg]\d+$").unwrap();
            if toks.len() > 1 && ow.is_match(toks[0]) { toks[1].to_string() } else { toks[0].to_string() }
        });
    (run, task, family)
}

/// The option the value module actually took for one logged action (None: a safety outcome or not a value option).
pub fn option_taken(e: &Value) -> Option<String> {
    let dec = s(e, "decision");
    let trail: Vec<String> = e.get("trail").and_then(|t| t.as_array()).map(|a| a.iter().map(|x| pystr(Some(x))).collect()).unwrap_or_default();
    let has = |k: &str| trail.iter().any(|t| t == k);
    // a person who did not answer (timeout, cancel, closed dialog) is "no answer", never a label that the user was
    // unwilling: such steps are not training rows for any option
    let ho = s(e, "human_outcome");
    if crate::server::NO_ANSWER_OUTCOMES.contains(&ho) || ho == "not_asked_again" {
        return None;
    }
    if truthy(e.get("executed")) {
        if dec != "direct" || s(e, "tier") == "human" {
            return None;
        }
        return Some(if has("healed") { "heal" } else if has("wait") { "wait" } else if has("check") { "check" }
                    else if has("reviewer_allow") || s(e, "tier") == "review" { "review" } else { "direct" }.into());
    }
    if has("reviewer_deny") || (s(e, "tier") == "review" && matches!(dec, "look_light" | "deny")) {
        return Some("review".into());
    }
    match dec {
        "look_light" | "look_full" | "reason" | "ask_help" | "review" => Some(dec.into()),
        _ => None,
    }
}

fn feats_vec(e: &Value) -> Vec<f64> {
    let mut v = vec![0.0; NF];
    let fo = e.get("features");
    for (i, k) in FEATURE_NAMES.iter().enumerate() {
        v[i] = as_f64(fo.and_then(|f| f.get(*k))).unwrap_or(0.0);
    }
    v[0] = 1.0;
    if let Some(sc) = e.get("shadow_check").and_then(|x| x.as_bool()) {
        let (p, f) = (fidx("check_pass").unwrap(), fidx("check_fail").unwrap());
        if v[p] == 0.0 && v[f] == 0.0 {
            v[p] = if sc { 1.0 } else { 0.0 };
            v[f] = if sc { 0.0 } else { 1.0 };
        }
    }
    v
}

pub fn read_task_labels(paths: &[String]) -> HashMap<String, bool> {
    let mut m = HashMap::new();
    for p in paths {
        for l in std::fs::read_to_string(p).unwrap_or_default().lines() {
            if let Ok(r) = serde_json::from_str::<Value>(l) {
                if truthy(r.get("infra")) {
                    continue;
                }
                if let Some(t) = r.get("tag").and_then(|t| t.as_str()) {
                    m.insert(t.to_string(), truthy(r.get("success")));
                }
            }
        }
    }
    m
}

/// label = "step" (the step's own outcome) or "task" (the run's task success from runs_scored.jsonl)
pub fn load_rows(paths: &[String], label: &str, task_labels: &HashMap<String, bool>, families: &HashMap<String, String>) -> (Vec<TRow>, LoadStats) {
    let mut rows: Vec<TRow> = vec![];
    let mut st = LoadStats::default();
    for file in crate::tools::files_of(paths) {
        let evs = crate::tools::events(&file);
        st.files += 1;
        let (run, task, family) = file_groups(&file, families);
        let stem = std::path::Path::new(&file).file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
        let task_y = task_labels.get(&stem).copied();
        if label == "task" && task_y.is_none() {
            st.files_without_task_label += 1;
            continue;
        }
        let fxp = format!("{}.fixture.jsonl", file.trim_end_matches(".jsonl"));
        let env: HashMap<i64, bool> = if std::path::Path::new(&fxp).exists() { crate::tools::outcomes_from_fixture(&evs, &fxp) } else { HashMap::new() };
        let retro: HashSet<i64> = evs.iter().filter(|e| s(e, "ev") == "retro_repeat").filter_map(|e| as_i64(e.get("aid"))).collect();
        let mut prev: Vec<(String, bool, bool)> = vec![];
        let mut last: Option<(String, f64, bool)> = None;
        let mut pending: Vec<(usize, f64)> = vec![];
        for e0 in evs.iter().filter(|e| s(e, "ev") == "action") {
            let mut e = e0.clone();
            let sess = s(&e, "session").to_string();
            let ts = e["ts"].as_f64().unwrap_or(0.0);
            let sig = s(&e, "sig").to_string();
            if e.get("features").and_then(|f| f.get("sess_fail")).is_none() {
                let role = e.get("target").map(|t| s(t, "role").to_string()).unwrap_or_default();
                if let Some(fm) = e.get_mut("features").and_then(|f| f.as_object_mut()) {
                    for (k, v) in crate::features::history_feats(&prev, &sig).into_iter().chain(crate::features::role_feats(&role)) {
                        fm.insert(k.to_string(), json!(v));
                    }
                }
            }
            let opt = option_taken(&e);
            let risky = !e.get("risk_level").map(|v| v.is_null()).unwrap_or(true) || !e.get("handoff").map(|v| v.is_null()).unwrap_or(true);
            let tier = s(&e, "tier").to_string();
            let check = e.get("check").and_then(|c| c.as_bool()).or_else(|| e.get("shadow_check").and_then(|c| c.as_bool()));
            if truthy(e.get("executed")) {
                let aid = as_i64(e.get("aid")).unwrap_or(-1);
                let step_ok = match env.get(&aid) {
                    Some(v) => { st.env_labels += 1; *v && !truthy(e.get("is_error")) }
                    None => !truthy(e.get("is_error")) && !truthy(e.get("stale_error")) && !(retro.contains(&aid) && !crate::decide::nav_signature(&sig)),
                };
                let y = if label == "task" { if task_y.unwrap_or(false) { 1.0 } else { 0.0 } } else if step_ok { 1.0 } else { 0.0 };
                for (i, t0) in pending.drain(..) {
                    if ts - t0 <= 180.0 {
                        rows[i].y = y;
                    }
                }
                match (opt, risky) {
                    (Some(o), false) => {
                        st.executed_rows += 1;
                        rows.push(TRow { session: sess, run: run.clone(), task: task.clone(), family: family.clone(), f: feats_vec(&e), option: o, y, tier, check });
                    }
                    (Some(_), true) => st.risky_excluded += 1,
                    (None, _) => st.safety_excluded += 1,
                }
                // session history exactly as the live gate keeps it
                let err = truthy(e.get("is_error"));
                if let (Some((lsig, lts, lerr)), Some(p)) = (&last, prev.last_mut()) {
                    if *lsig == sig && !*lerr && ts - *lts < 60.0 {
                        p.1 = true;
                    }
                }
                let none = s(&e, "effect") == "none";
                prev.push((sig.clone(), err || truthy(e.get("stale_error")) || none, none));
                last = Some((sig, ts, err));
            } else {
                match (opt, risky) {
                    (Some(o), false) => {
                        st.intervention_rows += 1;
                        pending.push((rows.len(), ts));
                        rows.push(TRow { session: sess, run: run.clone(), task: task.clone(), family: family.clone(), f: feats_vec(&e), option: o, y: f64::NAN, tier, check });
                    }
                    (Some(_), true) => st.risky_excluded += 1,
                    (None, _) => st.safety_excluded += 1,
                }
            }
        }
        st.interventions_unlabelled += pending.len();
    }
    let before = rows.len();
    rows.retain(|r| !r.y.is_nan());
    st.interventions_unlabelled = st.interventions_unlabelled.max(before - rows.len());
    st.intervention_rows -= before - rows.len();
    for r in &rows {
        *st.by_option.entry(r.option.clone()).or_default() += 1;
        *st.by_tier.entry(if r.tier.is_empty() { "?".into() } else { r.tier.clone() }).or_default() += 1;
    }
    (rows, st)
}

// ---------------------------------------------------------------- fitting

struct Dense {
    base: Vec<f64>,
    inter: Vec<Vec<f64>>, // per option index (VALUE_OPTIONS), direct row stays zero
}

fn opt_idx(o: &str) -> usize {
    VALUE_OPTIONS.iter().position(|x| *x == o).unwrap_or(0)
}

fn dense_logit(w: &Dense, r: &TRow) -> f64 {
    let oi = opt_idx(&r.option);
    let mut z = 0.0;
    for (k, x) in r.f.iter().enumerate() {
        if *x != 0.0 {
            z += w.base[k] * x;
            if oi != 0 {
                z += w.inter[oi][k] * x;
            }
        }
    }
    z
}

fn to_coefs(w: &Dense) -> Coefs {
    let base = FEATURE_NAMES.iter().enumerate().map(|(i, k)| (k.to_string(), w.base[i])).collect();
    let mut inter = IndexMap::new();
    for (oi, o) in VALUE_OPTIONS.iter().enumerate().skip(1) {
        if w.inter[oi].iter().any(|x| *x != 0.0) {
            inter.insert(o.to_string(), FEATURE_NAMES.iter().enumerate().map(|(i, k)| (k.to_string(), w.inter[oi][i])).collect());
        }
    }
    Coefs { base, inter }
}

fn from_coefs(c: &Coefs) -> Dense {
    let base = FEATURE_NAMES.iter().map(|k| c.base.get(*k).copied().unwrap_or(0.0)).collect();
    let inter = VALUE_OPTIONS.iter().map(|o| FEATURE_NAMES.iter().map(|k| c.inter.get(*o).and_then(|m| m.get(*k)).copied().unwrap_or(0.0)).collect()).collect();
    Dense { base, inter }
}

#[derive(Clone, Debug)]
pub struct FitCfg {
    pub l2: f64,
    pub l2_int: f64,
    pub l2_main: f64,
    pub iters: usize,
    pub lr: f64,
}

impl Default for FitCfg {
    fn default() -> Self {
        FitCfg { l2: 0.01, l2_int: 0.05, l2_main: 0.01, iters: 1500, lr: 0.5 }
    }
}

/// Penalised logistic regression with option one-hot × feature interactions:
///   loss = mean NLL + l2/2 |b − prior|² (bias free) + l2_main/2 Σ g_o[bias]² + l2_int/2 Σ g_o[k≠bias]².
/// Options present in `allowed` get interaction terms; the rest share the base (they are not chosen anyway).
pub fn fit_treatment(rows: &[&TRow], fc: &FitCfg, prior: &IndexMap<String, f64>, init: Option<&Coefs>, allowed: &HashSet<String>) -> Coefs {
    let center: Vec<f64> = FEATURE_NAMES.iter().map(|k| prior.get(*k).copied().unwrap_or(0.0)).collect();
    let mut w = match init { Some(c) => from_coefs(c), None => Dense { base: center.clone(), inter: vec![vec![0.0; NF]; VALUE_OPTIONS.len()] } };
    let n = rows.len().max(1) as f64;
    let on: Vec<bool> = VALUE_OPTIONS.iter().map(|o| *o != "direct" && allowed.contains(*o)).collect();
    for _ in 0..fc.iters {
        let mut gb = vec![0.0; NF];
        let mut gi = vec![vec![0.0; NF]; VALUE_OPTIONS.len()];
        for r in rows {
            let oi = opt_idx(&r.option);
            let d = sigmoid(dense_logit(&w, r)) - r.y;
            for (k, x) in r.f.iter().enumerate() {
                if *x != 0.0 {
                    gb[k] += d * x;
                    if on[oi] {
                        gi[oi][k] += d * x;
                    }
                }
            }
        }
        for k in 0..NF {
            let reg = if k == 0 { 0.0 } else { fc.l2 * (w.base[k] - center[k]) };
            w.base[k] -= fc.lr * (gb[k] / n + reg);
        }
        for oi in 1..VALUE_OPTIONS.len() {
            if !on[oi] {
                continue;
            }
            for k in 0..NF {
                let reg = if k == 0 { fc.l2_main } else { fc.l2_int } * w.inter[oi][k];
                w.inter[oi][k] -= fc.lr * (gi[oi][k] / n + reg);
            }
        }
    }
    to_coefs(&w)
}

/// P(check passes | x): base-only logistic on rows with a probe result (check features excluded).
fn fit_probe(rows: &[&TRow], fc: &FitCfg) -> Option<IndexMap<String, f64>> {
    let skip = [fidx("check_pass").unwrap(), fidx("check_fail").unwrap()];
    let data: Vec<(&Vec<f64>, f64)> = rows.iter().filter_map(|r| r.check.map(|c| (&r.f, if c { 1.0 } else { 0.0 }))).collect();
    if data.len() < 20 {
        return None;
    }
    let mut w = vec![0.0; NF];
    let n = data.len() as f64;
    for _ in 0..fc.iters {
        let mut g = vec![0.0; NF];
        for (f, y) in &data {
            let z: f64 = (0..NF).filter(|k| !skip.contains(k)).map(|k| w[k] * f[k]).sum();
            let d = sigmoid(z) - y;
            for k in 0..NF {
                if !skip.contains(&k) && f[k] != 0.0 {
                    g[k] += d * f[k];
                }
            }
        }
        for k in 0..NF {
            let reg = if k == 0 { 0.0 } else { fc.l2 * w[k] };
            w[k] -= fc.lr * (g[k] / n + reg);
        }
    }
    Some(FEATURE_NAMES.iter().enumerate().filter(|(k, _)| !skip.contains(k)).map(|(k, name)| (name.to_string(), w[k])).collect())
}

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(1))
    }
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Cluster bootstrap: resample groups (sessions) with replacement, return the row indices.
pub fn resample_groups(groups: &[String], rng: &mut Rng) -> Vec<usize> {
    let mut by: IndexMap<&str, Vec<usize>> = IndexMap::new();
    for (i, g) in groups.iter().enumerate() {
        by.entry(g.as_str()).or_default().push(i);
    }
    let keys: Vec<&str> = by.keys().cloned().collect();
    let mut out = vec![];
    for _ in 0..keys.len() {
        out.extend(by[keys[rng.below(keys.len())]].iter().cloned());
    }
    out
}

// ---------------------------------------------------------------- metrics

fn auc(ps: &[f64], ys: &[f64]) -> Option<f64> {
    let npos = ys.iter().filter(|y| **y == 1.0).count();
    let nneg = ys.len() - npos;
    if npos == 0 || nneg == 0 {
        return None;
    }
    let mut idx: Vec<usize> = (0..ps.len()).collect();
    idx.sort_by(|a, b| ps[*a].partial_cmp(&ps[*b]).unwrap_or(std::cmp::Ordering::Equal));
    // average ranks over ties
    let mut ranks = vec![0.0; ps.len()];
    let mut i = 0;
    while i < idx.len() {
        let mut j = i;
        while j + 1 < idx.len() && ps[idx[j + 1]] == ps[idx[i]] {
            j += 1;
        }
        let r = (i + j) as f64 / 2.0 + 1.0;
        for t in i..=j {
            ranks[idx[t]] = r;
        }
        i = j + 1;
    }
    let sum_pos: f64 = (0..ps.len()).filter(|k| ys[*k] == 1.0).map(|k| ranks[k]).sum();
    Some((sum_pos - npos as f64 * (npos as f64 + 1.0) / 2.0) / (npos * nneg) as f64)
}

/// Equal-mass bins (each ~n/bins rows, by predicted p): [(idx...)]
fn mass_bins(ps: &[f64], bins: usize) -> Vec<Vec<usize>> {
    let mut idx: Vec<usize> = (0..ps.len()).collect();
    idx.sort_by(|a, b| ps[*a].partial_cmp(&ps[*b]).unwrap_or(std::cmp::Ordering::Equal));
    let n = idx.len();
    let b = bins.min(n).max(1);
    (0..b).map(|i| idx[i * n / b..(i + 1) * n / b].to_vec()).filter(|v| !v.is_empty()).collect()
}

pub fn wilson(k: f64, n: f64, z: f64) -> (f64, f64) {
    if n <= 0.0 {
        return (0.0, 1.0);
    }
    let p = k / n;
    let d = 1.0 + z * z / n;
    let c = (p + z * z / (2.0 * n)) / d;
    let h = z * ((p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt()) / d;
    ((c - h).max(0.0), (c + h).min(1.0))
}

/// brier, nll, ece_mass (equal-mass bins), ece_width (equal-width, for comparison with older reports), auc
pub fn point_metrics(ps: &[f64], ys: &[f64], bins: usize) -> (f64, f64, f64, f64, Option<f64>) {
    let n = ys.len().max(1) as f64;
    let brier = ps.iter().zip(ys).map(|(p, y)| (p - y).powi(2)).sum::<f64>() / n;
    let nll = -ps.iter().zip(ys).map(|(p, y)| y * p.max(1e-6).ln() + (1.0 - y) * (1.0 - p).max(1e-6).ln()).sum::<f64>() / n;
    let ece_mass = mass_bins(ps, bins).iter().map(|b| {
        let m = b.len() as f64;
        (b.iter().map(|i| ps[*i]).sum::<f64>() / m - b.iter().map(|i| ys[*i]).sum::<f64>() / m).abs() * m / n
    }).sum();
    let mut ece_w = 0.0;
    for i in 0..bins {
        let (lo, hi) = (i as f64 / bins as f64, (i + 1) as f64 / bins as f64);
        let b: Vec<usize> = (0..ps.len()).filter(|j| (ps[*j] >= lo && ps[*j] < hi) || (i + 1 == bins && ps[*j] >= 1.0)).collect();
        if !b.is_empty() {
            let m = b.len() as f64;
            ece_w += (b.iter().map(|j| ps[*j]).sum::<f64>() / m - b.iter().map(|j| ys[*j]).sum::<f64>() / m).abs() * m / n;
        }
    }
    (brier, nll, ece_mass, ece_w, auc(ps, ys))
}

/// Metrics with cluster-bootstrap 95% CIs (groups = sessions) and an equal-mass reliability table.
pub fn metrics_ci(ps: &[f64], ys: &[f64], groups: &[String], reps: usize, seed: u64) -> Value {
    if ys.is_empty() {
        return json!({"n": 0});
    }
    let bins = 10;
    let (b, l, em, ew, a) = point_metrics(ps, ys, bins);
    let mut rng = Rng::new(seed ^ 0xC1);
    let mut acc: [Vec<f64>; 4] = [vec![], vec![], vec![], vec![]];
    for _ in 0..reps {
        let idx = resample_groups(groups, &mut rng);
        let p2: Vec<f64> = idx.iter().map(|i| ps[*i]).collect();
        let y2: Vec<f64> = idx.iter().map(|i| ys[*i]).collect();
        let (b2, l2, e2, _, a2) = point_metrics(&p2, &y2, bins);
        acc[0].push(b2);
        acc[1].push(l2);
        acc[2].push(e2);
        if let Some(a2) = a2 { acc[3].push(a2); }
    }
    let ci = |v: &Vec<f64>| if v.len() >= 10 { json!([round_to(crate::value::quantile(v, 0.025), 4), round_to(crate::value::quantile(v, 0.975), 4)]) } else { Value::Null };
    let table: Vec<Value> = mass_bins(ps, bins).iter().map(|bi| {
        let m = bi.len() as f64;
        let k: f64 = bi.iter().map(|i| ys[*i]).sum();
        let (lo, hi) = wilson(k, m, 1.96);
        json!({"n": bi.len(), "p_lo": round_to(ps[bi[0]], 3), "p_hi": round_to(ps[*bi.last().unwrap()], 3),
               "mean_p": round_to(bi.iter().map(|i| ps[*i]).sum::<f64>() / m, 3), "success": round_to(k / m, 3),
               "success_ci95": [round_to(lo, 3), round_to(hi, 3)]})
    }).collect();
    let ngroups = groups.iter().collect::<HashSet<_>>().len();
    json!({"n": ys.len(), "groups": ngroups, "base_rate": round_to(ys.iter().sum::<f64>() / ys.len() as f64, 3),
           "brier": round_to(b, 4), "brier_ci95": ci(&acc[0]), "nll": round_to(l, 4), "nll_ci95": ci(&acc[1]),
           "ece_mass": round_to(em, 4), "ece_mass_ci95": ci(&acc[2]), "ece_width": round_to(ew, 4),
           "auc": a.map(|x| round_to(x, 3)), "auc_ci95": ci(&acc[3]), "bootstrap": {"reps": reps, "resample": "sessions"},
           "reliability_equal_mass": table})
}

pub fn reliability_md(title: &str, m: &Value) -> String {
    let mut s = format!("### {title}\n\nn={} (groups {}), base rate {}; Brier {} {}, NLL {} {}, ECE (equal-mass) {} {}, AUC {} {}\n\n| bin p range | n | mean p | success | 95% CI |\n|---|---|---|---|---|\n",
        m["n"], m["groups"], m["base_rate"], m["brier"], m["brier_ci95"], m["nll"], m["nll_ci95"], m["ece_mass"], m["ece_mass_ci95"], m["auc"], m["auc_ci95"]);
    for r in m["reliability_equal_mass"].as_array().cloned().unwrap_or_default() {
        s.push_str(&format!("| {}-{} | {} | {} | {} | {} |\n", r["p_lo"], r["p_hi"], r["n"], r["mean_p"], r["success"], r["success_ci95"]));
    }
    s
}

// ---------------------------------------------------------------- splits

pub struct Split {
    pub train: Vec<usize>,
    pub test: Vec<usize>,
    pub desc: Value,
}

/// Held-out split by group keys. mode: session | task | run | family | run+family.
pub fn split_rows(keys: &[(String, String, String, String)], mode: &str, holdout: f64, seed: u64) -> Split {
    // keys: (session, run, task, family)
    let pick = |vals: Vec<String>, salt: u64| -> (HashSet<String>, usize) {
        let mut v: Vec<String> = vals.into_iter().collect::<HashSet<_>>().into_iter().collect();
        v.sort();
        let n = v.len();
        if n < 2 {
            return (HashSet::new(), n);
        }
        crate::tools::shuffle(&mut v, seed ^ salt);
        let k = ((n as f64 * holdout).round() as usize).clamp(1, n - 1);
        (v[..k].iter().cloned().collect(), n)
    };
    let col = |i: usize| keys.iter().map(|k| match i { 0 => k.0.clone(), 1 => k.1.clone(), 2 => k.2.clone(), _ => k.3.clone() }).collect::<Vec<_>>();
    let dims: Vec<usize> = match mode { "session" => vec![0], "task" => vec![2], "run" => vec![1], "family" => vec![3], _ => vec![1, 3] };
    let mut used = vec![];
    let mut notes = vec![];
    let mut tests: Vec<(usize, HashSet<String>)> = vec![];
    for d in &dims {
        let (t, n) = pick(col(*d), *d as u64 * 7919);
        let name = ["session", "run", "task", "family"][*d];
        if t.is_empty() {
            notes.push(format!("{name} split impossible: {n} distinct {name}(s)"));
        } else {
            let mut held: Vec<String> = t.iter().cloned().collect();
            held.sort();
            used.push(json!({"by": name, "distinct": n, "held_out": held}));
            tests.push((*d, t));
        }
    }
    if tests.is_empty() {
        // nothing to keep apart: fall back to a session split so there is still a held-out set
        let (t, _) = pick(col(0), 1);
        notes.push("fell back to a session split".into());
        tests.push((0, t));
    }
    fn get(k: &(String, String, String, String), d: usize) -> &String {
        match d { 0 => &k.0, 1 => &k.1, 2 => &k.2, _ => &k.3 }
    }
    let (mut train, mut test, mut dropped) = (vec![], vec![], 0usize);
    for (i, k) in keys.iter().enumerate() {
        let ins: Vec<bool> = tests.iter().map(|(d, t)| t.contains(get(k, *d))).collect();
        if ins.iter().all(|x| *x) {
            test.push(i);
        } else if ins.iter().all(|x| !*x) {
            train.push(i);
        } else {
            dropped += 1; // crosses: its run is held out but not its family, or the other way round
        }
    }
    Split { train, test, desc: json!({"mode": mode, "holdout": holdout, "seed": seed, "held_out": used, "notes": notes,
        "train_rows": 0, "test_rows": 0, "dropped_crossing_rows": dropped}) }.finish()
}

impl Split {
    fn finish(mut self) -> Self {
        self.desc["train_rows"] = json!(self.train.len());
        self.desc["test_rows"] = json!(self.test.len());
        self
    }
}

pub fn read_families(path: Option<&str>) -> HashMap<String, String> {
    path.and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), pystr(Some(v)))).collect())).unwrap_or_default()
}

/// session -> (run, task, family) for a set of logs (used by the direct-predictor trainer)
pub fn session_groups(paths: &[String], families: &HashMap<String, String>) -> HashMap<String, (String, String, String)> {
    let mut m = HashMap::new();
    for f in crate::tools::files_of(paths) {
        let g = file_groups(&f, families);
        for e in crate::tools::events(&f).iter().take(50) {
            let sess = s(e, "session");
            if !sess.is_empty() {
                m.entry(sess.to_string()).or_insert_with(|| g.clone());
            }
        }
    }
    m
}

pub const HYGIENE_NOTES: &[&str] = &[
    "labels: rows carry the option actually taken; gate-side wait/check/heal/review rows are no longer counted as direct",
    "handed-back steps are labelled with the next executed action of the session (<=180 s); none were labelled before",
    "blocked states have no outcome under direct and executed states none under untaken options: q_o is observational (confounded by the logged policy) until counterfactual branches exist (bench/counterfactual)",
    "safety outcomes and risky steps are excluded",
    "step labels = no driver error / stale error / immediate repeat; 'no error' is not 'helped the task' (use --label task for run success)",
    "regularisation centre: zero (treatment) / hand-set v1 (direct), not the built-in weights fitted on these logs",
];

// ---------------------------------------------------------------- train --model treatment

pub fn train_treatment_main(argv: &[String]) -> i32 {
    let mut paths = vec![];
    let mut test_paths: Vec<String> = vec![];
    let mut task_label_files: Vec<String> = vec![];
    let (mut out, mut holdout, mut seed, mut split, mut label, mut prior_name) =
        (String::new(), 0.3, 0u64, "run+family".to_string(), "step".to_string(), "zero".to_string());
    let (mut boot, mut level, mut min_rows, mut report, mut md, mut fit_all, mut fams) = (30usize, 0.9, 30usize, false, None::<String>, false, None::<String>);
    let mut fc = FitCfg::default();
    let mut in_test = false;
    let mut i = 0;
    let next = |i: &mut usize| { *i += 1; argv.get(*i).cloned().unwrap_or_default() };
    while i < argv.len() {
        match argv[i].as_str() {
            "--model" => { let _ = next(&mut i); }
            "-o" | "--out" => out = next(&mut i),
            "--holdout" => holdout = next(&mut i).parse().unwrap_or(holdout),
            "--seed" => seed = next(&mut i).parse().unwrap_or(seed),
            "--l2" => fc.l2 = next(&mut i).parse().unwrap_or(fc.l2),
            "--l2-int" => fc.l2_int = next(&mut i).parse().unwrap_or(fc.l2_int),
            "--l2-main" => fc.l2_main = next(&mut i).parse().unwrap_or(fc.l2_main),
            "--iters" => fc.iters = next(&mut i).parse().unwrap_or(fc.iters),
            "--boot" => boot = next(&mut i).parse().unwrap_or(boot),
            "--level" => level = next(&mut i).parse().unwrap_or(level),
            "--min-rows" => min_rows = next(&mut i).parse().unwrap_or(min_rows),
            "--split" => split = next(&mut i),
            "--label" => label = next(&mut i),
            "--task-labels" => task_label_files.push(next(&mut i)),
            "--families" => fams = Some(next(&mut i)),
            "--prior" => prior_name = next(&mut i),
            "--md" => md = Some(next(&mut i)),
            "--report" => report = true,
            "--fit-all" => fit_all = true,
            "--test" => in_test = true,
            "-h" | "--help" => { println!("usage: gui-gate train --model treatment LOGS... -o value.json [--split run+family|run|family|task|session] [--holdout 0.3] [--seed 0] [--l2 0.01] [--l2-main 0.01] [--l2-int 0.05] [--boot 30] [--level 0.9] [--min-rows 30] [--label step|task --task-labels runs_scored.jsonl] [--families map.json] [--prior zero|v1|builtin] [--report] [--md FILE] [--fit-all] [--test LOGS...]"); return 0; }
            p if in_test => test_paths.push(p.to_string()),
            p => paths.push(p.to_string()),
        }
        i += 1;
    }
    let families = read_families(fams.as_deref());
    let tl = read_task_labels(&task_label_files);
    let (rows, stats) = load_rows(&paths, &label, &tl, &families);
    if rows.len() < 20 {
        println!("{}", json!({"error": "need at least 20 labelled rows", "rows": rows.len(), "load": stats.json()}));
        return 1;
    }
    let prior = match prior_name.as_str() { "builtin" => crate::decide::prior_weights(), "v1" => crate::decide::prior_v1_weights(), _ => IndexMap::new() };
    let (train_idx, test_rows_ext, split_desc): (Vec<usize>, Vec<TRow>, Value) = if test_paths.is_empty() {
        let keys: Vec<(String, String, String, String)> = rows.iter().map(|r| (r.session.clone(), r.run.clone(), r.task.clone(), r.family.clone())).collect();
        let sp = split_rows(&keys, &split, holdout, seed);
        let test: Vec<TRow> = sp.test.iter().map(|i| rows[*i].clone()).collect();
        (sp.train, test, sp.desc)
    } else {
        let (ext, _) = load_rows(&test_paths, &label, &tl, &families);
        ((0..rows.len()).collect(), ext, json!({"mode": "external", "test_paths": test_paths}))
    };
    let train: Vec<&TRow> = train_idx.iter().map(|i| &rows[*i]).collect();
    let n_by: IndexMap<String, usize> = VALUE_OPTIONS.iter().map(|o| (o.to_string(), train.iter().filter(|r| r.option == *o).count())).collect();
    let allowed: HashSet<String> = n_by.iter().filter(|(_, n)| **n > 0).map(|(k, _)| k.clone()).collect();
    let coef = fit_treatment(&train, &fc, &prior, None, &allowed);
    let mut rng = Rng::new(seed);
    let groups: Vec<String> = train.iter().map(|r| r.session.clone()).collect();
    let mut bfc = fc.clone();
    bfc.iters = (fc.iters / 3).max(200);
    let mut reps = vec![];
    for _ in 0..boot {
        let idx = resample_groups(&groups, &mut rng);
        let sample: Vec<&TRow> = idx.iter().map(|i| train[*i]).collect();
        reps.push(fit_treatment(&sample, &bfc, &prior, Some(&coef), &allowed));
    }
    let mut model = TreatmentModel { name: crate::sidefx::basename(&out).trim_end_matches(".json").to_string(), coef, boot: reps,
        n_by_option: n_by, probe: fit_probe(&train, &fc), level, min_rows };
    if model.name.is_empty() { model.name = "treatment".into(); }
    // evaluation on held-out rows: q of the option actually taken
    let test: Vec<&TRow> = test_rows_ext.iter().collect();
    let feats = |r: &TRow| -> crate::features::Feats { FEATURE_NAMES.iter().enumerate().map(|(k, n)| (*n, r.f[k])).collect() };
    let ps: Vec<f64> = test.iter().map(|r| model.q(&feats(r), &r.option)).collect();
    let ys: Vec<f64> = test.iter().map(|r| r.y).collect();
    let gs: Vec<String> = test.iter().map(|r| r.session.clone()).collect();
    let creps = if report { 200 } else { 50 };
    let mut per_opt = serde_json::Map::new();
    for o in VALUE_OPTIONS {
        let ix: Vec<usize> = (0..test.len()).filter(|i| test[*i].option == *o).collect();
        if ix.len() >= 5 {
            per_opt.insert(o.to_string(), metrics_ci(&ix.iter().map(|i| ps[*i]).collect::<Vec<_>>(), &ix.iter().map(|i| ys[*i]).collect::<Vec<_>>(),
                                                    &ix.iter().map(|i| gs[*i].clone()).collect::<Vec<_>>(), creps, seed));
        }
    }
    let leak = prior_name == "builtin";
    let eval = json!({"label": label, "split": split_desc, "prior_center": prior_name, "prior_leak_risk": leak,
        "load": stats.json(), "train_rows": train.len(), "test_rows": test.len(),
        "treatment_on_test": metrics_ci(&ps, &ys, &gs, creps, seed), "treatment_on_test_by_option": per_opt,
        "incremental_calibration": "not estimable from logs (no paired branches per state); run bench/counterfactual and its analyze.py",
        "hygiene": HYGIENE_NOTES});
    if fit_all && test_paths.is_empty() {
        let all: Vec<&TRow> = rows.iter().collect();
        model.coef = fit_treatment(&all, &fc, &prior, Some(&model.coef), &allowed);
        model.n_by_option = VALUE_OPTIONS.iter().map(|o| (o.to_string(), all.iter().filter(|r| r.option == *o).count())).collect();
    }
    if let Some(p) = md {
        let mut s = format!("# value model report ({})\n\nsplit: {}\n\nprior centre: {prior_name} (leak risk: {leak})\n\nload: {}\n\n", model.name, eval["split"], eval["load"]);
        s.push_str(&reliability_md("treatment model, held-out rows (option actually taken)", &eval["treatment_on_test"]));
        for (o, m) in eval["treatment_on_test_by_option"].as_object().unwrap() {
            s.push('\n');
            s.push_str(&reliability_md(&format!("option {o}"), m));
        }
        s.push_str("\n## label hygiene\n\n");
        for n in HYGIENE_NOTES { s.push_str(&format!("- {n}\n")); }
        let _ = std::fs::write(&p, s);
    }
    if !out.is_empty() {
        let mut doc = model.to_json();
        doc["eval"] = eval.clone();
        if let Err(e) = std::fs::write(&out, serde_json::to_string_pretty(&doc).unwrap()) {
            eprintln!("cannot write {out}: {e}");
            return 1;
        }
    }
    println!("{}", serde_json::to_string_pretty(&eval).unwrap());
    0
}

// ---------------------------------------------------------------- gui-gate costs

/// Measured option costs from gate logs:
///   extra_s: wall seconds the option adds before the step is attempted - gate-side options: their gate_ms minus the
///            sessions' median direct gate_ms; hand-backs: refusal -> next action event of the session (the model's
///            extra turn); direct = 0 by definition;
///   tokens:  image tokens of the observations between the refusal and the next action (px / 750, an estimate:
///            the gate does not see the model's own token counts; merge Pi usage for exact numbers);
///   person_s: the person's answer time (human.ms) on ask_* / handoff decisions.
pub fn measure_costs(paths: &[String]) -> Value {
    let mut acc: IndexMap<String, Vec<(f64, f64, f64)>> = IndexMap::new();
    let mut direct_gate_ms: Vec<f64> = vec![];
    let mut gateside: Vec<(String, f64)> = vec![];
    for file in crate::tools::files_of(paths) {
        let evs = crate::tools::events(&file);
        for (i, e) in evs.iter().enumerate() {
            if s(e, "ev") != "action" {
                continue;
            }
            let dec = s(e, "decision");
            let opt = option_taken(e).or_else(|| if matches!(dec, "ask_human" | "handoff") { Some(dec.to_string()) } else { None });
            let Some(o) = opt else { continue };
            let gms = as_f64(e.get("gate_ms")).unwrap_or(0.0);
            let person = e.get("human").and_then(|h| as_f64(h.get("ms"))).unwrap_or(0.0) / 1000.0;
            if truthy(e.get("executed")) {
                if o == "direct" {
                    direct_gate_ms.push(gms);
                } else {
                    gateside.push((o, gms));
                }
                continue;
            }
            let ts = e["ts"].as_f64().unwrap_or(0.0);
            let mut tokens = 0.0;
            let mut gap = None;
            for e2 in &evs[i + 1..] {
                if s(e2, "ev") == "obs" {
                    tokens += as_f64(e2.get("px")).unwrap_or(0.0) / 750.0;
                }
                if s(e2, "ev") == "action" {
                    gap = Some((e2["ts"].as_f64().unwrap_or(ts) - ts).max(0.0));
                    break;
                }
            }
            if let Some(g) = gap.filter(|g| *g < 600.0) {
                acc.entry(o).or_default().push(((g - person).max(0.0) + gms / 1000.0, tokens, person));
            }
        }
    }
    let med = crate::value::quantile(&direct_gate_ms, 0.5);
    let med = if med.is_nan() { 0.0 } else { med };
    for (o, g) in gateside {
        acc.entry(o).or_default().push((((g - med) / 1000.0).max(0.0), 0.0, 0.0));
    }
    let mut costs = serde_json::Map::new();
    costs.insert("direct".into(), json!({"extra_s": 0.0, "tokens": 0.0, "person_s": 0.0, "n": direct_gate_ms.len(), "direct_gate_ms_median": round_to(med, 2)}));
    for (o, v) in &acc {
        let n = v.len() as f64;
        let col = |k: usize| v.iter().map(|x| match k { 0 => x.0, 1 => x.1, _ => x.2 }).collect::<Vec<f64>>();
        costs.insert(o.clone(), json!({"extra_s": round_to(col(0).iter().sum::<f64>() / n, 3), "extra_s_median": round_to(crate::value::quantile(&col(0), 0.5), 3),
            "tokens": round_to(col(1).iter().sum::<f64>() / n, 1), "person_s": round_to(col(2).iter().sum::<f64>() / n, 2), "n": v.len()}));
    }
    json!({"kind": "costs_v1", "costs": costs, "source": paths,
           "note": "means (expected cost); use with GUI_GATE_COSTS. Options with fewer than GUI_GATE_COST_MIN_N (5) measurements fall back to the prior seconds."})
}

pub fn costs_main(argv: &[String]) -> i32 {
    let mut out = String::new();
    let mut paths = vec![];
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "-o" | "--out" => { out = argv.get(i + 1).cloned().unwrap_or_default(); i += 1; }
            "-h" | "--help" => { println!("usage: gui-gate costs LOGS... [-o costs.json]  (then GUI_GATE_COSTS=costs.json)"); return 0; }
            p => paths.push(p.to_string()),
        }
        i += 1;
    }
    let d = measure_costs(&paths);
    let txt = serde_json::to_string_pretty(&d).unwrap();
    if !out.is_empty() {
        if let Err(e) = std::fs::write(&out, &txt) {
            eprintln!("cannot write {out}: {e}");
            return 1;
        }
    }
    println!("{txt}");
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(opt: &str, feats: &[(&str, f64)], y: f64, sess: &str) -> TRow {
        let mut f = vec![0.0; NF];
        f[0] = 1.0;
        for (k, v) in feats {
            f[fidx(k).unwrap()] = *v;
        }
        TRow { session: sess.into(), run: "r".into(), task: "t".into(), family: "f".into(), f, option: opt.into(), y, tier: "model".into(), check: None }
    }

    #[test]
    fn no_answer_is_never_a_training_label() {
        for o in ["timeout", "cancelled", "declined", "window_closed", "no_response", "not_asked_again"] {
            let e = json!({"ev": "action", "executed": false, "decision": "ask_help", "trail": ["ask_help"], "tier": "model", "human_outcome": o});
            assert_eq!(option_taken(&e), None, "{o}");
        }
        let e = json!({"ev": "action", "executed": false, "decision": "ask_help", "trail": ["ask_help"], "tier": "model", "human_outcome": "denied"});
        assert_eq!(option_taken(&e).as_deref(), Some("ask_help"));
    }

    #[test]
    fn treatment_fit_recovers_a_negative_and_a_positive_effect() {
        // simulated logs: direct 70% ok; look_light hurts (40%); look_full helps only on duplicate labels (90% vs 30%)
        let mut rows = vec![];
        let mut rng = Rng::new(7);
        let mut u = || (rng.next() >> 11) as f64 / (1u64 << 53) as f64;
        for i in 0..3000 {
            let dup = i % 2 == 0;
            let (opt, p) = match i % 3 {
                0 => ("direct", if dup { 0.3 } else { 0.7 }),
                1 => ("look_light", 0.4),
                _ => ("look_full", if dup { 0.9 } else { 0.6 }),
            };
            let y = if u() < p { 1.0 } else { 0.0 };
            rows.push(row(opt, &[("label_dup", if dup { 1.0 } else { 0.0 })], y, &format!("s{}", i % 60)));
        }
        let refs: Vec<&TRow> = rows.iter().collect();
        let allowed: HashSet<String> = ["direct", "look_light", "look_full"].iter().map(|s| s.to_string()).collect();
        let c = fit_treatment(&refs, &FitCfg { l2: 0.001, l2_int: 0.001, l2_main: 0.001, iters: 4000, ..Default::default() }, &IndexMap::new(), None, &allowed);
        let m = TreatmentModel { coef: c, n_by_option: allowed.iter().map(|o| (o.clone(), 1000)).collect(), ..Default::default() };
        let mut f: crate::features::Feats = FEATURE_NAMES.iter().map(|k| (*k, 0.0)).collect();
        f["bias"] = 1.0;
        let d_light = m.delta(&f, "look_light").mean;
        assert!(d_light < -0.15, "look_light should be estimated harmful: {d_light}");
        let d_full_plain = m.delta(&f, "look_full").mean;
        f["label_dup"] = 1.0;
        let d_full_dup = m.delta(&f, "look_full").mean;
        assert!(d_full_dup > 0.4, "{d_full_dup}");
        assert!(d_full_plain.abs() < 0.12, "{d_full_plain}");
    }

    #[test]
    fn metrics_and_intervals() {
        let ps = vec![0.9, 0.8, 0.7, 0.6, 0.4, 0.3, 0.2, 0.1, 0.95, 0.05];
        let ys = vec![1.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let (b, l, em, _, a) = point_metrics(&ps, &ys, 5);
        assert!(b > 0.0 && b < 0.25 && l > 0.0 && em >= 0.0);
        assert!((a.unwrap() - 0.96).abs() < 1e-9, "{a:?}");
        // perfect calibration on equal-mass bins
        let ps2 = vec![0.5; 10];
        let ys2 = vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0];
        assert!(point_metrics(&ps2, &ys2, 5).2 < 1e-12);
        let g: Vec<String> = (0..10).map(|i| format!("s{}", i % 5)).collect();
        let m = metrics_ci(&ps, &ys, &g, 100, 1);
        let ci = m["brier_ci95"].as_array().unwrap();
        assert!(ci[0].as_f64().unwrap() <= m["brier"].as_f64().unwrap() + 1e-9);
        assert_eq!(m["reliability_equal_mass"].as_array().unwrap().len(), 10);
        let (lo, hi) = wilson(137.0, 255.0, 1.96);
        assert!((lo - 0.476).abs() < 0.002 && (hi - 0.597).abs() < 0.002, "{lo} {hi}");
    }

    #[test]
    fn splits_keep_runs_and_families_apart() {
        let mut keys = vec![];
        for run in ["r1", "r2", "r3"] {
            for fam in ["calc", "writer", "chrome", "gimp"] {
                for k in 0..3 {
                    keys.push((format!("{run}-{fam}-{k}"), run.to_string(), format!("{fam}-{k}"), fam.to_string()));
                }
            }
        }
        let sp = split_rows(&keys, "run+family", 0.3, 0);
        assert!(!sp.train.is_empty() && !sp.test.is_empty());
        let tr_runs: HashSet<&String> = sp.train.iter().map(|i| &keys[*i].1).collect();
        let tr_fams: HashSet<&String> = sp.train.iter().map(|i| &keys[*i].3).collect();
        for i in &sp.test {
            assert!(!tr_runs.contains(&keys[*i].1) && !tr_fams.contains(&keys[*i].3));
        }
        assert!(sp.desc["dropped_crossing_rows"].as_u64().unwrap() > 0);
        // one run only: the run split is reported impossible, the family split still holds
        let one: Vec<_> = keys.iter().filter(|k| k.1 == "r1").cloned().collect();
        let sp1 = split_rows(&one, "run+family", 0.3, 0);
        assert!(sp1.desc["notes"][0].as_str().unwrap().contains("run split impossible"));
        let tf: HashSet<&String> = sp1.train.iter().map(|i| &one[*i].3).collect();
        assert!(sp1.test.iter().all(|i| !tf.contains(&one[*i].3)));
    }

    #[test]
    fn groups_from_paths() {
        let m = HashMap::new();
        assert_eq!(file_groups("results/osworld-verified-v099/gate/chrome-030eeff7-b492-gate8-s0.jsonl", &m),
                   ("osworld-verified-v099".into(), "chrome-030eeff7-b492".into(), "chrome".into()));
        assert_eq!(file_groups("x/run1/gate/ow01-libreoffice_calc-1de6-direct-s2-try2.jsonl", &m).2, "libreoffice_calc");
        let mut fm = HashMap::new();
        fm.insert("chrome-030eeff7".to_string(), "browser".to_string());
        assert_eq!(file_groups("r/gate/chrome-030eeff7-b492-gate8-s0.jsonl", &fm).2, "browser");
    }

    fn write_log(dir: &std::path::Path, name: &str, evs: &[Value]) -> String {
        let p = dir.join(name);
        std::fs::write(&p, evs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n")).unwrap();
        p.to_string_lossy().to_string()
    }

    #[test]
    fn rows_carry_the_option_taken_and_interventions_get_the_next_outcome() {
        let d = std::env::temp_dir().join(format!("gg-vt-{}", rand_hex(4)));
        std::fs::create_dir_all(d.join("runA/gate")).unwrap();
        let f = json!({"bias": 1.0, "is_token": 1.0});
        let evs = vec![
            json!({"ev": "action", "ts": 1.0, "session": "a", "aid": 1, "executed": true, "decision": "direct", "trail": ["direct"], "tier": "model", "features": f, "sig": "click|1", "gate_ms": 1.0, "exec_ms": 50.0}),
            json!({"ev": "action", "ts": 2.0, "session": "a", "aid": 2, "executed": true, "decision": "direct", "trail": ["wait", "direct"], "tier": "model", "features": f, "sig": "click|2", "gate_ms": 1501.0}),
            json!({"ev": "action", "ts": 3.0, "session": "a", "aid": 3, "executed": false, "decision": "look_full", "trail": ["look_full"], "tier": "model", "features": f, "sig": "click|3", "gate_ms": 2.0}),
            json!({"ev": "obs", "ts": 3.1, "session": "a", "px": 750000}),
            json!({"ev": "action", "ts": 13.0, "session": "a", "aid": 4, "executed": true, "decision": "direct", "trail": ["direct"], "tier": "model", "features": f, "sig": "click|3", "is_error": true, "gate_ms": 1.0}),
            json!({"ev": "action", "ts": 14.0, "session": "a", "aid": 5, "executed": false, "decision": "ask_human", "trail": ["ask_human"], "tier": "rule", "features": f, "sig": "click|9", "risk_level": "high", "human": {"ms": 30000.0}}),
            json!({"ev": "action", "ts": 50.0, "session": "a", "aid": 6, "executed": true, "decision": "direct", "trail": ["direct"], "tier": "model", "features": f, "sig": "click|4", "gate_ms": 1.0}),
        ];
        let p = write_log(&d.join("runA/gate"), "chrome-1234-gate8-s0.jsonl", &evs);
        let (rows, st) = load_rows(&[p.clone()], "step", &HashMap::new(), &HashMap::new());
        let opts: Vec<(&str, f64)> = rows.iter().map(|r| (r.option.as_str(), r.y)).collect();
        assert_eq!(opts, vec![("direct", 1.0), ("wait", 1.0), ("look_full", 0.0), ("direct", 0.0), ("direct", 1.0)]);
        assert_eq!(st.safety_excluded, 1);
        assert_eq!(rows[0].run, "runA");
        assert_eq!(rows[0].family, "chrome");
        // task labels: every row of the run gets the run's success
        let mut tl = HashMap::new();
        tl.insert("chrome-1234-gate8-s0".to_string(), false);
        let (rows_t, _) = load_rows(&[p.clone()], "task", &tl, &HashMap::new());
        assert!(rows_t.iter().all(|r| r.y == 0.0));
        // measured costs: look_full = 10 s turn + its gate ms, 1000 image tokens; wait = 1.5 s over direct
        let c = measure_costs(&[p]);
        assert!((c["costs"]["look_full"]["extra_s"].as_f64().unwrap() - 10.002).abs() < 1e-6, "{c}");
        assert_eq!(c["costs"]["look_full"]["tokens"], 1000.0);
        assert!((c["costs"]["wait"]["extra_s"].as_f64().unwrap() - 1.5).abs() < 1e-6);
        assert_eq!(c["costs"]["direct"]["extra_s"], 0.0);
        let t = crate::value::CostTable::from_json(&c);
        assert_eq!(t.measured["look_full"].n, 1);
        let _ = std::fs::remove_dir_all(&d);
    }
}
