//! Option values for the value gate (0.9.11).
//!
//! Two value models rank the options a safe step can take (direct, check, heal, wait, look_light, look_full,
//! review, reason, ask_help):
//!
//! * `treatment` - a treatment-interaction logistic model
//!       q_o(x) = P(step succeeds | x, option o taken) = sigmoid(b·x + g_o·x),   g_direct = 0,
//!   one shared feature vector b plus, per option, a main effect (g_o["bias"]) and option × feature interactions,
//!   L2-regularised, fitted by `gui-gate train --model treatment` from logged outcomes WITH THE OPTION ACTUALLY TAKEN.
//!   Incremental value Δ_o(x) = q_o(x) − q_direct(x) may be zero or negative; nothing clamps it. Bootstrap replicates
//!   (resampled by session) give an interval for every Δ_o and for the net benefit of the chosen option over the
//!   cheapest one. It is the default ONLY when trained weights for it are present (GUI_GATE_VALUE_WEIGHTS, or a
//!   `treatment` block in the --weights file).
//! * `legacy` - the 0.9.x prior: the direct predictor re-scored on hand-edited "after" features plus a HAND-SET
//!   residual gain per option, with P_o forced >= P_direct. Its gains are not estimated from any data, and the
//!   feature resets (wait clears loading, review clears duplicate labels) are assumptions. Kept so old runs can be
//!   reproduced (GUI_GATE_VALUE_MODEL=legacy), and active by default while no treatment weights exist.
//!
//! Costs come from logs (`gui-gate costs` -> GUI_GATE_COSTS) when an option has enough measurements; the fixed
//! seconds in decide::default_costs are only the fallback. `check` is valued as information: what acting after
//! seeing the probe result is expected to cost, minus what acting now costs, minus the probe's own cost.
//!
//! Fixed value policies for fair baselines (GUI_GATE_VALUE_POLICY): direct, observe_light, observe_full,
//! extra_reason, threshold, heuristic, random, learned (default). They replace ONLY this module's choice; the safety
//! rules, hard rules, auto-observe, heals and every other mechanism of the gate stay the same.
use crate::decide::{fail_cost, p_direct, sigmoid, Decision, GateConfig};
use crate::features::{fget, ActionCtx, Feats, ALL_FEATURES};
use crate::util::*;
use indexmap::IndexMap;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Mutex;

/// Options the value module ranks (safety outcomes - ask_human, handoff, deny, risk_review - are never valued here).
pub const VALUE_OPTIONS: &[&str] = &["direct", "check", "heal", "wait", "look_light", "look_full", "review", "reason", "ask_help"];
pub const VALUE_POLICIES: &[&str] = &["learned", "direct", "observe_light", "observe_full", "extra_reason", "threshold", "heuristic", "random"];
/// GUI_GATE_VALUE_MODEL values; `legacy` is the default.
pub const VALUE_MODELS: &[&str] = &["legacy", "treatment", "auto", "v2"];
/// Fewer bootstrap replicates than this: the 90 % percentile interval is unknown (not zero-width), and the treatment
/// model is not used for decisions (the legacy prior decides instead).
pub const MIN_BOOT: usize = 20;

// ---------------------------------------------------------------- the decision log of the last choose() call

thread_local! {
    static LAST: RefCell<Value> = const { RefCell::new(Value::Null) };
}

/// The value log of the newest choose() on this thread (taken: the next take returns Null until choose runs again).
pub fn take_last() -> Value {
    LAST.with(|l| std::mem::replace(&mut *l.borrow_mut(), Value::Null))
}

fn set_last(v: Value) {
    LAST.with(|l| *l.borrow_mut() = v);
}

// ---------------------------------------------------------------- treatment-interaction model

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Coefs {
    pub base: IndexMap<String, f64>,
    /// option -> interaction weights (key "bias" is the option's main effect); direct has none (reference)
    pub inter: IndexMap<String, IndexMap<String, f64>>,
}

