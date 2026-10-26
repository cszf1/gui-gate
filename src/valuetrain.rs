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
use crate::features::ALL_FEATURES as FEATURE_NAMES;
use crate::util::*;
use crate::value::{Coefs, TreatmentModel, VALUE_OPTIONS};
use indexmap::IndexMap;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

/// row layout: features::ALL_FEATURES (the 31 v1 features, then the v2 features; v1 rows keep the v2 slots at 0)
const NF: usize = 42;

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
    /// three label layers (research.rs), never standing in for one another: execution (the training label y of the
    /// step model), local progress (post-condition, when the driver could check one) and task success (joined from the
    /// evaluator; descriptive only - never copied onto actions as an action-value label)
    pub y_local: Option<f64>,
    pub y_task: Option<f64>,
    /// the features are the pre-decision state (x_pre logged, or no gate-side option ran so nothing was recomputed)
    pub pre_ok: bool,
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
    /// run_command rows left out of the GUI rows (--commands exclude, the default)
    pub commands_excluded: usize,
    /// actions of a second or later session in one log file (an aborted re-run appended to it; --all-sessions keeps them)
    pub later_sessions_excluded: usize,
    pub by_option: IndexMap<String, usize>,
    pub by_tier: IndexMap<String, usize>,
    /// rows whose pre-decision features cannot be recovered (logs without x_pre whose trail ran wait / check / heal):
    /// out of the causal rows, counted (and kept only with --include-post, for descriptive statistics)
    pub pre_unrecoverable: usize,
    pub rows_with_x_pre: usize,
    /// hand-backs labelled by their continuation: same candidate (and effect / decision id where logged)
    pub handback_attributed: usize,
    /// hand-backs whose next executed action was a different candidate / effect: "cannot be attributed", no label
    pub handback_unattributable: usize,
}

