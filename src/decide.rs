//! The value gate: hard rules, then a calibrated light predictor that compares the expected benefit of
//! extra observation / reasoning against its cost, with a cheap probe in the grey zone.
//!
//! Options: direct, check (cheap tree probe), heal (re-bind a stale token), wait, look_light (re-read the
//! tree), look_full (re-read with a screenshot), reason (double-check the target), risk_review (main model
//! re-thinks a risky step), model_review, ask_human, handoff (a person must act), deny (deny-and-continue).
use crate::features::{fget, ActionCtx, Feats, FEATURE_NAMES};
use crate::util::*;
use indexmap::IndexMap;
use serde_json::Value;

pub fn prior_weights() -> IndexMap<String, f64> {
    [("bias", 2.6), ("is_pixel", -0.9), ("is_token", 0.4), ("is_name", 0.3), ("is_keyboard", 0.2), ("no_obs", -2.5),
     ("token_stale", -3.2), ("log_obs_age", -0.15), ("acts_since_obs", -0.55), ("obs_had_shot", 0.1), ("last_err", -0.9),
     ("last_unverif", -0.3), ("last_popup", -1.2), ("repeat", -0.8), ("edge_close", -1.0), ("small_target", -0.6),
     ("label_empty", -0.4), ("label_dup", -0.9), ("loading_cue", -1.3), ("window_changed", -1.2), ("window_added", -1.5),
     ("elem_unknown", -0.7), ("check_pass", 1.6), ("check_fail", -3.5)]
        .iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

fn prior_gains() -> IndexMap<String, f64> {
    [("check", 0.45), ("heal", 0.65), ("wait", 0.35), ("look_light", 0.75), ("look_full", 0.92), ("reason", 0.55)]
        .iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

fn pixel_gain(o: &str) -> Option<f64> {
    match o {
        "look_light" => Some(0.35),
        "look_full" => Some(0.92),
        "check" => Some(0.0),
        _ => None,
    }
}

fn default_costs() -> IndexMap<String, f64> {
    [("direct", 0.0), ("check", 0.05), ("heal", 0.15), ("wait", 0.8), ("look_light", 3.0), ("look_full", 5.5),
     ("reason", 4.0), ("ask_human", 30.0)].iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

#[derive(Clone, Debug)]
pub struct GateConfig {
    pub policy: String,
    pub lam: f64,
    pub fail_cost: f64,
    pub grey_lo: f64,
    pub grey_hi: f64,
    pub risk_confirm: bool,
    pub risk_mode: String,
    pub decider: String,
    pub collect: bool,
    pub human_channel: String,
    pub human_timeout: f64,
    pub toolset: String,
    pub auto_observe: bool,
    pub delivery: String,
    pub task: String,
    pub batch: String,
    pub deny_consecutive: u32,
    pub deny_total: u32,
    pub costs: IndexMap<String, f64>,
    pub weights: IndexMap<String, f64>,
    pub gains: IndexMap<String, f64>,
    pub calib: Option<(f64, f64)>,
    pub model_name: String,
}

impl Default for GateConfig {
    fn default() -> Self {
        GateConfig {
            policy: "gate".into(), lam: 1.0, fail_cost: 12.0, grey_lo: 0.55, grey_hi: 0.9, risk_confirm: true,
            risk_mode: "auto".into(), decider: "formula".into(), collect: false, human_channel: "auto".into(),
            human_timeout: 120.0, toolset: "core".into(), auto_observe: true, delivery: "auto".into(), task: String::new(),
            batch: "auto".into(), deny_consecutive: 3, deny_total: 8, costs: default_costs(), weights: prior_weights(),
            gains: prior_gains(), calib: None, model_name: "prior_v0".into(),
        }
    }
}

fn env_flag(k: &str, default: bool) -> bool {
    match std::env::var(k) {
        Ok(v) => !matches!(v.to_lowercase().as_str(), "0" | "false" | "no" | ""),
        Err(_) => default,
    }
}

impl GateConfig {
    pub fn from_env() -> Self {
        let mut c = GateConfig::default();
        if let Some(v) = env("GUI_GATE_POLICY") { c.policy = v; }
        if let Some(v) = env("GUI_GATE_LAMBDA").and_then(|v| v.parse().ok()) { c.lam = v; }
        c.collect = matches!(std::env::var("GUI_GATE_COLLECT").unwrap_or_default().as_str(), "1" | "true" | "yes");
        c.risk_confirm = env_flag("GUI_GATE_RISK_CONFIRM", true);
        if let Some(v) = env("GUI_GATE_RISK_MODE") { c.risk_mode = v; }
        if let Some(v) = env("GUI_GATE_DECIDER") { c.decider = v; }
        if let Some(v) = env("GUI_GATE_HUMAN") { c.human_channel = v; }
        if let Some(v) = env("GUI_GATE_TOOLSET") { c.toolset = v; }
        c.auto_observe = env_flag("GUI_GATE_AUTO_OBSERVE", true);
        if let Some(v) = env("GUI_GATE_DELIVERY") { c.delivery = v; }
        if let Some(v) = env("GUI_GATE_TASK") { c.task = v; }
        if c.task.is_empty() {
            if let Some(tf) = env("GUI_GATE_TASK_FILE") {
                if let Ok(t) = std::fs::read_to_string(tf) { c.task = t.trim().to_string(); }
            }
        }
        if let Some(v) = env("GUI_GATE_BATCH") { c.batch = v; }
        if let Some(w) = env("GUI_GATE_WEIGHTS") {
            let _ = c.load_weights(&w);
        }
        c
    }

    pub fn load_weights(&mut self, path: &str) -> Result<(), String> {
        let d: Value = serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        for (field, map) in [("weights", &mut self.weights), ("gains", &mut self.gains)] {
            if let Some(o) = d.get(field).and_then(|x| x.as_object()) {
                for (k, v) in o {
                    if let Some(f) = v.as_f64() { map.insert(k.clone(), f); }
                }
            }
        }
        if let Some(c) = d.get("calib").filter(|c| c.is_object()) {
            self.calib = Some((as_f64(c.get("a")).unwrap_or(1.0), as_f64(c.get("b")).unwrap_or(0.0)));
        }
        self.model_name = d.get("name").and_then(|n| n.as_str()).map(String::from)
            .unwrap_or_else(|| crate::sidefx::basename(path));
        Ok(())
    }
}

pub fn sigmoid(z: f64) -> f64 {
    if z >= 0.0 { 1.0 / (1.0 + (-z).exp()) } else { let e = z.exp(); e / (1.0 + e) }
}

pub fn p_direct(f: &Feats, cfg: &GateConfig) -> f64 {
    let mut z: f64 = FEATURE_NAMES.iter().map(|k| cfg.weights.get(*k).copied().unwrap_or(0.0) * fget(f, k)).sum();
    if let Some((a, b)) = cfg.calib {
        z = a * z + b;
    }
    sigmoid(z)
}

#[derive(Clone, Debug)]
pub struct Decision {
    pub option: String,
    pub p_direct: f64,
    pub reason: String,
    pub tier: String, // rule | model | grey | policy | human | review | model_ack | model_review
    pub scores: IndexMap<String, f64>,
}

impl Decision {
    pub fn new(option: &str, p: f64, reason: impl Into<String>, tier: &str) -> Self {
        Decision { option: option.into(), p_direct: p, reason: reason.into(), tier: tier.into(), scores: IndexMap::new() }
    }
    pub fn with_scores(mut self, s: &IndexMap<String, f64>) -> Self {
        self.scores = s.clone();
        self
    }
    pub fn is(&self, opts: &[&str]) -> bool {
        opts.contains(&self.option.as_str())
    }
}

/// Tier 0: zero-cost rules (many from field lessons: stale indices, unknown results, loops).
pub fn hard_rules(ctx: &ActionCtx, f: &Feats, cfg: &GateConfig, confirmed: bool, ack: bool) -> Option<Decision> {
    let kind = ctx.target.kind.as_str();
    if let Some(h) = &ctx.handoff {
        if cfg.risk_confirm && !confirmed {
            return Some(Decision::new("handoff", 0.0, h.clone(), "rule"));
        }
    }
    if let Some(r) = &ctx.risk {
        if cfg.risk_confirm && !confirmed {
            let level = ctx.risk_level.clone().unwrap_or_else(|| "high".into());
            let to_human = cfg.risk_mode == "human" || (cfg.risk_mode == "auto" && level == "high");
            if to_human {
                return Some(Decision::new("ask_human", 0.0, format!("{level}-risk action: {r}"), "rule"));
            }
            if !ack {
                return Some(Decision::new("risk_review", 0.0, format!("{level}-risk action: {r}"), "rule"));
            }
        }
    }
    if fget(f, "token_stale") != 0.0 {
        return Some(Decision::new("heal", 0.0, "element token is from an older snapshot of this window", "rule"));
    }
    if fget(f, "no_obs") != 0.0 && matches!(kind, "token" | "index" | "pixel") {
        return Some(Decision::new(if kind == "pixel" { "look_full" } else { "look_light" }, 0.0,
                                  "no observation of this window has passed through the gate", "rule"));
    }
    if fget(f, "last_err") != 0.0 && fget(f, "repeat") >= 1.0 {
        return Some(Decision::new("look_full", 0.0, "the identical action just failed; its effect is unknown, so it must not be re-sent blind", "rule"));
    }
    if fget(f, "repeat") >= 2.0 && fget(f, "last_unverif") != 0.0 {
        return Some(Decision::new("reason", 0.0, "same action sent repeatedly without a confirmed effect (possible loop)", "rule"));
    }
    if kind == "pixel" && (fget(f, "window_changed") != 0.0 || fget(f, "last_popup") != 0.0) && fget(f, "acts_since_obs") >= 1.0 {
        return Some(Decision::new("look_full", 0.0, "pixel target but windows/popups changed since the screenshot", "rule"));
    }
    if fget(f, "window_added") != 0.0 && fget(f, "acts_since_obs") >= 1.0 && kind != "keyboard" {
        return Some(Decision::new("look_light", 0.0, "a new window or dialog of this app appeared since your last read (list_windows, then read it)", "rule"));
    }
    None
}

/// Tier 1 (+ grey zone): minimise lam*cost + (1-P)*fail_cost.
pub fn choose(ctx: &ActionCtx, f: &Feats, cfg: &GateConfig, check_available: bool, check_done: bool) -> Decision {
    let p = p_direct(f, cfg);
    let pixel = ctx.target.kind == "pixel";
    let keyboard = matches!(ctx.target.kind.as_str(), "keyboard" | "none");
    let mut avail = vec!["direct", "look_light", "look_full", "reason"];
    if check_available && !check_done {
        avail.push("check");
    }
    if fget(f, "loading_cue") != 0.0 || fget(f, "last_popup") != 0.0 {
        avail.push("wait");
    }
    if keyboard {
        avail.retain(|o| *o != "reason");
    }
    let mut scores: IndexMap<String, f64> = IndexMap::new();
    for o in &avail {
        let po = if *o == "direct" {
            p
        } else {
            let g = if pixel { pixel_gain(o).unwrap_or_else(|| cfg.gains.get(*o).copied().unwrap_or(0.0)) } else { cfg.gains.get(*o).copied().unwrap_or(0.0) };
            p + g * (1.0 - p)
        };
        scores.insert(o.to_string(), cfg.lam * cfg.costs.get(*o).copied().unwrap_or(0.0) + (1.0 - po) * cfg.fail_cost);
    }
    let mut best = scores.iter().fold((String::new(), f64::INFINITY), |acc, (k, v)| if *v < acc.1 { (k.clone(), *v) } else { acc }).0;
    let mut tier = "model";
    if check_available && !check_done && best != "direct" && ["look_light", "look_full", "reason"].contains(&best.as_str()) {
        best = "check".into();
        tier = "grey";
    } else if check_available && !check_done && best == "direct" && cfg.grey_lo <= p && p < cfg.grey_hi {
        best = "check".into();
        tier = "grey";
    }
    let mut sorted: Vec<(&String, &f64)> = scores.iter().collect();
    sorted.sort_by(|a, b| a.1.partial_cmp(b.1).unwrap());
    let top: Vec<String> = sorted.iter().take(3).map(|(k, v)| format!("{k}={v:.2}")).collect();
    let reason = format!("P(direct)={p:.2}; expected cost {}", top.join(", "));
    Decision::new(&best, p, reason, tier).with_scores(&scores)
}

/// Baselines. direct: never intervene. light/full: require a fresh read (tree / screenshot) every step.
pub fn policy_decision(ctx: &ActionCtx, f: &Feats, cfg: &GateConfig) -> Option<Decision> {
    if cfg.policy == "direct" {
        return Some(Decision::new("direct", 1.0, "baseline: always direct", "policy"));
    }
    if cfg.policy == "light" || cfg.policy == "full" {
        let no_obs = fget(f, "no_obs") != 0.0;
        if matches!(ctx.target.kind.as_str(), "keyboard" | "none") && no_obs {
            return Some(Decision::new("direct", 1.0, "baseline: nothing to observe", "policy"));
        }
        let need_shot = cfg.policy == "full";
        let fresh = !no_obs && fget(f, "acts_since_obs") == 0.0 && (fget(f, "obs_had_shot") != 0.0 || !need_shot)
            && fget(f, "token_stale") == 0.0;
        if fresh {
            return Some(Decision::new("direct", 1.0, format!("baseline {}: fresh observation", cfg.policy), "policy"));
        }
        return Some(Decision::new(if need_shot { "look_full" } else { "look_light" }, 0.0,
                                  format!("baseline {}: observe before every action", cfg.policy), "policy"));
    }
    None
}