impl Coefs {
    /// Over ALL_FEATURES: a v1 state has no v2 entries (fget = 0), so v1 models score exactly as before.
    pub fn logit(&self, f: &Feats, o: &str) -> f64 {
        let mut z: f64 = ALL_FEATURES.iter().map(|k| self.base.get(*k).copied().unwrap_or(0.0) * fget(f, k)).sum();
        if o != "direct" {
            if let Some(m) = self.inter.get(o) {
                z += ALL_FEATURES.iter().map(|k| m.get(*k).copied().unwrap_or(0.0) * fget(f, k)).sum::<f64>();
            }
        }
        z
    }
    fn to_json(&self) -> Value {
        let m = |x: &IndexMap<String, f64>| Value::Object(x.iter().filter(|(_, v)| **v != 0.0).map(|(k, v)| (k.clone(), json!(round_to(*v, 5)))).collect());
        json!({"base": m(&self.base), "inter": Value::Object(self.inter.iter().map(|(o, w)| (o.clone(), m(w))).collect())})
    }
    fn from_json(v: &Value) -> Coefs {
        let m = |x: Option<&Value>| -> IndexMap<String, f64> {
            x.and_then(|x| x.as_object()).map(|o| o.iter().filter_map(|(k, v)| v.as_f64().map(|f| (k.clone(), f))).collect()).unwrap_or_default()
        };
        let inter = v.get("inter").and_then(|x| x.as_object())
            .map(|o| o.iter().map(|(k, w)| (k.clone(), m(Some(w)))).collect()).unwrap_or_default();
        Coefs { base: m(v.get("base")), inter }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    pub mean: f64,
    pub lo: f64,
    pub hi: f64,
}

impl Interval {
    pub fn point(x: f64) -> Self {
        Interval { mean: x, lo: x, hi: x }
    }
    /// No (or too few) bootstrap replicates: the interval is unknown, i.e. (-inf, +inf), never a zero-width point.
    pub fn unknown(mean: f64) -> Self {
        Interval { mean, lo: f64::NEG_INFINITY, hi: f64::INFINITY }
    }
    pub fn known(&self) -> bool {
        self.lo.is_finite() && self.hi.is_finite()
    }
    pub fn crosses_zero(&self) -> bool {
        self.lo <= 0.0 && self.hi >= 0.0
    }
    /// [lo, mean, hi]; an unknown bound is null
    pub fn json(&self) -> Value {
        let b = |x: f64| if x.is_finite() { json!(round_to(x, 4)) } else { Value::Null };
        json!([b(self.lo), round_to(self.mean, 4), b(self.hi)])
    }
}

/// Linear-interpolated quantile of an unsorted sample (q in [0,1]).
pub fn quantile(xs: &[f64], q: f64) -> f64 {
    if xs.is_empty() {
        return f64::NAN;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pos = q.clamp(0.0, 1.0) * (v.len() - 1) as f64;
    let (i, frac) = (pos.floor() as usize, pos - pos.floor());
    if i + 1 < v.len() { v[i] * (1.0 - frac) + v[i + 1] * frac } else { v[i] }
}

/// The logged policy's propensity e_o(x) = P(option o | x) (softmax logistic, decision-time features only), saved with a
/// treatment model so the gate can check overlap PER STATE: an option is covered at x only when both e_o(x) and
/// e_direct(x) reach `clip` (and the option has `min_rows` effective rows overall). Without it, coverage per state is
/// unknown and no option but direct is covered.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PropensityModel {
    pub opts: Vec<String>,
    /// per option (same order as opts), weights over ALL_FEATURES; the first option is the reference (all zero)
    pub w: Vec<IndexMap<String, f64>>,
    pub clip: f64,
}

impl PropensityModel {
    pub fn probs(&self, f: &Feats) -> Vec<f64> {
        let z: Vec<f64> = self.w.iter().map(|wc| ALL_FEATURES.iter().map(|k| wc.get(*k).copied().unwrap_or(0.0) * fget(f, k)).sum::<f64>()).collect();
        let m = z.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let e: Vec<f64> = z.iter().map(|x| (x - m).exp()).collect();
        let s: f64 = e.iter().sum();
        e.iter().map(|x| x / s).collect()
    }
    pub fn p(&self, f: &Feats, o: &str) -> f64 {
        let ps = self.probs(f);
        self.opts.iter().position(|x| x == o).map(|i| ps[i]).unwrap_or(0.0)
    }
    pub fn to_json(&self) -> Value {
        json!({"opts": self.opts, "clip": self.clip,
               "w": self.w.iter().map(|m| Value::Object(m.iter().filter(|(_, v)| **v != 0.0).map(|(k, v)| (k.clone(), json!(round_to(*v, 5)))).collect())).collect::<Vec<_>>()})
    }
    pub fn from_json(v: &Value) -> Option<PropensityModel> {
        let opts: Vec<String> = v.get("opts")?.as_array()?.iter().filter_map(|x| x.as_str().map(String::from)).collect();
        let w: Vec<IndexMap<String, f64>> = v.get("w")?.as_array()?.iter()
            .map(|m| m.as_object().map(|o| o.iter().filter_map(|(k, x)| x.as_f64().map(|f| (k.clone(), f))).collect()).unwrap_or_default()).collect();
        if opts.is_empty() || opts.len() != w.len() {
            return None;
        }
        Some(PropensityModel { opts, w, clip: as_f64(v.get("clip")).unwrap_or(0.05) })
    }
}

#[derive(Clone, Debug)]
pub struct TreatmentModel {
    pub name: String,
    pub coef: Coefs,
    /// bootstrap replicates (resampled by session); fewer than MIN_BOOT = interval unknown, model not used
    pub boot: Vec<Coefs>,
    /// treatment_v1 (31 features) | treatment_v2 (ALL_FEATURES, GUI-only history, commands excluded)
    pub kind: String,
    /// training rows per option: options below min_rows are "uncovered" and never chosen by this model
    pub n_by_option: IndexMap<String, usize>,
    /// P(check passes | x) - logistic over the base features (None: derived from q_direct, see check_pass_prob)
    pub probe: Option<IndexMap<String, f64>>,
    pub level: f64,
    pub min_rows: usize,
    /// state-level overlap (None: unknown, so only direct is covered)
    pub propensity: Option<PropensityModel>,
}

impl Default for TreatmentModel {
    fn default() -> Self {
        TreatmentModel { name: "treatment".into(), coef: Coefs::default(), boot: vec![], kind: "treatment_v1".into(), n_by_option: IndexMap::new(), probe: None, level: 0.9, min_rows: 30, propensity: None }
    }
}

impl TreatmentModel {
    pub fn q(&self, f: &Feats, o: &str) -> f64 {
        sigmoid(self.coef.logit(f, o))
    }
    /// Option-level row count only (a necessary condition; the gate uses `covered_at`).
    pub fn covered(&self, o: &str) -> bool {
        o == "direct" || self.n_by_option.get(o).copied().unwrap_or(0) >= self.min_rows
    }
    /// State-level coverage: Ok(()) when this state has support for BOTH direct and o (e_o(x) >= clip and
    /// e_direct(x) >= clip under the saved propensity of the logged policy) and o has min_rows effective rows; otherwise
    /// Err(why) - "no estimate basis" for o here.
    pub fn covered_at(&self, f: &Feats, o: &str) -> Result<(), String> {
        if o == "direct" {
            return Ok(());
        }
        if !self.covered(o) {
            return Err(format!("{} effective training rows (< {})", self.n_by_option.get(o).copied().unwrap_or(0), self.min_rows));
        }
        let Some(p) = &self.propensity else { return Err("no propensity model: overlap at this state unknown".into()) };
        let (eo, ed) = (p.p(f, o), p.p(f, "direct"));
        if eo < p.clip || ed < p.clip {
            return Err(format!("no overlap at this state: e_{o}(x)={eo:.3}, e_direct(x)={ed:.3} (clip {})", p.clip));
        }
        Ok(())
    }
    /// Enough bootstrap replicates for a percentile interval at `level`.
    pub fn has_intervals(&self) -> bool {
        self.boot.len() >= MIN_BOOT
    }
    /// Δ_o = q_o − q_direct with a bootstrap percentile interval at `level` (unknown without enough replicates).
    pub fn delta(&self, f: &Feats, o: &str) -> Interval {
        let d = |c: &Coefs| sigmoid(c.logit(f, o)) - sigmoid(c.logit(f, "direct"));
        let mean = d(&self.coef);
        if !self.has_intervals() {
            return Interval::unknown(mean);
        }
        let xs: Vec<f64> = self.boot.iter().map(d).collect();
        let a = (1.0 - self.level) / 2.0;
        Interval { mean, lo: quantile(&xs, a), hi: quantile(&xs, 1.0 - a) }
    }
    /// Net benefit in seconds of option a over option b: (q_a − q_b)·C_fail − λ·(cost_a − cost_b), with its interval.
    pub fn net_benefit(&self, f: &Feats, a: &str, b: &str, cf: f64, lam_dcost: f64) -> Interval {
        let nb = |c: &Coefs| (sigmoid(c.logit(f, a)) - sigmoid(c.logit(f, b))) * cf - lam_dcost;
        let mean = nb(&self.coef);
        if !self.has_intervals() {
            return Interval::unknown(mean);
        }
        let xs: Vec<f64> = self.boot.iter().map(nb).collect();
        let al = (1.0 - self.level) / 2.0;
        Interval { mean, lo: quantile(&xs, al), hi: quantile(&xs, 1.0 - al) }
    }
    pub fn to_json(&self) -> Value {
        json!({"kind": self.kind, "name": self.name, "level": self.level, "min_rows": self.min_rows,
               "coef": self.coef.to_json(), "boot": self.boot.iter().map(|c| c.to_json()).collect::<Vec<_>>(),
               "n_by_option": Value::Object(self.n_by_option.iter().map(|(k, n)| (k.clone(), json!(n))).collect()),
               "probe": self.probe.as_ref().map(|p| Value::Object(p.iter().map(|(k, x)| (k.clone(), json!(x))).collect())),
               "propensity": self.propensity.as_ref().map(|p| p.to_json())})
    }
    pub fn from_json(v: &Value) -> Result<TreatmentModel, String> {
        let kind = s(v, "kind").to_string();
        if kind != "treatment_v1" && kind != "treatment_v2" {
            return Err("not a treatment_v1 / treatment_v2 model (kind)".into());
        }
        let coef = Coefs::from_json(v.get("coef").unwrap_or(&Value::Null));
        if coef.base.is_empty() {
            return Err("treatment model has no coefficients".into());
        }
        Ok(TreatmentModel {
            name: v.get("name").and_then(|n| n.as_str()).unwrap_or("treatment").to_string(), coef, kind,
            boot: v.get("boot").and_then(|b| b.as_array()).map(|a| a.iter().map(Coefs::from_json).collect()).unwrap_or_default(),
            n_by_option: v.get("n_by_option").and_then(|o| o.as_object())
                .map(|o| o.iter().map(|(k, n)| (k.clone(), n.as_u64().unwrap_or(0) as usize)).collect()).unwrap_or_default(),
            probe: v.get("probe").and_then(|p| p.as_object()).map(|o| o.iter().filter_map(|(k, x)| x.as_f64().map(|f| (k.clone(), f))).collect()),
            level: as_f64(v.get("level")).unwrap_or(0.9),
            min_rows: v.get("min_rows").and_then(|n| n.as_u64()).unwrap_or(30) as usize,
            propensity: v.get("propensity").and_then(PropensityModel::from_json),
        })
    }
    pub fn load(path: &str) -> Result<TreatmentModel, String> {
        let d: Value = serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        // either a bare treatment file or a weights file with a "treatment" block
        TreatmentModel::from_json(d.get("treatment").unwrap_or(&d))
    }
}

// ---------------------------------------------------------------- measured costs

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeasuredCost {
    /// wall seconds the option adds before the step is attempted (direct = 0 by definition)
    pub extra_s: f64,
    pub tokens: f64,
    pub person_s: f64,
    pub n: usize,
}

#[derive(Clone, Debug)]
pub struct CostTable {
    pub measured: IndexMap<String, MeasuredCost>,
    /// seconds charged per 1000 model tokens (GUI_GATE_COST_TOKEN_S, default 0: tokens logged, not charged)
    pub token_s_per_k: f64,
    /// weight of a person's active second relative to a wall second (GUI_GATE_COST_PERSON_W, default 1)
    pub person_w: f64,
    /// fewer measurements than this -> the prior second is used (GUI_GATE_COST_MIN_N, default 5)
    pub min_n: usize,
    pub source: String,
}

impl Default for CostTable {
    fn default() -> Self {
        CostTable { measured: IndexMap::new(), token_s_per_k: 0.0, person_w: 1.0, min_n: 5, source: String::new() }
    }
}

impl CostTable {
    pub fn from_json(v: &Value) -> CostTable {
        let mut t = CostTable::default();
        if let Some(o) = v.get("costs").and_then(|c| c.as_object()) {
            for (k, c) in o {
                t.measured.insert(k.clone(), MeasuredCost {
                    extra_s: as_f64(c.get("extra_s")).unwrap_or(0.0), tokens: as_f64(c.get("tokens")).unwrap_or(0.0),
                    person_s: as_f64(c.get("person_s")).unwrap_or(0.0), n: c.get("n").and_then(|n| n.as_u64()).unwrap_or(0) as usize,
                });
            }
        }
        t
    }
    pub fn load(path: &str) -> Result<CostTable, String> {
        let d: Value = serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let mut t = CostTable::from_json(&d);
        t.source = path.to_string();
        Ok(t)
    }
    /// (seconds, "measured" | "prior")
    pub fn cost(&self, o: &str, prior: &IndexMap<String, f64>) -> (f64, &'static str) {
        match self.measured.get(o) {
            Some(m) if m.n >= self.min_n => (m.extra_s + m.tokens / 1000.0 * self.token_s_per_k + self.person_w * m.person_s, "measured"),
            _ => (prior.get(o).copied().unwrap_or(0.0), "prior"),
        }
    }
}

impl CostTable {
    /// (extra wall seconds before the step, person seconds, "measured" | "prior") for the local net gain G_o: time and
    /// person time are priced apart (lam, gamma).
    pub fn split(&self, o: &str, prior: &IndexMap<String, f64>) -> (f64, f64, &'static str) {
        match self.measured.get(o) {
            Some(m) if m.n >= self.min_n => (m.extra_s + m.tokens / 1000.0 * self.token_s_per_k, m.person_s, "measured"),
            _ => prior_split(o, prior),
        }
    }
}

fn prior_split(o: &str, prior: &IndexMap<String, f64>) -> (f64, f64, &'static str) {
    let c = prior.get(o).copied().unwrap_or(0.0);
    if matches!(o, "ask_help" | "ask_human") { (0.0, c, "prior") } else { (c, 0.0, "prior") }
}

/// (Δt extra seconds, ΔH person seconds, source) of an option, for G_o.
pub fn cost_split(cfg: &GateConfig, o: &str) -> (f64, f64, &'static str) {
    match &cfg.cost_table {
        Some(t) => t.split(o, &cfg.costs),
        None => prior_split(o, &cfg.costs),
    }
}

pub fn cost_of(cfg: &GateConfig, o: &str) -> (f64, &'static str) {
    match &cfg.cost_table {
        Some(t) => t.cost(o, &cfg.costs),
        None => (cfg.costs.get(o).copied().unwrap_or(0.0), "prior"),
    }
}

// ---------------------------------------------------------------- the legacy prior (0.9.x behaviour)

/// What an option fixes beyond the features it resets (see `legacy_after`): HAND-SET, never estimated.
pub fn legacy_gains() -> IndexMap<String, f64> {
    [("check", 0.03), ("heal", 0.65), ("wait", 0.2), ("look_light", 0.15), ("look_full", 0.3), ("review", 0.35), ("ask_help", 0.9)]
        .iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

pub fn legacy_pixel_gain(o: &str) -> Option<f64> {
    match o {
        "look_light" => Some(0.05),
        "look_full" => Some(0.5),
        "check" => Some(0.0),
        _ => None,
    }
}

/// Legacy pseudo-counterfactual: the state an option is ASSUMED to leave behind (hand-edited features).
/// Not an estimate: a wait does not guarantee loading ends, a review does not change a duplicate label.
pub fn legacy_after(f: &Feats, o: &str) -> Feats {
    let mut g = f.clone();
    let mut set = |ks: &[&'static str], v: f64| for k in ks { if let Some(x) = g.get_mut(*k) { *x = v; } };
    match o {
        "look_light" | "look_full" => {
            set(&["no_obs", "token_stale", "log_obs_age", "acts_since_obs", "window_changed", "window_added", "last_popup",
                  "elem_unknown", "last_unverif", "prev_none"], 0.0);
            if o == "look_full" { set(&["obs_had_shot"], 1.0); }
        }
        "wait" => set(&["loading_cue", "last_popup"], 0.0),
        "heal" => set(&["token_stale", "elem_unknown"], 0.0),
        "review" => set(&["label_dup", "repeat", "sig_failed"], 0.0),
        _ => {}
    }
    g
}

/// Legacy P_o: predictor on the assumed after-state, clamped to >= P_direct, plus the hand-set gain.
pub fn legacy_po(f: &Feats, o: &str, pixel: bool, cfg: &GateConfig) -> f64 {
    let p = p_direct(f, cfg);
    if o == "direct" {
        return p;
    }
    let pa = p_direct(&legacy_after(f, o), cfg).max(p);
    let g = if pixel { legacy_pixel_gain(o).unwrap_or_else(|| cfg.gains.get(o).copied().unwrap_or(0.0)) } else { cfg.gains.get(o).copied().unwrap_or(0.0) };
    pa + g * (1.0 - pa)
}

// ---------------------------------------------------------------- check as value of information

/// P(the tree probe confirms the target | x) from a fitted probe model. v0.10.3: without a probe model there is NO
/// estimate (None). 0.10.2 flipped the check features and solved q(x) = P(pass)·q(x, pass) + (1 − P(pass))·q(x, fail)
/// for P(pass); that inverts the direct predictor, it does not measure the probe. Without probe data `check` is only a
/// "cheap verification heuristic" (the legacy grey-zone rule), never a valued option.
pub fn check_pass_prob(f: &Feats, probe: Option<&IndexMap<String, f64>>) -> Option<f64> {
    let w = probe?;
    let z: f64 = ALL_FEATURES.iter().filter(|k| !matches!(**k, "check_pass" | "check_fail")).map(|k| w.get(*k).copied().unwrap_or(0.0) * fget(f, k)).sum();
    Some(sigmoid(z).clamp(0.01, 0.99))
}

pub const CHECK_HEURISTIC_NOTE: &str = "no probe model: check is a cheap verification heuristic (value of information not estimated); if it re-binds the target it also repairs it";

/// The two states a probe can leave: confirmed (check_pass) or not (check_fail).
pub fn probe_states(f: &Feats) -> (Feats, Feats) {
    let (mut fp, mut ff) = (f.clone(), f.clone());
    fp["check_pass"] = 1.0;
    fp["check_fail"] = 0.0;
    ff["check_pass"] = 0.0;
    ff["check_fail"] = 1.0;
    (fp, ff)
}

#[derive(Clone, Debug)]
pub struct Voi {
    pub p_pass: f64,
    pub v_now: f64,
    pub v_pass: f64,
    pub v_fail: f64,
    pub ev_check: f64,
    pub choice_pass: String,
    pub choice_fail: String,
}

impl Voi {
    /// expected seconds saved by probing first (> 0: probe)
    pub fn gain(&self) -> f64 {
        self.v_now - self.ev_check
    }
    pub fn json(&self) -> Value {
        json!({"p_pass": round_to(self.p_pass, 3), "v_now": round_to(self.v_now, 2), "v_pass": round_to(self.v_pass, 2),
               "v_fail": round_to(self.v_fail, 2), "ev_check": round_to(self.ev_check, 2), "gain_s": round_to(self.gain(), 2),
               "after_pass": self.choice_pass, "after_fail": self.choice_fail})
    }
}

/// EV(check) = λ·cost(check) + P(pass)·min_o E[cost(o) | pass] + P(fail)·min_o E[cost(o) | fail], compared with
/// min_o E[cost(o)] now. `score(f, o)` is the expected cost of option o in state f.
pub fn check_voi(f: &Feats, opts: &[&str], score: &dyn Fn(&Feats, &str) -> f64, p_pass: f64, check_cost: f64) -> Voi {
    let best = |g: &Feats| -> (String, f64) {
        opts.iter().filter(|o| **o != "check").map(|o| (o.to_string(), score(g, o)))
            .fold((String::from("direct"), f64::INFINITY), |a, b| if b.1 < a.1 { b } else { a })
    };
    let (fp, ff) = probe_states(f);
    let (_, v_now) = best(f);
    let (cp, v_pass) = best(&fp);
    let (cfail, v_fail) = best(&ff);
    let ev = check_cost + p_pass * v_pass + (1.0 - p_pass) * v_fail;
    Voi { p_pass, v_now, v_pass, v_fail, ev_check: ev, choice_pass: cp, choice_fail: cfail }
}

// ---------------------------------------------------------------- fixed value policies (fair baselines)

static REASONED: Mutex<Option<HashSet<String>>> = Mutex::new(None);
static RANDOM_N: Mutex<u64> = Mutex::new(0);

fn splitmix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

fn look_for(ctx: &ActionCtx, cfg: &GateConfig) -> &'static str {
    if ctx.target.kind == "pixel" && !cfg.text_only { "look_full" } else { "look_light" }
}

const REASON_TEXT: &str = "think once more before acting: is this the right target and the right action for the task? No new observation is attached; send the same action again if it is right, or a different one";

/// A fixed policy's choice (None: the learned / legacy model decides).
pub fn fixed_policy(ctx: &ActionCtx, f: &Feats, cfg: &GateConfig, p: f64) -> Option<Decision> {
    let nothing_to_see = matches!(ctx.target.kind.as_str(), "keyboard" | "none") && fget(f, "no_obs") != 0.0;
    let d = |o: &str, why: String| Some(Decision::new(o, p, format!("value policy {}: {why}", cfg.value_policy), "policy"));
    match cfg.value_policy.as_str() {
        "direct" => d("direct", "no extra observation or reasoning".into()),
        "observe_light" | "observe_full" => {
            let shot = cfg.value_policy == "observe_full" && !cfg.text_only;
            let fresh = fget(f, "no_obs") == 0.0 && fget(f, "acts_since_obs") == 0.0 && (!shot || fget(f, "obs_had_shot") != 0.0);
            if fresh || nothing_to_see {
                d("direct", "fresh observation".into())
            } else {
                d(if shot { "look_full" } else { "look_light" }, "observe before every action".into())
            }
        }
        "extra_reason" => {
            // one extra think per distinct step: the re-sent step runs
            let mut g = REASONED.lock().unwrap();
            let set = g.get_or_insert_with(HashSet::new);
            if set.remove(&ctx.signature) {
                d("direct", "re-sent after the extra think".into())
            } else {
                set.insert(ctx.signature.clone());
                d("reason", REASON_TEXT.into())
            }
        }
        "threshold" => {
            if p >= cfg.value_threshold || nothing_to_see {
                d("direct", format!("P(direct)={p:.2} >= {}", cfg.value_threshold))
            } else {
                d(look_for(ctx, cfg), format!("P(direct)={p:.2} < {}", cfg.value_threshold))
            }
        }
        "heuristic" => {
            if fget(f, "loading_cue") != 0.0 || fget(f, "last_popup") != 0.0 {
                d("wait", "loading cue or popup".into())
            } else if fget(f, "consec_fail") >= 2.0 {
                d(look_for(ctx, cfg), "two or more failures in a row".into())
            } else if (fget(f, "label_dup") != 0.0 || fget(f, "sig_failed") != 0.0) && ctx.target.is_elem() {
                d(if cfg.reviewer && ctx.target.kind != "pixel" { "review" } else { "look_light" }, "duplicate label or this step failed before".into())
            } else if fget(f, "acts_since_obs") >= 3.0 && !nothing_to_see {
                d(look_for(ctx, cfg), "three or more actions since the last read".into())
            } else {
                d("direct", "no heuristic fired".into())
            }
        }
        "random" => {
            let mut n = RANDOM_N.lock().unwrap();
            *n += 1;
            let u = (splitmix(cfg.value_seed.wrapping_mul(0x100000001B3).wrapping_add(*n)) >> 11) as f64 / (1u64 << 53) as f64;
            if u < cfg.value_random_rate && !nothing_to_see {
                d(look_for(ctx, cfg), format!("random intervention (rate {})", cfg.value_random_rate))
            } else {
                d("direct", "random: no intervention".into())
            }
        }
        _ => None,
    }
}

// ---------------------------------------------------------------- the choice

fn argmin(scores: &IndexMap<String, f64>) -> String {
    scores.iter().fold((String::new(), f64::INFINITY), |acc, (k, v)| if *v < acc.1 { (k.clone(), *v) } else { acc }).0
}

fn reason_line(p: f64, f: &Feats, scores: &IndexMap<String, f64>) -> String {
    let mut sorted: Vec<(&String, &f64)> = scores.iter().collect();
    sorted.sort_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal));
    let top: Vec<String> = sorted.iter().take(3).map(|(k, v)| format!("{k}={v:.1}s")).collect();
    let stuck = if fget(f, "consec_fail") >= 1.0 { format!("; {} failed in a row", fget(f, "consec_fail")) } else { String::new() };
    format!("P(direct)={p:.2}{stuck}; expected cost {}", top.join(", "))
}