impl LoadStats {
    fn json(&self) -> Value {
        json!({"files": self.files, "executed_rows": self.executed_rows, "intervention_rows": self.intervention_rows,
               "interventions_unlabelled_dropped": self.interventions_unlabelled, "safety_outcomes_excluded": self.safety_excluded,
               "risky_steps_excluded": self.risky_excluded, "environment_labels": self.env_labels,
               "files_without_task_label": self.files_without_task_label,
               "commands_excluded": self.commands_excluded, "later_sessions_excluded": self.later_sessions_excluded,
               "pre_decision_state_unrecoverable": self.pre_unrecoverable, "rows_with_x_pre": self.rows_with_x_pre,
               "handback_attributed": self.handback_attributed, "handback_unattributable": self.handback_unattributable,
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

/// The logged decision-time features. A collect-mode shadow probe (`shadow_check`) is NOT copied in: the live gate
/// decided without it, so as a feature it would be information from outside the decision (it stays the probe
/// model's label, TRow::check).
fn feats_vec(e: &Value) -> Vec<f64> {
    let mut v = vec![0.0; NF];
    let fo = e.get(feature_key(e));
    for (i, k) in FEATURE_NAMES.iter().enumerate() {
        v[i] = as_f64(fo.and_then(|f| f.get(*k))).unwrap_or(0.0);
    }
    v[0] = 1.0;
    v
}

/// The key of the decision-time features: `x_pre` (v0.10.3 logs: immutable, before any gate-side option) when logged,
/// else the legacy `features` (the state the final decision used).
pub fn feature_key(e: &Value) -> &'static str {
    if e.get("x_pre").map(|x| x.is_object()).unwrap_or(false) { "x_pre" } else { "features" }
}

/// Can this row's pre-decision state be recovered? Yes with x_pre; for older logs only when no gate-side option ran
/// (no wait / check / heal in the trail: then `features` was computed once, before the decision, and never changed).
pub fn pre_state_recoverable(e: &Value) -> bool {
    if feature_key(e) == "x_pre" {
        return true;
    }
    let trail: Vec<String> = e.get("trail").and_then(|t| t.as_array()).map(|a| a.iter().map(|x| pystr(Some(x))).collect()).unwrap_or_default();
    !trail.iter().any(|t| matches!(t.as_str(), "wait" | "check" | "healed" | "heal" | "heal_risky"))
}

/// Local-progress label of an executed action: the logged layer when present, else the driver's effect report.
fn local_label(e: &Value) -> Option<f64> {
    if let Some(l) = e.get("labels").and_then(|l| l.get("local")).and_then(|x| x.as_str()) {
        return match l { "met" => Some(1.0), "not_met" => Some(0.0), _ => None };
    }
    match s(e, "effect") {
        "none" => Some(0.0),
        "verified" | "changed" | "ok" => Some(1.0),
        _ if truthy(e.get("is_error")) => Some(0.0),
        _ => None,
    }
}

/// What load_rows keeps.
#[derive(Clone, Debug)]
pub struct LoadOpts {
    /// run_command rows: exclude (default: commands are not GUI steps) | include (0.10.1 behaviour) | only
    pub commands: String,
    /// keep every session of a log file (default false: only the first; a later one is an aborted re-run)
    pub all_sessions: bool,
    /// v1 (as logged) | v2 (GUI-only history and last_* recomputed without commands, plus features::V2_FEATURES)
    pub features: String,
    /// keep rows whose pre-decision state is unrecoverable (descriptive statistics only; never for causal estimates)
    pub include_post: bool,
}

impl Default for LoadOpts {
    fn default() -> Self {
        LoadOpts { commands: "exclude".into(), all_sessions: false, features: "v1".into(), include_post: false }
    }
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

/// y = the step's execution label (no driver error / stale error / immediate repeat); hand-backs get the label of
/// their continuation only when it is the same candidate action (and the same effect / decision where logged).
/// label = "task" no longer replaces y by the task's success (0.10.2 copied it onto every action of the task):
/// rows keep y and carry the task result in y_task, for task-level descriptive statistics only.
pub fn load_rows(paths: &[String], label: &str, task_labels: &HashMap<String, bool>, families: &HashMap<String, String>) -> (Vec<TRow>, LoadStats) {
    load_rows_opts(paths, label, task_labels, families, &LoadOpts::default())
}

pub fn load_rows_opts(paths: &[String], label: &str, task_labels: &HashMap<String, bool>, families: &HashMap<String, String>, opts: &LoadOpts) -> (Vec<TRow>, LoadStats) {
    let v2 = opts.features == "v2";
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
        let mut last_gui: Option<(bool, bool, bool)> = None; // (is_error, unverifiable, popup) of the last GUI action
        // pending hand-backs: (row index or None when the row was not kept, ts, sig, effect id, decision id)
        let mut pending: Vec<(Option<usize>, f64, String, String, String)> = vec![];
        let first_sess = evs.iter().map(|e| s(e, "session")).find(|x| !x.is_empty()).unwrap_or("").to_string();
        for e0 in evs.iter().filter(|e| s(e, "ev") == "action") {
            let mut e = e0.clone();
            let sess = s(&e, "session").to_string();
            if !opts.all_sessions && sess != first_sess {
                st.later_sessions_excluded += 1;
                continue;
            }
            let is_cmd = crate::features::is_command_tool(s(&e, "tool"));
            if (is_cmd && opts.commands == "exclude") || (!is_cmd && opts.commands == "only") {
                if is_cmd { st.commands_excluded += 1; }
                continue; // not a row, not a label for a pending hand-back, not GUI history
            }
            let ts = e["ts"].as_f64().unwrap_or(0.0);
            let sig = s(&e, "sig").to_string();
            let fk = feature_key(&e);
            let pre_ok = pre_state_recoverable(&e);
            if fk == "x_pre" { st.rows_with_x_pre += 1; }
            if v2 {
                // the v2 gate computes these over GUI actions only (features::features with GateState::gui_history)
                let role = e.get("target").map(|t| s(t, "role").to_string()).unwrap_or_default();
                let kind = e.get("target").map(|t| s(t, "kind").to_string()).unwrap_or_default();
                let tool = s(&e, "tool").to_string();
                if let Some(fm) = e.get_mut(fk).and_then(|f| f.as_object_mut()) {
                    for (k, v) in crate::features::history_feats(&prev, &sig).into_iter().chain(crate::features::v2_feats(&tool, &kind, &role)) {
                        fm.insert(k.to_string(), json!(v));
                    }
                    let (le, lu, lp) = last_gui.unwrap_or((false, false, false));
                    let b = |x: bool| if x { 1.0 } else { 0.0 };
                    fm.insert("last_err".into(), json!(b(le)));
                    fm.insert("last_unverif".into(), json!(b(lu)));
                    fm.insert("last_popup".into(), json!(b(lp)));
                }
            } else if e.get(fk).and_then(|f| f.get("sess_fail")).is_none() {
                let role = e.get("target").map(|t| s(t, "role").to_string()).unwrap_or_default();
                if let Some(fm) = e.get_mut(fk).and_then(|f| f.as_object_mut()) {
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
                let _ = label;
                let y = if step_ok { 1.0 } else { 0.0 };
                let y_task = task_y.map(|t| if t { 1.0 } else { 0.0 });
                // a hand-back is labelled by its continuation only when it is the same candidate action: with the
                // logged continuation link (v0.10.3) the same decision and effect; in older logs the same signature
                let cont = e.get("continues").filter(|c| c.is_object()).cloned();
                let eid = e.get("candidate").map(|c| s(c, "effect_id").to_string()).unwrap_or_default();
                for (i, t0, psig, peid, pdid) in pending.drain(..) {
                    let attributable = ts - t0 <= crate::research::CONTINUE_WINDOW_S && match &cont {
                        Some(c) => s(c, "decision_id") == pdid && truthy(c.get("attributable")),
                        None => psig == sig && (peid.is_empty() || eid.is_empty() || peid == eid),
                    };
                    if attributable {
                        st.handback_attributed += 1;
                        if let Some(i) = i { rows[i].y = y; rows[i].y_local = local_label(&e); }
                    } else {
                        st.handback_unattributable += 1;
                    }
                }
                match (opt, risky) {
                    (Some(_), false) if !pre_ok && !opts.include_post => { st.pre_unrecoverable += 1; }
                    (Some(o), false) => {
                        if !pre_ok { st.pre_unrecoverable += 1; }
                        st.executed_rows += 1;
                        rows.push(TRow { session: sess, run: run.clone(), task: task.clone(), family: family.clone(), f: feats_vec(&e), option: o, y, tier, check,
                                         y_local: local_label(&e), y_task, pre_ok });
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
                last_gui = Some((err, s(&e, "effect") == "unverifiable", truthy(e.get("popup"))));
                last = Some((sig, ts, err));
            } else {
                let eid = e.get("candidate").map(|c| s(c, "effect_id").to_string()).unwrap_or_default();
                let did = s(&e, "decision_id").to_string();
                match (opt, risky) {
                    (Some(_), false) if !pre_ok && !opts.include_post => {
                        st.pre_unrecoverable += 1;
                        pending.push((None, ts, sig.clone(), eid, did));
                    }
                    (Some(o), false) => {
                        if !pre_ok { st.pre_unrecoverable += 1; }
                        st.intervention_rows += 1;
                        pending.push((Some(rows.len()), ts, sig.clone(), eid, did));
                        rows.push(TRow { session: sess, run: run.clone(), task: task.clone(), family: family.clone(), f: feats_vec(&e), option: o, y: f64::NAN, tier, check,
                                         y_local: None, y_task: task_y.map(|t| if t { 1.0 } else { 0.0 }), pre_ok });
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
    fit_treatment_w(rows, None, fc, prior, init, allowed)
}

/// fit_treatment with per-row weights (inverse-propensity weights; None = all 1). Weighted mean NLL.
pub fn fit_treatment_w(rows: &[&TRow], wts: Option<&[f64]>, fc: &FitCfg, prior: &IndexMap<String, f64>, init: Option<&Coefs>, allowed: &HashSet<String>) -> Coefs {
    let center: Vec<f64> = FEATURE_NAMES.iter().map(|k| prior.get(*k).copied().unwrap_or(0.0)).collect();
    let mut w = match init { Some(c) => from_coefs(c), None => Dense { base: center.clone(), inter: vec![vec![0.0; NF]; VALUE_OPTIONS.len()] } };
    let wt = |i: usize| wts.map(|v| v[i]).unwrap_or(1.0);
    let n = (0..rows.len()).map(wt).sum::<f64>().max(1e-9);
    let on: Vec<bool> = VALUE_OPTIONS.iter().map(|o| *o != "direct" && allowed.contains(*o)).collect();
    for _ in 0..fc.iters {
        let mut gb = vec![0.0; NF];
        let mut gi = vec![vec![0.0; NF]; VALUE_OPTIONS.len()];
        for (ri, r) in rows.iter().enumerate() {
            let oi = opt_idx(&r.option);
            let d = (sigmoid(dense_logit(&w, r)) - r.y) * wt(ri);
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

/// Default bootstrap replicates of `gui-gate train --model treatment` (30 = pipeline check only).
pub const DEFAULT_BOOT: usize = 200;

/// Share of training states where each option has support together with direct (state-level overlap).
fn state_support(rows: &[&TRow], prop: &Propensity, clip: f64) -> Value {
    let n = rows.len().max(1) as f64;
    Value::Object(prop.opts.iter().map(|o| {
        let k = rows.iter().filter(|r| prop.p_of(r, o) >= clip && prop.p_of(r, "direct") >= clip).count();
        (o.clone(), json!(round_to(k as f64 / n, 4)))
    }).collect())
}

/// Cross-fitted doubly robust estimate of tau_o = E[Y(o) − Y(direct)] (5 folds by session). On each held-out fold,
/// with the outcome model m̂ and the propensity ê fitted on the other folds:
///   phi_o = m̂_o(X) − m̂_d(X) + 1[A=o]/ê_o(X)·(Y − m̂_o(X)) − 1[A=d]/ê_d(X)·(Y − m̂_d(X)).
/// Rows where ê_o or ê_d is below `clip` are trimmed (counted): the estimand is then tau_o on the overlap region only.
/// Valid only if (1) the option means the same thing in every row (one treatment definition), (2) the logged
/// pre-decision features contain every common cause of the choice and the outcome (no unmeasured confounding),
/// (3) overlap holds on the reported region, and (4) the label is the execution layer as defined - DR protects against
/// ONE of m̂ / ê being misspecified, it does not create missing counterfactuals.
pub fn dr_estimates(rows: &[&TRow], fc: &FitCfg, prior: &IndexMap<String, f64>, allowed: &HashSet<String>, clip: f64, seed: u64) -> Value {
    let k = 5usize;
    let mut sess: Vec<String> = rows.iter().map(|r| r.session.clone()).collect::<HashSet<_>>().into_iter().collect();
    sess.sort();
    crate::tools::shuffle(&mut sess, seed ^ 0xD2);
    let fold_of: HashMap<String, usize> = sess.iter().enumerate().map(|(i, x)| (x.clone(), i % k)).collect();
    let opts: Vec<String> = VALUE_OPTIONS.iter().filter(|o| **o != "direct" && allowed.contains(**o)).map(|o| o.to_string()).collect();
    let mut phis: HashMap<String, Vec<(String, f64)>> = HashMap::new();
    let mut trimmed: HashMap<String, usize> = HashMap::new();
    let feats = |r: &TRow| -> crate::features::Feats { FEATURE_NAMES.iter().enumerate().map(|(j, n)| (*n, r.f[j])).collect() };
    for fold in 0..k {
        let tr: Vec<&TRow> = rows.iter().filter(|r| fold_of[&r.session] != fold).copied().collect();
        let te: Vec<&TRow> = rows.iter().filter(|r| fold_of[&r.session] == fold).copied().collect();
        if tr.len() < 20 || te.is_empty() {
            continue;
        }
        let m = TreatmentModel { coef: fit_treatment(&tr, fc, prior, None, allowed), ..Default::default() };
        let pr = Propensity::fit(&tr, fc);
        for r in &te {
            let x = feats(r);
            let (md, ed) = (m.q(&x, "direct"), pr.p_of(r, "direct"));
            for o in &opts {
                let (mo, eo) = (m.q(&x, o), pr.p_of(r, o));
                if eo < clip || ed < clip {
                    *trimmed.entry(o.clone()).or_default() += 1;
                    continue;
                }
                let a_o = if r.option == *o { 1.0 } else { 0.0 };
                let a_d = if r.option == "direct" { 1.0 } else { 0.0 };
                let phi = mo - md + a_o / eo * (r.y - mo) - a_d / ed * (r.y - md);
                phis.entry(o.clone()).or_default().push((r.session.clone(), phi));
            }
        }
    }
    let mut out = serde_json::Map::new();
    for o in &opts {
        let v = phis.get(o).cloned().unwrap_or_default();
        if v.len() < 10 {
            out.insert(o.clone(), json!({"n": v.len(), "trimmed": trimmed.get(o).copied().unwrap_or(0), "tau": Value::Null, "note": "too few rows with overlap"}));
            continue;
        }
        let mean = v.iter().map(|x| x.1).sum::<f64>() / v.len() as f64;
        let groups: Vec<String> = v.iter().map(|x| x.0.clone()).collect();
        let mut rng = Rng::new(seed ^ 0xD3);
        let bs: Vec<f64> = (0..200).map(|_| { let ix = resample_groups(&groups, &mut rng); ix.iter().map(|i| v[*i].1).sum::<f64>() / ix.len().max(1) as f64 }).collect();
        out.insert(o.clone(), json!({"n": v.len(), "trimmed": trimmed.get(o).copied().unwrap_or(0), "tau": round_to(mean, 4),
            "ci95": [round_to(crate::value::quantile(&bs, 0.025), 4), round_to(crate::value::quantile(&bs, 0.975), 4)]}));
    }
    json!({"estimand": "tau_o = E[Y(o) - Y(direct)] on the overlap region (e_o, e_direct >= clip), Y = execution label",
           "formula": "phi = m_o(X) - m_d(X) + 1[A=o]/e_o(X) (Y - m_o(X)) - 1[A=d]/e_d(X) (Y - m_d(X)); 5 session folds (cross-fitting); CI: session bootstrap of phi",
           "assumptions": ["one treatment definition per option", "pre-decision features contain all common causes (x_pre only)", "overlap on the reported region", "DR is consistent if either the outcome model or the propensity is right; it does not create missing counterfactuals"],
           "clip": clip, "by_option": out})
}

pub const HYGIENE_NOTES: &[&str] = &[
    "labels: rows carry the option actually taken; gate-side wait/check/heal/review rows are no longer counted as direct",
    "handed-back steps are labelled by their continuation only when it is the same candidate action (same decision / effect where logged, same signature in older logs) within 180 s; otherwise 'cannot be attributed' (no label, counted)",
    "features: x_pre (immutable pre-decision state) where logged; older rows whose trail ran wait / check / heal have no recoverable pre-decision state and are out of the causal rows (counted; --include-post keeps them for description)",
    "labels: execution / local progress / task are separate layers; the task result is never copied onto actions",
    "blocked states have no outcome under direct and executed states none under untaken options: q_o is observational (confounded by the logged policy) until counterfactual branches exist (bench/counterfactual)",
    "safety outcomes and risky steps are excluded",
    "step labels = no driver error / stale error / immediate repeat; 'no error' is not 'helped the task' (use --label task for run success)",
    "regularisation centre: zero (treatment) / hand-set v1 (direct), not the built-in weights fitted on these logs",
    "commands (run_command) are not GUI rows by default; only the first session of a log file is used (a later one is an aborted re-run)",
    "a collect-mode shadow probe is not a feature (the live decision never saw it); propensities never see probe results (post-decision)",
    "without >= MIN_BOOT bootstrap replicates the model's intervals are unknown and the gate does not use it",
];

// ---------------------------------------------------------------- train --model treatment

pub fn train_treatment_main(argv: &[String]) -> i32 {
    let mut paths = vec![];
    let mut test_paths: Vec<String> = vec![];
    let mut task_label_files: Vec<String> = vec![];
    let (mut out, mut holdout, mut seed, mut split, mut label, mut prior_name) =
        (String::new(), 0.3, 0u64, "run+family".to_string(), "step".to_string(), "zero".to_string());
    let (mut boot, mut level, mut min_rows, mut report, mut md, mut fit_all, mut fams) = (DEFAULT_BOOT, 0.9, 30usize, false, None::<String>, false, None::<String>);
    let mut ipw: Option<f64> = None;
    let mut support_clip = 0.05;
    let mut dr = false;
    let mut min_support = 0.2;
    let mut lopts = LoadOpts::default();
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
            "--ipw" => ipw = Some(ipw.unwrap_or(0.05)),
            "--ipw-clip" => ipw = Some(next(&mut i).parse().unwrap_or(0.05)),
            "--fit-all" => fit_all = true,
            "--commands" => lopts.commands = next(&mut i),
            "--all-sessions" => lopts.all_sessions = true,
            "--features" => lopts.features = next(&mut i),
            "--min-support" => min_support = next(&mut i).parse().unwrap_or(min_support),
            "--test" => in_test = true,
            "--include-post" => lopts.include_post = true,
            "--support-clip" => support_clip = next(&mut i).parse().unwrap_or(support_clip),
            "--dr" => dr = true,
            "-h" | "--help" => { println!("usage: gui-gate train --model treatment LOGS... -o value.json [--split run+family|run|family|task|session] [--holdout 0.3] [--seed 0] [--l2 0.01] [--l2-main 0.01] [--l2-int 0.05] [--boot 30] [--level 0.9] [--min-rows 30] [--label step|task --task-labels runs_scored.jsonl] [--families map.json] [--prior zero|v1|builtin] [--ipw] [--ipw-clip 0.05] [--min-support 0.2] [--commands exclude|include|only] [--all-sessions] [--features v1|v2] [--report] [--md FILE] [--fit-all] [--test LOGS...] [--include-post] [--support-clip 0.05] [--dr]\n  --boot: bootstrap replicates (default 200; each replicate refits the outcome model AND, under --ipw, the propensity, with the full iteration count; fewer than 200 is a pipeline check, not a decision interval)\n  --label task: refused (a task result copied onto every action is not an action-value label)\n  --include-post: keep rows whose pre-decision state is unrecoverable (descriptive only)\n  --support-clip: state-level overlap threshold saved with the model (e_o(x) and e_direct(x) >= clip)\n  --dr: cross-fitted doubly robust estimates of tau_o = E[Y(o) - Y(direct)] on the training rows (5 session folds)\n  --commands: run_command rows are not GUI rows (default exclude)\n  --all-sessions: keep later sessions of a log file (default: first session only)\n  --features v2: GUI-only history + v2 features (model kind treatment_v2, for GUI_GATE_VALUE_MODEL=v2)\n  --ipw: weight rows by 1/max(p(option|x), clip) from a propensity model of the logged policy (ESS reported)\n  --report: also compare interaction vs separate-per-option vs boosted-stumps models on the held-out split"); return 0; }
            p if in_test => test_paths.push(p.to_string()),
            p => paths.push(p.to_string()),
        }
        i += 1;
    }
    if label == "task" {
        println!("{}", json!({"error": "--label task is refused: the task's success copied onto every action of the task is not an action-value label (long trajectories are weighted more and the later policy is mixed in). Use the step label (execution layer); task success is reported per task, offline (bench/offline_v2.py)."}));
        return 2;
    }
    let families = read_families(fams.as_deref());
    let tl = read_task_labels(&task_label_files);
    let (rows, stats) = load_rows_opts(&paths, &label, &tl, &families, &lopts);
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
        let (ext, _) = load_rows_opts(&test_paths, &label, &tl, &families, &lopts);
        ((0..rows.len()).collect(), ext, json!({"mode": "external", "test_paths": test_paths}))
    };
    let train: Vec<&TRow> = train_idx.iter().map(|i| &rows[*i]).collect();
    let n_by: IndexMap<String, usize> = VALUE_OPTIONS.iter().map(|o| (o.to_string(), train.iter().filter(|r| r.option == *o).count())).collect();
    let allowed: HashSet<String> = n_by.iter().filter(|(_, n)| **n > 0).map(|(k, _)| k.clone()).collect();
    // inverse-propensity weighting (opt-in; the default stays unweighted)
    // the logged policy's propensity: always fitted (state-level coverage at run time), used for weights under --ipw
    let prop = Propensity::fit(&train, &fc);
    let (wts, ipw_info) = match ipw {
        Some(clip) => { let (w, i) = ipw_weights(&train, &prop, clip); (Some(w), i) }
        None => (None, Value::Null),
    };
    let coef = fit_treatment_w(&train, wts.as_deref(), &fc, &prior, None, &allowed);
    let mut rng = Rng::new(seed);
    let groups: Vec<String> = train.iter().map(|r| r.session.clone()).collect();
    // every replicate repeats the WHOLE estimation on its resample: the propensity is refitted (under --ipw) and the
    // outcome model runs the full iteration count (0.10.2 reused the original weights and a third of the iterations)
    let mut reps = vec![];
    for _ in 0..boot {
        let idx = resample_groups(&groups, &mut rng);
        let sample: Vec<&TRow> = idx.iter().map(|i| train[*i]).collect();
        let sw: Option<Vec<f64>> = ipw.map(|clip| { let p = Propensity::fit(&sample, &fc); ipw_weights(&sample, &p, clip).0 });
        reps.push(fit_treatment_w(&sample, sw.as_deref(), &fc, &prior, Some(&coef), &allowed));
    }
    // under IPW an option is covered only by its effective sample size and only where the logged policy overlaps
    let n_cov = if ipw.is_some() { ipw_effective_counts(&ipw_info, &n_by, min_rows, min_support) } else { n_by.clone() };
    let mut model = TreatmentModel { name: crate::sidefx::basename(&out).trim_end_matches(".json").to_string(), coef, boot: reps,
        kind: if lopts.features == "v2" { "treatment_v2".into() } else { "treatment_v1".into() },
        n_by_option: n_cov.clone(), probe: fit_probe(&train, &fc), level, min_rows, propensity: Some(prop.to_model(support_clip)) };
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
        "ipw": ipw_info, "coverage": {"rows_by_option": Value::Object(n_by.iter().map(|(k, n)| (k.clone(), json!(n))).collect()),
            "counted_by_option": Value::Object(n_cov.iter().map(|(k, n)| (k.clone(), json!(n))).collect()), "min_rows": min_rows, "min_support": min_support,
            "rule": if ipw.is_some() { "IPW: min(n, ESS), 0 when support < min_support" } else { "rows (no IPW)" }},
        "load_opts": {"commands": lopts.commands, "all_sessions": lopts.all_sessions, "features": lopts.features},
        "intervals": {"bootstrap_replicates": boot, "min_boot": crate::value::MIN_BOOT, "usable": boot >= crate::value::MIN_BOOT,
                      "refit_per_replicate": {"outcome_iters": fc.iters, "propensity_refit": ipw.is_some()},
                      "note": if boot < DEFAULT_BOOT { "fewer than 200 replicates: a pipeline check (e.g. 30), not a stable interval for decisions" } else { "percentile interval over session-bootstrap refits" }},
        "state_coverage": {"propensity_saved": true, "clip": support_clip, "rule": "an option is considered at state x only when e_o(x) >= clip and e_direct(x) >= clip and it has min_rows effective rows; otherwise: no estimate basis (legacy heuristic fallback, logged)",
                           "train_states_supported": state_support(&train, &prop, support_clip)},
        "dr": if dr { dr_estimates(&train, &fc, &prior, &allowed, support_clip, seed) } else { Value::Null },
        "labels": {"y": "execution layer (no driver error / stale error / immediate repeat)", "local": "post-condition layer where the driver reported an effect", "task": "joined per task, descriptive only (never an action label)"},
        "hygiene": HYGIENE_NOTES});
    let mut eval = eval;
    if report {
        eval["variants"] = compare_variants(&train, &test, &fc, &prior, &allowed, ipw, creps, seed);
    }
    if fit_all && test_paths.is_empty() {
        let all: Vec<&TRow> = rows.iter().collect();
        model.coef = fit_treatment(&all, &fc, &prior, Some(&model.coef), &allowed);
        if ipw.is_none() {
            model.n_by_option = VALUE_OPTIONS.iter().map(|o| (o.to_string(), all.iter().filter(|r| r.option == *o).count())).collect();
        }
    }
    if let Some(p) = md {
        let mut s = format!("# value model report ({})\n\nsplit: {}\n\nprior centre: {prior_name} (leak risk: {leak})\n\nload: {}\n\n", model.name, eval["split"], eval["load"]);
        s.push_str(&reliability_md("treatment model, held-out rows (option actually taken)", &eval["treatment_on_test"]));
        for (o, m) in eval["treatment_on_test_by_option"].as_object().unwrap() {
            s.push('\n');
            s.push_str(&reliability_md(&format!("option {o}"), m));
        }
        if let Some(v) = eval.get("variants").and_then(|v| v.get("models")).and_then(|m| m.as_object()) {
            s.push_str("\n## model variants on the same held-out split\n\n| model | n | Brier | NLL | ECE (equal-mass) |\n|---|---|---|---|---|\n");
            for (k, m) in v {
                s.push_str(&format!("| {k} | {} | {} {} | {} {} | {} {} |\n", m["n"], m["brier"], m["brier_ci95"], m["nll"], m["nll_ci95"], m["ece_mass"], m["ece_mass_ci95"]));
            }
            if !eval["ipw"].is_null() {
                s.push_str(&format!("\nIPW: {}\n", eval["ipw"]));
            }
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

// ---------------------------------------------------------------- propensities (IPW) and model variants

/// The logged policy's propensity P(option | x): multinomial (softmax) logistic over the options seen in training.
pub struct Propensity {
    pub opts: Vec<String>,
    pub w: Vec<Vec<f64>>, // per option, NF weights (the first option is the reference with all zeros)
}

/// Features that exist only AFTER an option ran (the probe result of `check`): never a propensity input, or
/// "check was taken" becomes perfectly predictable and the overlap diagnostics lie.
pub const POST_DECISION_FEATURES: &[&str] = &["check_pass", "check_fail"];

fn post_decision_mask() -> Vec<bool> {
    FEATURE_NAMES.iter().map(|k| POST_DECISION_FEATURES.contains(k)).collect()
}

impl Propensity {
    /// The saved form for the gate's state-level coverage check (value::PropensityModel).
    pub fn to_model(&self, clip: f64) -> crate::value::PropensityModel {
        crate::value::PropensityModel { opts: self.opts.clone(), clip,
            w: self.w.iter().map(|wc| FEATURE_NAMES.iter().enumerate().map(|(j, k)| (k.to_string(), wc[j])).collect()).collect() }
    }

    pub fn fit(rows: &[&TRow], fc: &FitCfg) -> Propensity {
        let mut opts: Vec<String> = rows.iter().map(|r| r.option.clone()).collect::<HashSet<_>>().into_iter().collect();
        opts.sort_by_key(|o| opt_idx(o));
        let k = opts.len();
        let mut w = vec![vec![0.0; NF]; k];
        let n = rows.len().max(1) as f64;
        let iters = fc.iters.max(300);
        let post = post_decision_mask();
        for _ in 0..iters {
            let mut g = vec![vec![0.0; NF]; k];
            for r in rows {
                let p = Self::probs(&w, &r.f);
                let yi = opts.iter().position(|o| *o == r.option).unwrap_or(0);
                for c in 1..k {
                    let d = p[c] - if c == yi { 1.0 } else { 0.0 };
                    for (j, x) in r.f.iter().enumerate() {
                        if *x != 0.0 && !post[j] {
                            g[c][j] += d * x;
                        }
                    }
                }
            }
            for c in 1..k {
                for j in 0..NF {
                    if post[j] {
                        continue; // stays 0: not a decision-time input
                    }
                    let reg = if j == 0 { 0.0 } else { fc.l2 * w[c][j] };
                    w[c][j] -= fc.lr * (g[c][j] / n + reg);
                }
            }
        }
        Propensity { opts, w }
    }

    fn probs(w: &[Vec<f64>], f: &[f64]) -> Vec<f64> {
        let z: Vec<f64> = w.iter().map(|wc| wc.iter().zip(f).map(|(a, b)| a * b).sum::<f64>()).collect();
        let m = z.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let e: Vec<f64> = z.iter().map(|x| (x - m).exp()).collect();
        let s: f64 = e.iter().sum();
        e.iter().map(|x| x / s).collect()
    }

    pub fn p(&self, r: &TRow) -> f64 {
        self.p_of(r, &r.option)
    }

    /// P(option o | the row's state), whatever option the row took (0 for an option never seen in training)
    pub fn p_of(&self, r: &TRow, o: &str) -> f64 {
        let ps = Self::probs(&self.w, &r.f);
        self.opts.iter().position(|x| x == o).map(|i| ps[i]).unwrap_or(0.0)
    }
}

/// Inverse-propensity weights 1/max(p̂(o|x), clip), normalised to mean 1, and the effective sample size
/// (Σw)²/Σw² overall and per option.
pub fn ipw_weights(rows: &[&TRow], prop: &Propensity, clip: f64) -> (Vec<f64>, Value) {
    let ps: Vec<f64> = rows.iter().map(|r| prop.p(r)).collect();
    let clipped = ps.iter().filter(|p| **p < clip).count();
    let raw: Vec<f64> = ps.iter().map(|p| 1.0 / p.max(clip)).collect();
    let mean = raw.iter().sum::<f64>() / raw.len().max(1) as f64;
    let w: Vec<f64> = raw.iter().map(|x| x / mean.max(1e-12)).collect();
    let ess = |ix: &[usize]| { let a: f64 = ix.iter().map(|i| w[*i]).sum(); let b: f64 = ix.iter().map(|i| w[*i] * w[*i]).sum(); if b > 0.0 { a * a / b } else { 0.0 } };
    let all: Vec<usize> = (0..rows.len()).collect();
    let mut per = serde_json::Map::new();
    for o in &prop.opts {
        let ix: Vec<usize> = (0..rows.len()).filter(|i| rows[*i].option == *o).collect();
        let pm = ix.iter().map(|i| ps[*i]).sum::<f64>() / ix.len().max(1) as f64;
        // overlap: on what share of ALL logged states could the logged policy have taken o (p̂(o|x) >= clip)?
        let support = rows.iter().filter(|r| prop.p_of(r, o) >= clip).count() as f64 / rows.len().max(1) as f64;
        let clipped_o = ix.iter().filter(|i| ps[**i] < clip).count();
        per.insert(o.clone(), json!({"n": ix.len(), "ess": round_to(ess(&ix), 1), "mean_propensity": round_to(pm, 3),
                                     "support": round_to(support, 3), "clipped_rows": clipped_o}));
    }
    (w.clone(), json!({"clip": clip, "clipped_rows": clipped, "n": rows.len(), "ess": round_to(ess(&all), 1),
                       "max_weight": round_to(w.iter().cloned().fold(0.0, f64::max), 2), "by_option": per,
                       "post_decision_features_excluded": POST_DECISION_FEATURES,
                       "note": "propensities estimated from the logged policy (softmax logistic on the decision-time features, probe results excluded); a near-deterministic policy gives tiny overlap and a small ESS; support = share of all states with p(o|x) >= clip"}))
}

/// Coverage under IPW: an option counts min(n, ⌊ESS⌋) rows, and 0 (uncovered: never chosen) when its support - the
/// share of logged states where the logged policy could have taken it - is below `min_support`. `direct` is the
/// reference and keeps its n. Clipping weights does not create overlap; this keeps the model from acting where it
/// has none.
pub fn ipw_effective_counts(info: &Value, n_by: &IndexMap<String, usize>, min_rows: usize, min_support: f64) -> IndexMap<String, usize> {
    let _ = min_rows; // TreatmentModel::covered applies min_rows to the counts returned here
    n_by.iter().map(|(o, n)| {
        if o == "direct" || *n == 0 {
            return (o.clone(), *n);
        }
        let b = &info["by_option"][o.as_str()];
        let sup = b["support"].as_f64().unwrap_or(0.0);
        let ess = b["ess"].as_f64().unwrap_or(0.0);
        let eff = if sup < min_support { 0 } else { (*n).min(ess.floor().max(0.0) as usize) };
        (o.clone(), eff)
    }).collect()
}

/// Variant: one separate logistic model per option (no shared base), each L2 toward zero.
pub struct SeparateModels {
    pub w: HashMap<String, Vec<f64>>,
}

impl SeparateModels {
    pub fn fit(rows: &[&TRow], wts: Option<&[f64]>, fc: &FitCfg) -> SeparateModels {
        let mut w = HashMap::new();
        let opts: HashSet<String> = rows.iter().map(|r| r.option.clone()).collect();
        for o in opts {
            let ix: Vec<usize> = (0..rows.len()).filter(|i| rows[*i].option == o).collect();
            let n = ix.iter().map(|i| wts.map(|v| v[*i]).unwrap_or(1.0)).sum::<f64>().max(1e-9);
            let mut b = vec![0.0; NF];
            for _ in 0..fc.iters {
                let mut g = vec![0.0; NF];
                for i in &ix {
                    let r = rows[*i];
                    let z: f64 = b.iter().zip(&r.f).map(|(a, x)| a * x).sum();
                    let d = (sigmoid(z) - r.y) * wts.map(|v| v[*i]).unwrap_or(1.0);
                    for (k, x) in r.f.iter().enumerate() {
                        if *x != 0.0 {
                            g[k] += d * x;
                        }
                    }
                }
                for k in 0..NF {
                    let reg = if k == 0 { 0.0 } else { fc.l2_int.max(fc.l2) * b[k] };
                    b[k] -= fc.lr * (g[k] / n + reg);
                }
            }
            w.insert(o, b);
        }
        SeparateModels { w }
    }

    pub fn q(&self, r: &TRow) -> f64 {
        match self.w.get(&r.option) {
            Some(b) => sigmoid(b.iter().zip(&r.f).map(|(a, x)| a * x).sum()),
            None => 0.5,
        }
    }
}

/// Variant: gradient-boosted decision stumps (logistic loss, Newton leaves, shrinkage) on the features plus option
/// one-hots and option × feature products, so a stump can split on an option's interaction with the state.
pub struct Stumps {
    pub base: f64,
    pub trees: Vec<(usize, f64, f64, f64)>, // (column, threshold, value if x <= t, value if x > t)
    pub lr: f64,
}

fn stump_x(r: &TRow) -> Vec<f64> {
    let oi = opt_idx(&r.option);
    let k = VALUE_OPTIONS.len();
    let mut x = r.f.clone();
    for o in 1..k {
        x.push(if oi == o { 1.0 } else { 0.0 });
    }
    for o in 1..k {
        for j in 1..NF {
            x.push(if oi == o { r.f[j] } else { 0.0 });
        }
    }
    x
}

impl Stumps {
    pub fn fit(rows: &[&TRow], wts: Option<&[f64]>, rounds: usize, lr: f64, l2: f64) -> Stumps {
        let xs: Vec<Vec<f64>> = rows.iter().map(|r| stump_x(r)).collect();
        let ys: Vec<f64> = rows.iter().map(|r| r.y).collect();
        let wt: Vec<f64> = (0..rows.len()).map(|i| wts.map(|v| v[i]).unwrap_or(1.0)).collect();
        let ncol = xs.first().map(|x| x.len()).unwrap_or(0);
        let sw: f64 = wt.iter().sum::<f64>().max(1e-9);
        let p0 = (ys.iter().zip(&wt).map(|(y, w)| y * w).sum::<f64>() / sw).clamp(1e-3, 1.0 - 1e-3);
        let base = (p0 / (1.0 - p0)).ln();
        let mut f: Vec<f64> = vec![base; rows.len()];
        // candidate thresholds per column: midpoints of up to 12 distinct values
        let cands: Vec<Vec<f64>> = (0..ncol).map(|c| {
            let mut v: Vec<f64> = xs.iter().map(|x| x[c]).collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v.dedup();
            if v.len() <= 1 { return vec![]; }
            let step = (v.len() as f64 / 12.0).ceil().max(1.0) as usize;
            (0..v.len() - 1).step_by(step).map(|i| (v[i] + v[i + 1]) / 2.0).collect()
        }).collect();
        let mut trees = vec![];
        for _ in 0..rounds {
            let g: Vec<f64> = (0..rows.len()).map(|i| (sigmoid(f[i]) - ys[i]) * wt[i]).collect();
            let h: Vec<f64> = (0..rows.len()).map(|i| { let p = sigmoid(f[i]); (p * (1.0 - p)).max(1e-6) * wt[i] }).collect();
            let (gt, ht): (f64, f64) = (g.iter().sum(), h.iter().sum());
            let mut best: Option<(f64, usize, f64, f64, f64)> = None;
            for c in 0..ncol {
                for t in &cands[c] {
                    let (mut gl, mut hl) = (0.0, 0.0);
                    for i in 0..rows.len() {
                        if xs[i][c] <= *t {
                            gl += g[i];
                            hl += h[i];
                        }
                    }
                    let (gr, hr) = (gt - gl, ht - hl);
                    let gain = gl * gl / (hl + l2) + gr * gr / (hr + l2) - gt * gt / (ht + l2);
                    if best.map(|b| gain > b.0).unwrap_or(true) {
                        best = Some((gain, c, *t, -gl / (hl + l2), -gr / (hr + l2)));
                    }
                }
            }
            let Some((gain, c, t, vl, vr)) = best else { break };
            if gain <= 1e-9 {
                break;
            }
            for i in 0..rows.len() {
                f[i] += lr * if xs[i][c] <= t { vl } else { vr };
            }
            trees.push((c, t, vl, vr));
        }
        Stumps { base, trees, lr }
    }

    pub fn q(&self, r: &TRow) -> f64 {
        let x = stump_x(r);
        sigmoid(self.base + self.trees.iter().map(|(c, t, vl, vr)| self.lr * if x[*c] <= *t { *vl } else { *vr }).sum::<f64>())
    }
}

/// Held-out comparison of the value-model variants on the same split: Brier, NLL, ECE (cluster-bootstrap CIs).
pub fn compare_variants(train: &[&TRow], test: &[&TRow], fc: &FitCfg, prior: &IndexMap<String, f64>, allowed: &HashSet<String>,
                        ipw: Option<f64>, reps: usize, seed: u64) -> Value {
    let (wts, ipw_info) = match ipw {
        Some(clip) => { let p = Propensity::fit(train, fc); let (w, i) = ipw_weights(train, &p, clip); (Some(w), i) }
        None => (None, Value::Null),
    };
    let w = wts.as_deref();
    let ys: Vec<f64> = test.iter().map(|r| r.y).collect();
    let gs: Vec<String> = test.iter().map(|r| r.session.clone()).collect();
    let feats = |r: &TRow| -> crate::features::Feats { FEATURE_NAMES.iter().enumerate().map(|(k, n)| (*n, r.f[k])).collect() };
    let mut out = serde_json::Map::new();
    let inter = fit_treatment_w(train, None, fc, prior, None, allowed);
    let m = TreatmentModel { coef: inter, ..Default::default() };
    out.insert("interaction".into(), metrics_ci(&test.iter().map(|r| m.q(&feats(r), &r.option)).collect::<Vec<_>>(), &ys, &gs, reps, seed));
    if w.is_some() {
        let mi = TreatmentModel { coef: fit_treatment_w(train, w, fc, prior, None, allowed), ..Default::default() };
        out.insert("interaction_ipw".into(), metrics_ci(&test.iter().map(|r| mi.q(&feats(r), &r.option)).collect::<Vec<_>>(), &ys, &gs, reps, seed));
    }
    let sep = SeparateModels::fit(train, w, fc);
    out.insert("separate_per_option".into(), metrics_ci(&test.iter().map(|r| sep.q(r)).collect::<Vec<_>>(), &ys, &gs, reps, seed));
    let st = Stumps::fit(train, w, 60, 0.3, 1.0);
    let mut stm = metrics_ci(&test.iter().map(|r| st.q(r)).collect::<Vec<_>>(), &ys, &gs, reps, seed);
    stm["stumps"] = json!(st.trees.len());
    out.insert("boosted_stumps".into(), stm);
    json!({"held_out_rows": test.len(), "train_rows": train.len(), "weighting": if ipw.is_some() { "ipw (separate/stumps fitted with the same weights; metrics unweighted)" } else { "none" },
           "ipw": ipw_info, "models": out,
           "note": "all variants are observational (logged policy, no paired branches); compare on the SAME held-out run+family split"})
}

// ---------------------------------------------------------------- gui-gate costs

/// Measured option costs from gate logs.
///
/// v0.10.3 logs (`chain` / `continues`, research.rs) give COMPONENT TIMES:
///   gate-side options (wait / check / heal): the chain step's own start..end (`src: component_timer`);
///   hand-backs: from the refusal to the decision that continues the SAME candidate (`continues.attributable`); a
///   continuation by another action (the model switched) or none within 600 s is censored and counted, not averaged.
/// Older logs have no component timer: gate-side options use gate_ms over the median direct gate_ms, hand-backs the gap
/// to the session's next action of any kind. That gap includes everything the model did in between, so it is only an
/// UPPER BOUND / correlated proxy of the option's extra cost (`src: next_action_gap_upper_bound`).
///   tokens: the model's real tokens between refusal and next action when the client sent them (usage events), else
///   the image tokens of the observations in between (px / 750, an estimate);
///   person_s: the person's answer time (human.ms) on ask_* / handoff decisions.
/// Every option reports mean (extra_s, used by GUI_GATE_COSTS), median, p90, p99, n and the censored count.
pub fn measure_costs(paths: &[String]) -> Value {
    let mut acc: IndexMap<String, Vec<(f64, f64, f64)>> = IndexMap::new();
    let mut src: IndexMap<String, (usize, usize)> = IndexMap::new(); // option -> (client usage, pixel estimate)
    let mut timing: IndexMap<String, (usize, usize)> = IndexMap::new(); // option -> (component timer, proxy)
    let mut censored: IndexMap<String, usize> = IndexMap::new();
    let (mut ui, mut uo, mut uc, mut un) = (0.0, 0.0, 0.0, 0usize);
    let mut direct_gate_ms: Vec<f64> = vec![];
    let mut gateside: Vec<(String, f64)> = vec![];
    for file in crate::tools::files_of(paths) {
        let evs = crate::tools::events(&file);
        for e in evs.iter().filter(|e| s(e, "ev") == "usage") {
            ui += as_f64(e.get("input")).unwrap_or(0.0);
            uo += as_f64(e.get("output")).unwrap_or(0.0);
            uc += as_f64(e.get("cache_read")).unwrap_or(0.0);
            un += 1;
        }
        for (i, e) in evs.iter().enumerate() {
            if s(e, "ev") != "action" {
                continue;
            }
            let dec = s(e, "decision");
            let opt = option_taken(e).or_else(|| if matches!(dec, "ask_human" | "handoff") { Some(dec.to_string()) } else { None });
            let Some(o) = opt else { continue };
            let gms = as_f64(e.get("gate_ms")).unwrap_or(0.0);
            let person = e.get("human").and_then(|h| as_f64(h.get("ms"))).unwrap_or(0.0) / 1000.0;
            let chain: Vec<Value> = e.get("chain").and_then(|c| c.as_array()).cloned().unwrap_or_default();
            if truthy(e.get("executed")) {
                if o == "direct" {
                    direct_gate_ms.push(gms);
                } else if !chain.is_empty() {
                    // component timer: the option's own steps (a wait that was followed by a check counts both)
                    let secs: f64 = chain.iter().filter(|c| s(c, "option") == o || (o == "heal" && s(c, "option") == "heal"))
                        .map(|c| (as_f64(c.get("t1_ms")).unwrap_or(0.0) - as_f64(c.get("t0_ms")).unwrap_or(0.0)).max(0.0) / 1000.0).sum();
                    acc.entry(o.clone()).or_default().push((secs, 0.0, 0.0));
                    timing.entry(o).or_default().0 += 1;
                } else {
                    gateside.push((o, gms));
                }
                continue;
            }
            let ts = e["ts"].as_f64().unwrap_or(0.0);
            let did = s(e, "decision_id").to_string();
            let mut tokens = 0.0;
            let mut real: Option<f64> = None;
            let mut gap = None;
            let mut component = false;
            for e2 in &evs[i + 1..] {
                if s(e2, "ev") == "obs" {
                    tokens += as_f64(e2.get("px")).unwrap_or(0.0) / 750.0;
                }
                if s(e2, "ev") == "usage" {
                    *real.get_or_insert(0.0) += as_f64(e2.get("input")).unwrap_or(0.0) + as_f64(e2.get("output")).unwrap_or(0.0);
                }
                if s(e2, "ev") == "action" {
                    if !did.is_empty() {
                        // v0.10.3: only a continuation of the same candidate times this hand-back
                        let c = e2.get("continues").cloned().unwrap_or(Value::Null);
                        component = true;
                        if s(&c, "decision_id") == did && truthy(c.get("attributable")) {
                            gap = Some((e2["ts"].as_f64().unwrap_or(ts) - ts).max(0.0));
                        }
                    } else {
                        gap = Some((e2["ts"].as_f64().unwrap_or(ts) - ts).max(0.0));
                    }
                    break;
                }
            }
            match gap.filter(|g| *g < 600.0) {
                Some(g) => {
                    let sc = src.entry(o.clone()).or_default();
                    if real.is_some() { sc.0 += 1 } else { sc.1 += 1 }
                    let tokens = real.unwrap_or(tokens);
                    acc.entry(o.clone()).or_default().push(((g - person).max(0.0) + gms / 1000.0, tokens, person));
                    let t = timing.entry(o).or_default();
                    if component { t.0 += 1 } else { t.1 += 1 }
                }
                None => *censored.entry(o).or_default() += 1,
            }
        }
    }
    let med = crate::value::quantile(&direct_gate_ms, 0.5);
    let med = if med.is_nan() { 0.0 } else { med };
    for (o, g) in gateside {
        acc.entry(o.clone()).or_default().push((((g - med) / 1000.0).max(0.0), 0.0, 0.0));
        timing.entry(o).or_default().1 += 1;
    }
    let mut costs = serde_json::Map::new();
    costs.insert("direct".into(), json!({"extra_s": 0.0, "tokens": 0.0, "person_s": 0.0, "n": direct_gate_ms.len(), "direct_gate_ms_median": round_to(med, 2)}));
    for (o, v) in &acc {
        let n = v.len() as f64;
        let col = |k: usize| v.iter().map(|x| match k { 0 => x.0, 1 => x.1, _ => x.2 }).collect::<Vec<f64>>();
        let (tc, tp) = timing.get(o).copied().unwrap_or((0, 0));
        let q = |p: f64| round_to(crate::value::quantile(&col(0), p), 3);
        costs.insert(o.clone(), json!({"extra_s": round_to(col(0).iter().sum::<f64>() / n, 3), "extra_s_median": q(0.5), "extra_s_p90": q(0.9), "extra_s_p99": q(0.99),
            "tokens": round_to(col(1).iter().sum::<f64>() / n, 1), "person_s": round_to(col(2).iter().sum::<f64>() / n, 2), "n": v.len(),
            "censored": censored.get(o).copied().unwrap_or(0),
            "timing_src": if tp == 0 { "component_timer" } else if tc == 0 { "next_action_gap_upper_bound" } else { "mixed" },
            "tokens_src": match src.get(o) { Some((u, 0)) if *u > 0 => "client_usage", Some((0, _)) | None => "pixel_estimate", _ => "mixed" }}));
    }
    for (o, c) in &censored {
        if !costs.contains_key(o) {
            costs.insert(o.clone(), json!({"n": 0, "censored": c, "extra_s": Value::Null}));
        }
    }
    costs_usage_note(&mut costs, ui, uo, uc, un);
    json!({"kind": "costs_v1", "costs": costs, "source": paths,
           "note": "means (expected cost); use with GUI_GATE_COSTS. Options with fewer than GUI_GATE_COST_MIN_N (5) measurements fall back to the prior seconds. timing_src next_action_gap_upper_bound: logs without component timers - the gap to the next action includes whatever else the model did, so it is an upper bound / correlated proxy, not the option's extra cost; censored = no continuation of the same candidate within 600 s"})
}

fn costs_usage_note(costs: &mut serde_json::Map<String, Value>, i: f64, o: f64, c: f64, n: usize) {
    if let Some(d) = costs.get_mut("direct") {
        d["usage_total"] = json!({"input": i, "output": o, "cache_read": c, "calls": n});
    }
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
        TRow { session: sess.into(), run: "r".into(), task: "t".into(), family: "f".into(), f, option: opt.into(), y, tier: "model".into(), check: None,
               y_local: None, y_task: None, pre_ok: true }
    }

    #[test]
    fn ipw_and_model_variants_on_a_confounded_policy() {
        // the logged policy picks look_full mostly on duplicate labels (confounding); look_full helps there
        let mut rng = Rng::new(11);
        let mut u = || (rng.next() >> 11) as f64 / (1u64 << 53) as f64;
        let mut rows = vec![];
        for i in 0..1200 {
            let dup = u() < 0.5;
            let full = u() < if dup { 0.8 } else { 0.15 };
            let p = match (full, dup) { (true, true) => 0.85, (true, false) => 0.6, (false, true) => 0.3, (false, false) => 0.75 };
            let y = if u() < p { 1.0 } else { 0.0 };
            let mut r = row(if full { "look_full" } else { "direct" }, &[("label_dup", if dup { 1.0 } else { 0.0 })], y, &format!("s{}", i % 60));
            r.run = format!("run{}", i % 4);
            r.family = format!("fam{}", i % 3);
            rows.push(r);
        }
        let all: Vec<&TRow> = rows.iter().collect();
        let fc = FitCfg { iters: 400, ..FitCfg::default() };
        let prop = Propensity::fit(&all, &fc);
        let dup_full = rows.iter().find(|r| r.option == "look_full" && r.f[fidx("label_dup").unwrap()] == 1.0).unwrap();
        let nodup_full = rows.iter().find(|r| r.option == "look_full" && r.f[fidx("label_dup").unwrap()] == 0.0).unwrap();
        assert!(prop.p(dup_full) > 0.65 && prop.p(nodup_full) < 0.3, "{} {}", prop.p(dup_full), prop.p(nodup_full));
        let (w, info) = ipw_weights(&all, &prop, 0.05);
        assert!((w.iter().sum::<f64>() / w.len() as f64 - 1.0).abs() < 1e-9);
        let ess = info["ess"].as_f64().unwrap();
        assert!(ess > 100.0 && ess < 1200.0, "{info}");
        assert!(info["by_option"]["look_full"]["ess"].as_f64().unwrap() > 0.0);
        let (train, test): (Vec<&TRow>, Vec<&TRow>) = all.iter().partition(|r| r.run != "run0");
        let allowed: HashSet<String> = ["direct", "look_full"].iter().map(|x| x.to_string()).collect();
        let v = compare_variants(&train, &test, &fc, &IndexMap::new(), &allowed, Some(0.05), 20, 1);
        for k in ["interaction", "interaction_ipw", "separate_per_option", "boosted_stumps"] {
            let m = &v["models"][k];
            assert!(m["brier"].as_f64().unwrap() < 0.25, "{k}: {m}");
            assert!(m["nll"].as_f64().is_some() && m["ece_mass"].as_f64().is_some(), "{k}");
            assert!(m["brier_ci95"].is_array(), "{k}");
        }
        // the stumps find the option x duplicate-label interaction
        let st = Stumps::fit(&all, None, 60, 0.3, 1.0);
        let q = |o: &str, d: f64| st.q(&row(o, &[("label_dup", d)], 0.0, "x"));
        assert!(q("look_full", 1.0) > q("direct", 1.0) + 0.3, "{} {}", q("look_full", 1.0), q("direct", 1.0));
        assert!(q("direct", 0.0) > q("direct", 1.0) + 0.2);
        let sep = SeparateModels::fit(&all, None, &fc);
        assert!(sep.q(&row("look_full", &[("label_dup", 1.0)], 0.0, "x")) > 0.75);
    }

    #[test]
    fn a_shadow_probe_is_not_a_decision_time_feature() {
        // collect mode probes every step for labelling; the live decision never sees that result, so training must not
        let e = json!({"ev": "action", "features": {"bias": 1.0, "is_token": 1.0}, "shadow_check": true});
        let v = feats_vec(&e);
        assert_eq!(v[fidx("check_pass").unwrap()], 0.0);
        assert_eq!(v[fidx("check_fail").unwrap()], 0.0);
        // a probe the gate really ran before deciding stays a feature
        let e2 = json!({"ev": "action", "features": {"bias": 1.0, "check_pass": 1.0}});
        assert_eq!(feats_vec(&e2)[fidx("check_pass").unwrap()], 1.0);
    }

    #[test]
    fn propensity_does_not_see_the_result_of_the_option_it_explains() {
        // check_pass / check_fail exist only AFTER the check option ran: as propensity inputs they predict
        // "check was taken" perfectly and hide the real overlap
        let mut rows = vec![];
        for i in 0..400 {
            let chk = i % 4 == 0;
            let r = if chk { row("check", &[("check_pass", 1.0)], 1.0, &format!("s{}", i % 40)) } else { row("direct", &[], 1.0, &format!("s{}", i % 40)) };
            rows.push(r);
        }
        let all: Vec<&TRow> = rows.iter().collect();
        let prop = Propensity::fit(&all, &FitCfg { iters: 600, ..FitCfg::default() });
        let pc = prop.p(&rows[0]);
        assert!((pc - 0.25).abs() < 0.05, "P(check|x) must be the base rate when x carries no pre-decision signal: {pc}");
    }

    #[test]
    fn ipw_coverage_follows_effective_sample_size_and_overlap() {
        // the logged policy takes look_full only on duplicate labels (deterministic): no overlap off that region
        let mut rows = vec![];
        for i in 0..600 {
            let dup = i % 2 == 0;
            let full = dup && i % 6 == 0;
            rows.push(row(if full { "look_full" } else { "direct" }, &[("label_dup", if dup { 1.0 } else { 0.0 })], 1.0, &format!("s{}", i % 50)));
        }
        let all: Vec<&TRow> = rows.iter().collect();
        let prop = Propensity::fit(&all, &FitCfg { iters: 600, ..FitCfg::default() });
        let (_, info) = ipw_weights(&all, &prop, 0.05);
        let sup = info["by_option"]["look_full"]["support"].as_f64().unwrap();
        assert!(sup < 0.6, "look_full has support only where labels are duplicated: {info}");
        let n_by: IndexMap<String, usize> = [("direct".to_string(), 500usize), ("look_full".to_string(), 100)].into_iter().collect();
        let eff = ipw_effective_counts(&info, &n_by, 30, 0.8);
        assert_eq!(eff["direct"], 500);
        assert_eq!(eff["look_full"], 0, "an option without overlap over most states is uncovered: {info}");
        let eff2 = ipw_effective_counts(&info, &n_by, 30, 0.2);
        assert!(eff2["look_full"] > 0 && eff2["look_full"] <= 100);
    }

    fn cmd_log(dir: &std::path::Path) -> String {
        let f = json!({"bias": 1.0, "is_token": 1.0});
        let fc = json!({"bias": 1.0, "no_obs": 1.0});
        let mut evs = vec![json!({"ev": "client", "ts": 0.5, "session": "a"})];
        for i in 0..4 {
            evs.push(json!({"ev": "action", "ts": 1.0 + i as f64, "session": "a", "aid": i + 1, "tool": "run_command", "executed": true, "decision": "direct",
                            "trail": ["look_light"], "tier": "rule", "features": fc, "sig": format!("run_command||grep {i} f"), "is_error": true,
                            "target": {"kind": "command", "role": "command", "label": "grep"}}));
        }
        evs.push(json!({"ev": "action", "ts": 9.0, "session": "a", "aid": 9, "tool": "scroll", "executed": true, "decision": "direct", "trail": ["direct"],
                        "tier": "model", "features": {"bias": 1.0, "is_pixel": 1.0, "sess_fail": 0.0, "consec_fail": 3.0}, "sig": "scroll|1|700|500",
                        "target": {"kind": "pixel", "role": "", "label": ""}}));
        evs.push(json!({"ev": "action", "ts": 10.0, "session": "a", "aid": 10, "tool": "click", "executed": true, "decision": "direct", "trail": ["direct"],
                        "tier": "model", "features": f, "sig": "click|1|button|OK", "target": {"kind": "token", "role": "push button", "label": "OK"}}));
        // a second session in the same file (an aborted re-run appended to it)
        evs.push(json!({"ev": "client", "ts": 500.0, "session": "b"}));
        evs.push(json!({"ev": "action", "ts": 501.0, "session": "b", "aid": 1, "tool": "click", "executed": true, "decision": "direct", "trail": ["direct"],
                        "tier": "model", "features": f, "sig": "click|1|button|OK", "is_error": true, "target": {"kind": "token", "role": "push button", "label": "OK"}}));
        write_log(dir, "chrome-9-gate8-s0.jsonl", &evs)
    }

    #[test]
    fn commands_and_later_sessions_are_not_gui_training_rows() {
        let d = std::env::temp_dir().join(format!("gg-vc-{}", rand_hex(4)));
        std::fs::create_dir_all(d.join("runA/gate")).unwrap();
        let p = cmd_log(&d.join("runA/gate"));
        let (rows, st) = load_rows_opts(&[p.clone()], "step", &HashMap::new(), &HashMap::new(), &LoadOpts::default());
        assert_eq!(rows.len(), 2, "4 commands and the second session are out: {rows:?}");
        assert_eq!(st.commands_excluded, 4);
        assert_eq!(st.later_sessions_excluded, 1);
        assert!(rows.iter().all(|r| r.session == "a"));
        // v1 features: as logged (the command failures leaked into consec_fail of the scroll)
        assert_eq!(rows[0].f[fidx("consec_fail").unwrap()], 3.0);
        // the old behaviour stays reachable
        let (all, _) = load_rows_opts(&[p.clone()], "step", &HashMap::new(), &HashMap::new(),
                                      &LoadOpts { commands: "include".into(), all_sessions: true, ..LoadOpts::default() });
        assert_eq!(all.len(), 7);
        let (only, _) = load_rows_opts(&[p.clone()], "step", &HashMap::new(), &HashMap::new(), &LoadOpts { commands: "only".into(), ..LoadOpts::default() });
        assert_eq!(only.len(), 4);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn v2_features_recompute_gui_history_without_commands() {
        let d = std::env::temp_dir().join(format!("gg-v2f-{}", rand_hex(4)));
        std::fs::create_dir_all(d.join("runA/gate")).unwrap();
        let p = cmd_log(&d.join("runA/gate"));
        let (rows, _) = load_rows_opts(&[p], "step", &HashMap::new(), &HashMap::new(), &LoadOpts { features: "v2".into(), ..LoadOpts::default() });
        let (scroll, click) = (&rows[0], &rows[1]);
        assert_eq!(scroll.f[fidx("consec_fail").unwrap()], 0.0, "command exit codes are not GUI failures");
        assert_eq!(scroll.f[fidx("tool_scroll").unwrap()], 1.0);
        assert_eq!(click.f[fidx("tool_click").unwrap()], 1.0);
        assert_eq!(click.f[fidx("role_button").unwrap()], 1.0);
        assert_eq!(click.f[fidx("tool_scroll").unwrap()], 0.0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn no_answer_is_never_a_training_label() {
        for o in ["timeout", "expired", "cancelled", "declined", "window_closed", "no_response", "not_asked_again"] {
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
        // the wait row (trail wait -> direct, no x_pre) has no recoverable pre-decision state: out of the causal rows
        assert_eq!(opts, vec![("direct", 1.0), ("look_full", 0.0), ("direct", 0.0), ("direct", 1.0)]);
        assert_eq!(st.pre_unrecoverable, 1);
        assert_eq!(st.handback_attributed, 1, "look_full on click|3 is continued by click|3");
        assert_eq!(st.safety_excluded, 1);
        let lo = LoadOpts { include_post: true, ..Default::default() };
        let (rows_all, _) = load_rows_opts(&[p.clone()], "step", &HashMap::new(), &HashMap::new(), &lo);
        assert_eq!(rows_all.len(), 5);
        assert!(!rows_all[1].pre_ok && rows_all[1].option == "wait");
        assert_eq!(rows[0].run, "runA");
        assert_eq!(rows[0].family, "chrome");
        // task labels: kept apart (y_task), never copied onto the actions' label
        let mut tl = HashMap::new();
        tl.insert("chrome-1234-gate8-s0".to_string(), false);
        let (rows_t, _) = load_rows(&[p.clone()], "task", &tl, &HashMap::new());
        assert!(rows_t.iter().all(|r| r.y_task == Some(0.0)));
        assert_eq!(rows_t.iter().map(|r| r.y).collect::<Vec<_>>(), rows.iter().map(|r| r.y).collect::<Vec<_>>());
        // measured costs: look_full = 10 s turn + its gate ms, 1000 image tokens; wait = 1.5 s over direct
        let c = measure_costs(&[p]);
        assert!((c["costs"]["look_full"]["extra_s"].as_f64().unwrap() - 10.002).abs() < 1e-6, "{c}");
        assert_eq!(c["costs"]["look_full"]["tokens"], 1000.0);
        assert!((c["costs"]["wait"]["extra_s"].as_f64().unwrap() - 1.5).abs() < 1e-6);
        assert_eq!(c["costs"]["direct"]["extra_s"], 0.0);
        let t = crate::value::CostTable::from_json(&c);
        assert_eq!(t.measured["look_full"].n, 1);
        assert_eq!(c["costs"]["look_full"]["tokens_src"], "pixel_estimate");
        // with the client's real usage on the next call, it replaces the pixel estimate
        let mut evs2 = evs.clone();
        evs2.insert(4, json!({"ev": "usage", "ts": 12.9, "session": "a", "input": 3000.0, "output": 200.0, "cache_read": 9000.0, "same_message": false}));
        let p2 = write_log(&d.join("runA/gate"), "chrome-1234-gate8-s1.jsonl", &evs2);
        let c2 = measure_costs(&[p2]);
        assert_eq!(c2["costs"]["look_full"]["tokens"], 3200.0, "{c2}");
        assert_eq!(c2["costs"]["look_full"]["tokens_src"], "client_usage");
        assert_eq!(c2["costs"]["direct"]["usage_total"]["cache_read"], 9000.0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn x_pre_continuations_and_component_costs_of_the_v0103_contract() {
        let d = std::env::temp_dir().join(format!("gg-vt3-{}", rand_hex(4)));
        std::fs::create_dir_all(d.join("runB/gate")).unwrap();
        let pre = json!({"bias": 1.0, "is_token": 1.0, "loading_cue": 1.0});
        let post = json!({"bias": 1.0, "is_token": 1.0, "loading_cue": 0.0});
        let cand = |sig: &str, eid: &str| json!({"tool": "click", "sig": sig, "effect_id": eid});
        let evs = vec![
            // a wait with x_pre: the row keeps the pre-decision loading cue, its cost is the chain's own wait time
            json!({"ev": "action", "ts": 1.0, "session": "b", "aid": 1, "decision_id": "d-b-1", "executed": true, "decision": "direct", "trail": ["wait", "direct"], "tier": "model",
                   "features": post, "x_pre": pre, "x_post": post, "waited": true, "sig": "click|1", "candidate": cand("click|1", "e1"), "gate_ms": 1600.0,
                   "chain": [{"option": "wait", "t0_ms": 1.0, "t1_ms": 1251.0}], "continues": null, "labels": {"exec": "delivered", "local": "met"}}),
            // look_full refusal continued by the SAME candidate: attributable
            json!({"ev": "action", "ts": 2.0, "session": "b", "aid": 2, "decision_id": "d-b-2", "executed": false, "decision": "look_full", "trail": ["look_full"], "tier": "model",
                   "features": pre, "x_pre": pre, "sig": "click|2", "candidate": cand("click|2", "e2"), "gate_ms": 2.0, "continues": null}),
            json!({"ev": "action", "ts": 8.0, "session": "b", "aid": 3, "decision_id": "d-b-3", "executed": true, "decision": "direct", "trail": ["direct"], "tier": "model",
                   "features": post, "x_pre": post, "sig": "click|2", "candidate": cand("click|2", "e2"), "gate_ms": 1.0, "is_error": true,
                   "continues": {"decision_id": "d-b-2", "same_candidate": true, "same_effect": true, "attributable": true, "model_switched": false}}),
            // look_light refusal, then the model switched to another action: not attributable, no label, censored cost
            json!({"ev": "action", "ts": 9.0, "session": "b", "aid": 4, "decision_id": "d-b-4", "executed": false, "decision": "look_light", "trail": ["look_light"], "tier": "model",
                   "features": pre, "x_pre": pre, "sig": "click|4", "candidate": cand("click|4", "e4"), "gate_ms": 2.0, "continues": null}),
            json!({"ev": "action", "ts": 12.0, "session": "b", "aid": 5, "decision_id": "d-b-5", "executed": true, "decision": "direct", "trail": ["direct"], "tier": "model",
                   "features": post, "x_pre": post, "sig": "press_key|1|ctrl+s", "candidate": cand("press_key|1|ctrl+s", "e5"), "gate_ms": 1.0,
                   "continues": {"decision_id": "d-b-4", "same_candidate": false, "same_effect": false, "attributable": false, "model_switched": true}}),
        ];
        let p = write_log(&d.join("runB/gate"), "chrome-9-gate8-s0.jsonl", &evs);
        let (rows, st) = load_rows(&[p.clone()], "step", &HashMap::new(), &HashMap::new());
        assert_eq!(st.rows_with_x_pre, 5);
        assert_eq!(st.pre_unrecoverable, 0, "x_pre makes the wait row usable");
        let wait = rows.iter().find(|r| r.option == "wait").unwrap();
        assert_eq!(wait.f[fidx("loading_cue").unwrap()], 1.0, "training sees the state BEFORE the wait");
        assert_eq!(wait.y_local, Some(1.0));
        assert_eq!(st.handback_attributed, 1);
        assert_eq!(st.handback_unattributable, 1);
        assert!(rows.iter().all(|r| r.option != "look_light"), "an unattributable hand-back gets no label");
        assert_eq!(rows.iter().find(|r| r.option == "look_full").unwrap().y, 0.0);
        let c = measure_costs(&[p]);
        assert!((c["costs"]["wait"]["extra_s"].as_f64().unwrap() - 1.25).abs() < 1e-9, "{c}");
        assert_eq!(c["costs"]["wait"]["timing_src"], "component_timer");
        assert!((c["costs"]["look_full"]["extra_s"].as_f64().unwrap() - 6.002).abs() < 1e-9, "{c}");
        assert_eq!(c["costs"]["look_full"]["timing_src"], "component_timer");
        assert_eq!(c["costs"]["look_light"]["censored"], 1);
        assert_eq!(c["costs"]["look_light"]["n"], 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn task_label_is_refused_and_dr_recovers_a_known_effect() {
        assert_eq!(train_treatment_main(&["--label".into(), "task".into(), "x".into()]), 2);
        // randomized-like logged policy: look_full taken in ~1/3 of states regardless of x; it raises success by 0.3
        let mut rows = vec![];
        let mut rng = Rng::new(7);
        for i in 0..900 {
            let x = (rng.below(2)) as f64;
            let look = rng.below(3) == 0;
            let p = 0.4 + 0.2 * x + if look { 0.3 } else { 0.0 };
            let y = if (rng.below(1000) as f64) / 1000.0 < p { 1.0 } else { 0.0 };
            rows.push(row(if look { "look_full" } else { "direct" }, &[("label_dup", x)], y, &format!("s{}", i % 60)));
        }
        let refs: Vec<&TRow> = rows.iter().collect();
        let allowed: HashSet<String> = ["direct", "look_full"].iter().map(|x| x.to_string()).collect();
        let fc = FitCfg { iters: 400, ..Default::default() };
        let v = dr_estimates(&refs, &fc, &IndexMap::new(), &allowed, 0.05, 1);
        let tau = v["by_option"]["look_full"]["tau"].as_f64().unwrap();
        assert!((tau - 0.3).abs() < 0.12, "{v}");
        let ci = v["by_option"]["look_full"]["ci95"].as_array().unwrap();
        assert!(ci[0].as_f64().unwrap() < tau && tau < ci[1].as_f64().unwrap());
        assert!(v["assumptions"].as_array().unwrap().len() >= 3);
    }

}