fn costs_json(cfg: &GateConfig, opts: &[&str]) -> Value {
    Value::Object(opts.iter().map(|o| {
        let (c, src) = cost_of(cfg, o);
        (o.to_string(), json!({"s": round_to(c, 2), "src": src}))
    }).collect())
}

/// The value step: E[cost(o)] = λ·cost(o) + (1 − P_o)·C_fail over the options available here.
pub fn choose(ctx: &ActionCtx, f: &Feats, cfg: &GateConfig, check_available: bool, check_done: bool) -> Decision {
    // v2: a command is not a GUI step. It has no window to observe, so the GUI predictor (which punishes no_obs) does
    // not score it; only the safety rules and the loop breaker (which run elsewhere) can hold it
    if cfg.value_v2 && crate::features::is_command_tool(&ctx.tool) {
        set_last(json!({"policy": cfg.value_policy, "model": "command", "p_direct": Value::Null,
                        "note": "commands are not scored by the GUI predictor (v2)", "chosen": "direct"}));
        return Decision::new("direct", f64::NAN, "command: not scored by the GUI predictor (value model v2)", "command");
    }
    let p = p_direct(f, cfg);
    if let Some(d) = fixed_policy(ctx, f, cfg, p) {
        set_last(json!({"policy": cfg.value_policy, "model": "fixed", "p_direct": round_to(p, 4), "chosen": d.option}));
        return d;
    }
    let pixel = ctx.target.kind == "pixel";
    let keyboard = matches!(ctx.target.kind.as_str(), "keyboard" | "none");
    // a pixel target is re-aimed on a screenshot: a tree-only look cannot fix it (except for a text-only client)
    let mut avail = if pixel && !cfg.text_only { vec!["direct", "look_full"] } else { vec!["direct", "look_light", "look_full"] };
    if cfg.reviewer && !pixel && !keyboard {
        avail.push("review"); // the gate's own independent model checks the target; the acting model is not asked
    }
    if check_available && !check_done {
        avail.push("check");
    }
    if fget(f, "loading_cue") != 0.0 || fget(f, "last_popup") != 0.0 {
        avail.push("wait");
    }
    if cfg.human_channel != "deny" && fget(f, "consec_fail") >= 2.0 {
        avail.push("ask_help");
    }
    let cf = fail_cost(ctx, f, cfg);
    match cfg.treatment.as_ref().filter(|_| cfg.value_model == "treatment") {
        None => legacy_choose(ctx, f, cfg, &avail, p, cf, check_available, check_done),
        Some(m) if !m.has_intervals() => {
            // no bootstrap replicates: every interval is unknown, so the model cannot back a choice; the legacy prior
            // (with its own check rule) decides, exactly as on the default path
            let mut lc = cfg.clone();
            lc.check_model = "legacy".into();
            let d = legacy_choose(ctx, f, &lc, &avail, p, cf, check_available, check_done);
            LAST.with(|l| {
                if let Some(o) = l.borrow_mut().as_object_mut() {
                    o.insert("fallback".into(), json!("legacy_heuristic"));
                    o.insert("learned".into(), json!(false));
                    o.insert("fallback_reason".into(), json!(format!("treatment model {} has {} bootstrap replicates (< {MIN_BOOT}): interval unknown, legacy prior used", m.name, m.boot.len())));
                }
            });
            d
        }
        Some(m) => {
            avail.push("reason"); // one more think of the acting model; only kept when the model has data for it
            if cfg.value_v2 {
                let mut f2 = f.clone();
                let mut extra = serde_json::Map::new();
                for (k, v) in crate::features::v2_feats(&ctx.tool, &ctx.target.kind, ctx.target.the_role()) {
                    let v = *f2.entry(k).or_insert(v);
                    extra.insert(k.to_string(), json!(v));
                }
                let d = treatment_choose(ctx, &f2, cfg, m, &avail, p, cf);
                LAST.with(|l| {
                    if let Some(o) = l.borrow_mut().as_object_mut() {
                        o.insert("v2_features".into(), Value::Object(extra));
                    }
                });
                return d;
            }
            treatment_choose(ctx, f, cfg, m, &avail, p, cf)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn legacy_choose(ctx: &ActionCtx, f: &Feats, cfg: &GateConfig, avail: &[&str], p: f64, cf: f64, check_available: bool, check_done: bool) -> Decision {
    let pixel = ctx.target.kind == "pixel";
    let score = |g: &Feats, o: &str| cfg.lam * cost_of(cfg, o).0 + (1.0 - legacy_po(g, o, pixel, cfg)) * cf;
    let scores: IndexMap<String, f64> = avail.iter().map(|o| (o.to_string(), score(f, o))).collect();
    let mut best = argmin(&scores);
    let mut tier = "model";
    let mut voi = Value::Null;
    if cfg.check_model == "voi" && avail.contains(&"check") {
        let no_check: IndexMap<String, f64> = scores.iter().filter(|(k, _)| k.as_str() != "check").map(|(k, v)| (k.clone(), *v)).collect();
        best = argmin(&no_check);
        // the legacy prior has no probe model: check is not valued (0.10.2 inverted the predictor here); the grey-zone
        // heuristic below applies instead
        voi = json!({"status": CHECK_HEURISTIC_NOTE});
        if check_available && !check_done && ["look_light", "look_full", "review"].contains(&best.as_str()) {
            best = "check".into();
            tier = "grey";
        }
    } else if check_available && !check_done && ["look_light", "look_full", "review"].contains(&best.as_str()) {
        // legacy: before handing the step back to the model (a whole turn), try the cheap gate-side probe once
        best = "check".into();
        tier = "grey";
    }
    set_last(json!({"policy": cfg.value_policy, "model": "legacy_prior", "gains": "hand-set (not estimated)", "p_direct": round_to(p, 4),
        "fail_cost_s": round_to(cf, 2), "costs": costs_json(cfg, avail), "check_model": cfg.check_model, "voi": voi, "chosen": best}));
    Decision::new(&best, p, reason_line(p, f, &scores), tier).with_scores(&scores)
}

/// The learned choice (GUI_GATE_VALUE_MODEL=treatment|auto|v2 with weights). v0.10.3:
/// * an option is considered only where the state supports BOTH direct and it (`covered_at`: saved propensity of the
///   logged policy, e_o(x) >= clip and e_direct(x) >= clip, plus min_rows effective rows); elsewhere it has "no
///   estimate basis" and is listed as such;
/// * every option is compared on the conservative lower bound of its local net gain over direct,
///   G_o = C_recover·Δp_o − λ·Δt_o − γ·ΔH_o − η·1[o≠direct] (research::local_gain), the `level` percentile interval
///   over the bootstrap replicates; the option with the largest lower bound is taken when that bound is > 0, else
///   direct. The same rule holds for check (valued as information only with a probe model; otherwise it has no
///   estimate basis);
/// * when no option but direct is covered here, the model has nothing to compare: the legacy prior decides and the log
///   says fallback = "legacy_heuristic", learned = false.
fn treatment_choose(ctx: &ActionCtx, f: &Feats, cfg: &GateConfig, m: &TreatmentModel, avail: &[&str], p: f64, cf: f64) -> Decision {
    let prices = crate::research::Prices { lam: cfg.lam, gamma: cfg.gamma, eta: cfg.eta };
    let mut no_basis = serde_json::Map::new();
    let mut covered: Vec<&str> = vec![];
    for o in avail.iter().filter(|o| **o != "direct") {
        let r = if *o == "check" && m.probe.is_none() { Err(CHECK_HEURISTIC_NOTE.to_string()) } else { m.covered_at(f, o) };
        match r {
            Ok(()) => covered.push(o),
            Err(why) => { no_basis.insert(o.to_string(), json!(why)); }
        }
    }
    let uncovered: Vec<&str> = no_basis.keys().map(|k| avail.iter().find(|o| **o == k.as_str()).copied().unwrap_or("?")).collect();
    if covered.is_empty() {
        // no estimate basis for any option here: the learned model cannot back a choice
        let mut lc = cfg.clone();
        lc.check_model = "legacy".into();
        let d = legacy_choose(ctx, f, &lc, avail, p, cf, avail.contains(&"check"), !avail.contains(&"check"));
        LAST.with(|l| {
            if let Some(o) = l.borrow_mut().as_object_mut() {
                o.insert("fallback".into(), json!("legacy_heuristic"));
                o.insert("learned".into(), json!(false));
                o.insert("fallback_reason".into(), json!(format!("treatment model {}: no estimate basis at this state for any option but direct", m.name)));
                o.insert("no_estimate_basis".into(), Value::Object(no_basis.clone()));
            }
        });
        return d;
    }
    let al = (1.0 - m.level) / 2.0;
    // G_o under one coefficient set
    let g_of = |c: &Coefs, g: &Feats, o: &str| -> f64 {
        let (dt, dh, _) = cost_split(cfg, o);
        let dp = sigmoid(c.logit(g, o)) - sigmoid(c.logit(g, "direct"));
        crate::research::local_gain(o, cf, dp, dt, dh, prices)
    };
    let interval = |xs: Vec<f64>, mean: f64| Interval { mean, lo: quantile(&xs, al), hi: quantile(&xs, 1.0 - al) };
    let mut gains: IndexMap<String, Interval> = IndexMap::new();
    let mut voi = Value::Null;
    for o in &covered {
        if *o == "check" {
            // value of information with the probe model, per replicate: best G after seeing the probe minus best G now
            let acts: Vec<&str> = covered.iter().filter(|x| **x != "check").copied().collect();
            let probe = m.probe.as_ref();
            let voi_g = |c: &Coefs| -> f64 {
                let best = |g: &Feats| acts.iter().map(|a| g_of(c, g, a)).fold(0.0_f64, f64::max); // direct = 0
                let pp = check_pass_prob(f, probe).unwrap_or(0.5);
                let (fp, ff) = probe_states(f);
                let (dt, dh, _) = cost_split(cfg, "check");
                pp * best(&fp) + (1.0 - pp) * best(&ff) - best(f) - prices.lam * dt - prices.gamma * dh - prices.eta
            };
            let iv = interval(m.boot.iter().map(voi_g).collect(), voi_g(&m.coef));
            voi = json!({"gain_s": iv.json(), "p_pass": check_pass_prob(f, probe).map(|x| round_to(x, 3)), "note": "includes a repair when the probe re-binds the target"});
            gains.insert("check".into(), iv);
        } else {
            let iv = interval(m.boot.iter().map(|c| g_of(c, f, o)).collect(), g_of(&m.coef, f, o));
            gains.insert(o.to_string(), iv);
        }
    }
    // the same conservative rule for every option: the largest lower bound, only when it is above zero
    let (mut best, mut best_lb) = ("direct".to_string(), 0.0_f64);
    for (o, iv) in &gains {
        if iv.lo > best_lb {
            best = o.clone();
            best_lb = iv.lo;
        }
    }
    let best_mean = gains.iter().fold(("direct".to_string(), 0.0_f64), |a, (o, iv)| if iv.mean > a.1 { (o.clone(), iv.mean) } else { a }).0;
    let tier = if best == "direct" && best_mean != "direct" { "model_lb" } else if best == "check" { "voi" } else { "model" };
    // expected costs for the decision's scores (cost(o) − G_o-style view kept for the log and the reason line)
    let scores: IndexMap<String, f64> = std::iter::once("direct").chain(covered.iter().copied()).filter(|o| *o != "check")
        .map(|o| (o.to_string(), cfg.lam * cost_of(cfg, o).0 + (1.0 - m.q(f, o)) * cf)).collect();
    let qd = m.q(f, "direct");
    let deltas: serde_json::Map<String, Value> = covered.iter().filter(|o| **o != "check").map(|o| (o.to_string(), m.delta(f, o).json())).collect();
    set_last(json!({"policy": cfg.value_policy, "model": m.name, "kind": m.kind, "learned": true, "p_direct": round_to(p, 4), "q_direct": round_to(qd, 4),
        "fail_cost_s": round_to(cf, 2), "fail_cost_kind": "prior (2·turn_cost + side_cost, by severity and stuck factor)",
        "delta": deltas, "level": m.level, "gain": Value::Object(gains.iter().map(|(o, iv)| (o.clone(), iv.json())).collect()),
        "rule": "max lower bound of G_o over direct, > 0", "point_best": best_mean, "prices": {"lam": prices.lam, "gamma": prices.gamma, "eta": prices.eta},
        "uncovered": uncovered, "no_estimate_basis": Value::Object(no_basis), "costs": costs_json(cfg, avail), "voi": voi, "chosen": best}));
    let mut d = Decision::new(&best, p, format!("{}; lower bound of the net gain {best_lb:.1}s", reason_line(qd, f, &scores)), tier).with_scores(&scores);
    if best == "reason" {
        d.reason = format!("{}; {REASON_TEXT}", d.reason);
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::{Target, FEATURE_NAMES};

    fn ctx(tool: &str, kind: &str, label: &str) -> ActionCtx {
        ActionCtx { tool: tool.into(), args: json!({}), target: Target { kind: kind.into(), label: label.into(), ..Default::default() },
                    signature: format!("{tool}|{label}"), ..Default::default() }
    }
    fn feats() -> Feats {
        let mut f: Feats = FEATURE_NAMES.iter().map(|k| (*k, 0.0)).collect();
        f["bias"] = 1.0;
        f["is_token"] = 1.0;
        f
    }
    fn w(pairs: &[(&str, f64)]) -> IndexMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }
    /// q_direct = sigmoid(0.4) ~ 0.6; look_light hurts (-1.5 main effect), look_full helps when label_dup
    fn model() -> TreatmentModel {
        let base = w(&[("bias", 0.4), ("label_dup", -0.8)]);
        let mut inter = IndexMap::new();
        inter.insert("look_light".to_string(), w(&[("bias", -1.5)]));
        inter.insert("look_full".to_string(), w(&[("bias", -0.2), ("label_dup", 2.5)]));
        let n: IndexMap<String, usize> = ["direct", "look_light", "look_full", "check", "review", "wait", "reason"].iter().map(|o| (o.to_string(), 100)).collect();
        TreatmentModel { coef: Coefs { base, inter }, n_by_option: n, propensity: Some(uniform_prop()), ..Default::default() }
    }
    /// a logged policy that could take every option everywhere (1/7 each >= clip 0.05)
    fn uniform_prop() -> PropensityModel {
        let opts: Vec<String> = ["direct", "look_light", "look_full", "check", "review", "wait", "reason"].iter().map(|o| o.to_string()).collect();
        PropensityModel { w: opts.iter().map(|_| IndexMap::new()).collect(), opts, clip: 0.05 }
    }
    fn cfg_with(m: TreatmentModel) -> GateConfig {
        let mut c = GateConfig::default();
        c.treatment = Some(std::sync::Arc::new(m));
        c.value_model = "treatment".into();
        c.check_model = "voi".into();
        c
    }

    /// 20 identical replicates: a confident interval (the model's own point estimate everywhere)
    fn with_boot(mut m: TreatmentModel) -> TreatmentModel {
        for _ in 0..20 { let c = m.coef.clone(); m.boot.push(c); }
        m
    }

    #[test]
    fn no_bootstrap_means_an_unknown_interval_not_a_zero_width_one() {
        let mut f = feats();
        f["label_dup"] = 1.0;
        let m = model();
        assert!(m.boot.is_empty());
        let iv = m.delta(&f, "look_full");
        assert!(iv.lo == f64::NEG_INFINITY && iv.hi == f64::INFINITY, "no replicates must not look certain: {iv:?}");
        assert!(iv.crosses_zero());
        assert!(iv.json()[0].is_null() && iv.json()[2].is_null(), "{}", iv.json());
        let nb = m.net_benefit(&f, "look_full", "direct", 19.0, 9.0);
        assert!(nb.lo == f64::NEG_INFINITY && nb.hi == f64::INFINITY);
        // too few replicates for a 90 % percentile interval are no better than none
        let mut few = model();
        for _ in 0..3 { let c = few.coef.clone(); few.boot.push(c); }
        assert!(few.delta(&f, "look_full").hi == f64::INFINITY);
        // with enough replicates the interval is finite
        let ok = with_boot(model()).delta(&f, "look_full");
        assert!(ok.lo.is_finite() && ok.hi.is_finite());
    }

    #[test]
    fn a_treatment_model_without_intervals_is_not_used_the_legacy_choice_runs() {
        let mut f = feats();
        f["label_dup"] = 1.0;
        let c = ctx("click", "token", "Save");
        let legacy = choose(&c, &f, &GateConfig::default(), false, false);
        let _ = take_last();
        let d = choose(&c, &f, &cfg_with(model()), false, false);
        assert_eq!(d.option, legacy.option, "no bootstrap -> fall back to the legacy prior ({})", d.reason);
        assert_eq!(d.scores, legacy.scores);
        let log = take_last();
        assert_eq!(log["model"], "legacy_prior");
        assert_eq!(log["fallback"], "legacy_heuristic");
        assert_eq!(log["learned"], false);
        assert!(log["fallback_reason"].as_str().unwrap_or("").contains("bootstrap"), "{log}");
    }

    #[test]
    fn treatment_weights_alone_do_not_switch_the_default() {
        let mut cfg = GateConfig::default();
        assert_eq!(cfg.value_model_req, "legacy", "default request is the legacy prior");
        let p = std::env::temp_dir().join(format!("gg-tm2-{}.json", rand_hex(4)));
        std::fs::write(&p, json!({"name": "w", "weights": {"bias": 1.0}, "treatment": with_boot(model()).to_json()}).to_string()).unwrap();
        cfg.load_weights(p.to_str().unwrap()).unwrap();
        assert_eq!(cfg.value_model, "legacy", "loading treatment weights must not change the default decision path");
        cfg.value_model_req = "treatment".into();
        cfg.resolve_value_model();
        assert_eq!(cfg.value_model, "treatment");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn v2_commands_are_not_scored_by_the_gui_predictor() {
        let mut cfg = GateConfig::default();
        cfg.value_model_req = "v2".into();
        cfg.resolve_value_model();
        let mut f = feats();
        f["is_token"] = 0.0;
        f["no_obs"] = 1.0; // a command never has a window observation: v1 punished it for that
        let c = ctx("run_command", "command", "grep x f");
        let d = choose(&c, &f, &cfg, false, false);
        assert_eq!(d.option, "direct");
        assert_eq!(d.tier, "command");
        assert!(d.p_direct.is_nan(), "no GUI probability for a command: {}", d.p_direct);
        let log = take_last();
        assert_eq!(log["model"], "command");
        assert!(log["p_direct"].is_null());
        // the default path still scores it (unchanged legacy behaviour)
        let d0 = choose(&c, &f, &GateConfig::default(), false, false);
        assert!(d0.p_direct < 0.5 && d0.tier != "command");
    }

    #[test]
    fn v2_uses_its_extra_features_when_the_model_has_them() {
        // a v2 model: scrolling at a pixel fails more often; it must see tool_scroll at decision time
        let mut m = with_boot(model());
        m.coef.base.insert("tool_scroll".into(), -3.0);
        let mut cfg = cfg_with(m);
        cfg.value_model_req = "v2".into();
        cfg.resolve_value_model();
        assert_eq!(cfg.value_model, "treatment");
        let f = feats();
        let _ = choose(&ctx("scroll", "token", "list"), &f, &cfg, false, false);
        let log = take_last();
        assert_eq!(log["v2_features"]["tool_scroll"], 1.0, "{log}");
        assert!(log["q_direct"].as_f64().unwrap() < 0.1, "{log}");
    }

    #[test]
    fn negative_gain_is_kept_and_direct_wins() {
        let m = model();
        let f = feats();
        let d = m.delta(&f, "look_light");
        assert!(d.mean < -0.2, "negative incremental value must not be clamped: {d:?}");
        let c = choose(&ctx("click", "token", "Bold"), &f, &cfg_with(with_boot(m)), false, false);
        assert_eq!(c.option, "direct", "{} {:?}", c.reason, c.scores);
        // the legacy prior would never let a look be worse than acting: P_o >= P_direct
        let cfg = GateConfig::default();
        assert!(legacy_po(&f, "look_light", false, &cfg) >= p_direct(&f, &cfg));
    }

    #[test]
    fn positive_interaction_picks_the_option() {
        let mut f = feats();
        f["label_dup"] = 1.0;
        let m = model();
        assert!(m.delta(&f, "look_full").mean > 0.3);
        let c = choose(&ctx("click", "token", "Save"), &f, &cfg_with(with_boot(m)), false, false);
        assert_eq!(c.option, "look_full", "{} {:?}", c.reason, c.scores);
        let log = take_last();
        assert_eq!(log["kind"], "treatment_v1");
        assert!(log["delta"]["look_light"][1].as_f64().unwrap() < 0.0);
        assert!(take_last().is_null(), "the log is taken once");
    }

    #[test]
    fn interval_crossing_zero_prefers_the_cheaper_option_on_low_risk_steps() {
        let mut f = feats();
        f["label_dup"] = 1.0;
        let mut m = model();
        // replicates: half say look_full helps a lot, half say it hurts -> the net-benefit interval crosses zero
        for i in 0..20 {
            let mut c = m.coef.clone();
            c.inter.get_mut("look_full").unwrap().insert("label_dup".into(), if i % 2 == 0 { 4.0 } else { -2.0 });
            m.boot.push(c);
        }
        let iv = m.delta(&f, "look_full");
        assert!(iv.crosses_zero(), "{iv:?}");
        let cfg = cfg_with(m.clone());
        let c = choose(&ctx("click", "token", "Save"), &f, &cfg, false, false);
        assert_eq!(c.option, "direct", "{}", c.reason);
        assert_eq!(c.tier, "model_lb");
        let log = take_last();
        assert_eq!(log["point_best"], "look_full", "the point estimate alone would have picked the look: {log}");
        assert!(log["gain"]["look_full"][0].as_f64().unwrap() <= 0.0, "{log}");
        // v0.10.3: the same conservative rule for every option and every step (a risky step's safety asks are rules,
        // decided before the value model; its higher fail cost is already in C_recover)
        let mut risky = ctx("click", "token", "Save");
        risky.risk_level = Some("medium".into());
        assert_eq!(choose(&risky, &f, &cfg, false, false).option, "direct");
        // with confident replicates the option stays
        let mut m2 = model();
        for _ in 0..20 { m2.boot.push(m2.coef.clone()); }
        assert_eq!(choose(&ctx("click", "token", "Save"), &f, &cfg_with(m2), false, false).option, "look_full");
    }

    #[test]
    fn uncovered_options_are_never_chosen() {
        let mut f = feats();
        f["label_dup"] = 1.0;
        let mut m = model();
        m.n_by_option.insert("look_full".into(), 3);
        let c = choose(&ctx("click", "token", "Save"), &f, &cfg_with(with_boot(m)), false, false);
        assert_ne!(c.option, "look_full");
        assert!(take_last()["uncovered"].as_array().unwrap().iter().any(|x| x == "look_full"));
    }

    #[test]
    fn measured_costs_replace_the_prior_when_there_are_enough() {
        let mut f = feats();
        f["label_dup"] = 1.0;
        let mut cfg = cfg_with(with_boot(model()));
        assert_eq!(cost_of(&cfg, "look_full"), (9.0, "prior"));
        let mut t = CostTable::default();
        t.measured.insert("look_full".into(), MeasuredCost { extra_s: 60.0, tokens: 2000.0, person_s: 0.0, n: 50 });
        t.measured.insert("look_light".into(), MeasuredCost { extra_s: 1.0, tokens: 0.0, person_s: 0.0, n: 2 }); // too few
        t.token_s_per_k = 2.0;
        cfg.cost_table = Some(t);
        assert_eq!(cost_of(&cfg, "look_full"), (64.0, "measured"));
        assert_eq!(cost_of(&cfg, "look_light"), (7.0, "prior"));
        // a measured 64 s look is no longer worth it here (the prior 9 s one was)
        let c = choose(&ctx("click", "token", "Save"), &f, &cfg, false, false);
        assert_ne!(c.option, "look_full", "{:?}", c.scores);
        let log = take_last();
        assert_eq!(log["costs"]["look_full"]["src"], "measured");
        assert_eq!(log["costs"]["look_light"]["src"], "prior");
        // legacy mode uses measured costs too
        let mut lg = GateConfig::default();
        lg.cost_table = cfg.cost_table.clone();
        let _ = choose(&ctx("click", "token", "Save"), &f, &lg, false, false);
        assert_eq!(take_last()["costs"]["look_full"]["s"], 64.0);
    }

    #[test]
    fn check_is_valued_as_information() {
        // the probe result flips the choice: pass -> act now, fail -> full look. Probing first pays only when the
        // flip is likely enough and the probe cheap enough
        let mut base = w(&[("bias", 0.0), ("check_pass", 3.0), ("check_fail", -3.0)]);
        let mut inter = IndexMap::new();
        inter.insert("look_full".to_string(), w(&[("bias", 1.0), ("check_fail", 4.0)]));
        let n: IndexMap<String, usize> = ["direct", "look_full", "check"].iter().map(|o| (o.to_string(), 100)).collect();
        let m = TreatmentModel { coef: Coefs { base: base.clone(), inter: inter.clone() }, n_by_option: n.clone(), propensity: Some(uniform_prop()),
                                 probe: Some(w(&[("bias", 0.0)])), ..Default::default() };
        let cfg = cfg_with(with_boot(m.clone()));
        let f = feats();
        let score = |g: &Feats, o: &str| cfg.lam * cost_of(&cfg, o).0 + (1.0 - m.q(g, o)) * 19.0;
        let v = check_voi(&f, &["direct", "look_full", "check"], &score, 0.5, 0.2);
        assert_eq!(v.choice_pass, "direct");
        assert_eq!(v.choice_fail, "look_full");
        assert!(v.gain() > 0.0, "{v:?}");
        let c = choose(&ctx("click", "token", "Bold"), &f, &cfg, true, false);
        assert_eq!(c.option, "check", "{}", c.reason);
        assert_eq!(c.tier, "voi");
        // when the result cannot change the choice, a probe is pure cost
        base.insert("check_pass".into(), 0.0);
        base.insert("check_fail".into(), 0.0);
        inter.get_mut("look_full").unwrap().insert("check_fail".into(), 0.0);
        let m2 = TreatmentModel { coef: Coefs { base, inter }, n_by_option: n.clone(), propensity: Some(uniform_prop()), probe: Some(w(&[("bias", 0.0)])), ..Default::default() };
        let c2 = choose(&ctx("click", "token", "Bold"), &f, &cfg_with(with_boot(m2)), true, false);
        assert_ne!(c2.option, "check");
        assert!(take_last()["voi"]["gain_s"][1].as_f64().unwrap() <= 0.0);
        // without a probe model there is no P(pass) estimate (0.10.2 inverted the predictor) and check is not valued
        assert!(check_pass_prob(&f, None).is_none());
        assert_eq!(check_pass_prob(&f, Some(&w(&[("bias", 0.0)]))), Some(0.5));
        let mut m3 = m.clone();
        m3.probe = None;
        let c3 = choose(&ctx("click", "token", "Bold"), &f, &cfg_with(with_boot(m3)), true, false);
        assert_ne!(c3.option, "check");
        let log = take_last();
        assert!(log["no_estimate_basis"]["check"].as_str().unwrap().contains("heuristic"), "{log}");
    }

    #[test]
    fn legacy_is_unchanged_by_default() {
        let cfg = GateConfig::default();
        assert_eq!(cfg.value_model, "legacy");
        assert_eq!(cfg.check_model, "legacy");
        let c = choose(&ctx("click", "token", "Bold"), &feats(), &cfg, true, false);
        assert_eq!(c.option, "direct");
        let log = take_last();
        assert_eq!(log["model"], "legacy_prior");
        assert_eq!(log["gains"], "hand-set (not estimated)");
    }

    #[test]
    fn fixed_policies_change_only_the_value_choice() {
        let mut f = feats();
        f["acts_since_obs"] = 3.0;
        let mut cfg = GateConfig::default();
        let c = ctx("click", "token", "Bold");
        for (pol, want) in [("direct", "direct"), ("observe_light", "look_light"), ("observe_full", "look_full"), ("heuristic", "look_light")] {
            cfg.value_policy = pol.into();
            let d = choose(&c, &f, &cfg, true, false);
            assert_eq!(d.option, want, "{pol}");
            assert_eq!(d.tier, "policy");
        }
        cfg.value_policy = "threshold".into();
        cfg.value_threshold = 0.99;
        assert_eq!(choose(&c, &f, &cfg, true, false).option, "look_light");
        cfg.value_threshold = 0.01;
        assert_eq!(choose(&c, &f, &cfg, true, false).option, "direct");
        cfg.value_policy = "extra_reason".into();
        let mut c2 = c.clone();
        c2.signature = "click|unique-extra-reason-test".into();
        assert_eq!(choose(&c2, &f, &cfg, true, false).option, "reason");
        assert_eq!(choose(&c2, &f, &cfg, true, false).option, "direct");
        cfg.value_policy = "random".into();
        cfg.value_random_rate = 1.0;
        assert_eq!(choose(&c, &f, &cfg, true, false).option, "look_light");
        cfg.value_random_rate = 0.0;
        assert_eq!(choose(&c, &f, &cfg, true, false).option, "direct");
    }

    #[test]
    fn treatment_is_used_only_when_requested_and_its_weights_are_loaded() {
        let mut cfg = GateConfig::default();
        assert_eq!(cfg.value_model, "legacy");
        let p = std::env::temp_dir().join(format!("gg-tm-{}.json", rand_hex(4)));
        std::fs::write(&p, json!({"name": "w", "weights": {"bias": 1.0}, "treatment": with_boot(model()).to_json()}).to_string()).unwrap();
        cfg.value_model_req = "auto".into(); // the 0.10.1 default, now opt-in
        cfg.load_weights(p.to_str().unwrap()).unwrap();
        assert_eq!(cfg.value_model, "treatment");
        assert_eq!(cfg.check_model, "voi");
        let _ = choose(&ctx("click", "token", "Bold"), &feats(), &cfg, false, false);
        assert_eq!(take_last()["kind"], "treatment_v1");
        cfg.value_model_req = "legacy".into();
        cfg.resolve_value_model();
        assert_eq!(cfg.value_model, "legacy");
        let mut none = GateConfig::default();
        none.value_model_req = "treatment".into();
        none.resolve_value_model();
        assert_eq!(none.value_model, "legacy", "no weights: the legacy prior stays active");
        assert!(TreatmentModel::load(p.to_str().unwrap()).is_ok());
        let _ = std::fs::remove_file(p);
    }

    /// The default path (no switch set) must produce exactly the decisions and value logs of c3893bc:
    /// a fixed grid of contexts x states x configs, compared with src/testdata/value_default_golden.json
    /// (written once from c3893bc with GG_WRITE_GOLDEN=1).
    #[test]
    fn default_path_output_is_unchanged() {
        let ctxs = [("click", "token", "Bold"), ("click", "token", "Save"), ("click", "pixel", ""), ("scroll", "pixel", ""),
                    ("press_key", "keyboard", ""), ("type_text", "token", "Name"), ("run_command", "command", "ls /tmp")];
        let states: Vec<Vec<(&str, f64)>> = vec![
            vec![],
            vec![("label_dup", 1.0)],
            vec![("acts_since_obs", 3.0), ("log_obs_age", 2.5)],
            vec![("loading_cue", 1.0)],
            vec![("last_popup", 1.0), ("window_changed", 1.0)],
            vec![("consec_fail", 2.0), ("sess_fail", -0.3), ("sig_failed", 1.0), ("repeat", 1.0)],
            vec![("no_obs", 1.0)],
            vec![("check_pass", 1.0), ("role_menu", 1.0)],
            vec![("elem_unknown", 1.0), ("small_target", 1.0), ("label_empty", 1.0)],
        ];
        let mut out = vec![];
        for (ci, (tool, kind, label)) in ctxs.iter().enumerate() {
            for (si, st) in states.iter().enumerate() {
                for variant in 0..3 {
                    let mut cfg = GateConfig::default();
                    if variant == 1 { cfg.reviewer = true; }
                    if variant == 2 { cfg.human_channel = "deny".into(); }
                    let mut f = feats();
                    if *kind != "token" { f["is_token"] = 0.0; }
                    if *kind == "pixel" { f["is_pixel"] = 1.0; }
                    if *kind == "keyboard" { f["is_keyboard"] = 1.0; }
                    for (k, v) in st { f[*k] = *v; }
                    let c = ctx(tool, kind, label);
                    let _ = take_last();
                    let d = choose(&c, &f, &cfg, *kind == "token", false);
                    let hard = crate::decide::hard_rules(&c, &f, &cfg, false, false).map(|h| h.option);
                    out.push(json!({"c": ci, "s": si, "v": variant, "option": d.option, "tier": d.tier, "p": round_to(d.p_direct, 9),
                        "reason": d.reason, "scores": d.scores.iter().map(|(k, v)| (k.clone(), json!(round_to(*v, 9)))).collect::<serde_json::Map<_, _>>(),
                        "log": take_last(), "hard": hard, "fail_cost": round_to(fail_cost(&c, &f, &cfg), 9)}));
                }
            }
        }
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/testdata/value_default_golden.json");
        let got = Value::Array(out);
        if std::env::var("GG_WRITE_GOLDEN").is_ok() {
            std::fs::write(path, serde_json::to_string_pretty(&got).unwrap()).unwrap();
        }
        let want: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("golden file")).unwrap();
        let (g, w) = (got.as_array().unwrap(), want.as_array().unwrap());
        assert_eq!(g.len(), w.len());
        for (a, b) in g.iter().zip(w) {
            assert_eq!(a, b, "default-path decision changed");
        }
    }

    #[test]
    fn coverage_is_checked_per_state_and_no_basis_falls_back_to_the_legacy_heuristic() {
        let mut f = feats();
        f["label_dup"] = 1.0;
        // the logged policy took look_full only when label_dup = 0: at this state (label_dup = 1) there is no overlap
        let mut m = with_boot(model());
        let mut p = uniform_prop();
        let i = p.opts.iter().position(|o| o == "look_full").unwrap();
        p.w[i].insert("label_dup".into(), -12.0);
        m.propensity = Some(p);
        assert!(m.covered("look_full"), "enough rows overall");
        assert!(m.covered_at(&f, "look_full").is_err(), "but no support at this state");
        let mut f0 = feats();
        f0["label_dup"] = 0.0;
        assert!(m.covered_at(&f0, "look_full").is_ok());
        let c = choose(&ctx("click", "token", "Save"), &f, &cfg_with(m.clone()), false, false);
        assert_ne!(c.option, "look_full", "{}", c.reason);
        let log = take_last();
        assert!(log["no_estimate_basis"]["look_full"].as_str().unwrap().contains("no overlap"), "{log}");
        // no propensity saved: overlap per state unknown -> nothing but direct is covered -> legacy fallback, not learned
        let mut m2 = with_boot(model());
        m2.propensity = None;
        let legacy = choose(&ctx("click", "token", "Save"), &f, &GateConfig::default(), false, false);
        let _ = take_last();
        let d = choose(&ctx("click", "token", "Save"), &f, &cfg_with(m2), false, false);
        assert_eq!(d.option, legacy.option);
        let log = take_last();
        assert_eq!(log["fallback"], "legacy_heuristic");
        assert_eq!(log["learned"], false);
        assert_eq!(log["model"], "legacy_prior");
        assert!(log["no_estimate_basis"]["look_full"].as_str().unwrap().contains("propensity"), "{log}");
    }

    #[test]
    fn options_are_compared_on_the_lower_bound_not_the_point_estimate() {
        let mut f = feats();
        f["label_dup"] = 1.0;
        // look_full: large mean gain but very uncertain; review: smaller mean, tight -> review wins on the lower bound
        let mut m = model();
        m.coef.inter.insert("review".to_string(), w(&[("label_dup", 1.2)]));
        for i in 0..20 {
            let mut c = m.coef.clone();
            c.inter.get_mut("look_full").unwrap().insert("label_dup".into(), if i % 2 == 0 { 6.0 } else { -1.0 });
            m.boot.push(c);
        }
        let mut cfg = cfg_with(m);
        cfg.reviewer = true;
        let d = choose(&ctx("click", "token", "Save"), &f, &cfg, false, false);
        let log = take_last();
        assert_eq!(log["point_best"], "look_full", "{log}");
        assert_eq!(d.option, "review", "{log}");
        assert_eq!(log["rule"], "max lower bound of G_o over direct, > 0");
        // eta (intervention-budget price) moves the choice back to direct without touching lambda
        cfg.eta = 50.0;
        assert_eq!(choose(&ctx("click", "token", "Save"), &f, &cfg, false, false).option, "direct");
        assert_eq!(take_last()["prices"]["lam"], 1.0);
    }

    #[test]
    fn model_json_roundtrip() {
        let mut m = model();
        m.boot.push(m.coef.clone());
        m.probe = Some(w(&[("bias", 1.0)]));
        let back = TreatmentModel::from_json(&m.to_json()).unwrap();
        assert_eq!(back.propensity.as_ref().map(|p| p.opts.len()), Some(7));
        assert_eq!(back.coef, m.coef);
        assert_eq!(back.boot.len(), 1);
        assert_eq!(back.n_by_option, m.n_by_option);
        assert!(TreatmentModel::from_json(&json!({"kind": "x"})).is_err());
        assert!((quantile(&[3.0, 1.0, 2.0], 0.5) - 2.0).abs() < 1e-12);
    }
}
