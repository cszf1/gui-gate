//! The value gate: hard rules, then a calibrated light predictor that compares the expected benefit of
//! extra observation / reasoning against its cost, with a cheap probe in the grey zone.
//!
//! Options: direct, check (cheap tree probe), heal (re-bind a stale token), wait, look_light (re-read the
//! tree), look_full (re-read with a screenshot), reason (double-check the target), risk_review (main model
//! review (the gate's own independent reviewer model checks the step; the acting model is never asked to judge
//! its own action), risk_review (reviewer decides a medium-risk step), ask_human, ask_help, handoff (a person must
//! act), deny (deny-and-continue).
use crate::features::{fget, ActionCtx, Feats, FEATURE_NAMES};
use crate::util::*;
use indexmap::IndexMap;
use serde_json::Value;

/// Built-in weights (osworld_v3, 0.9.6): fitted on 6,866 executed actions from 427 sessions of every logged run up to
/// 2026-10-22 (deepseek + screenshots and mercury-3 text-only, clean labels: navigation-key repeats are not failures),
/// `gui-gate train`, L2 towards the v2 weights, Platt-calibrated. Cross-run held-out AUC: trained without the mercury-3
/// run -> 0.785 on it (v2: 0.705); trained without the deepseek 0.9.4 run -> 0.747 on it (v2: 0.607).
/// weights/osworld-v3.json; the previous built-in is weights/osworld-v2.json.
pub fn prior_weights() -> IndexMap<String, f64> {
    [("bias", 1.277), ("is_pixel", -0.535), ("is_token", -0.245), ("is_name", 0.300), ("is_keyboard", 0.488), ("no_obs", -2.413), ("token_stale", -3.215), ("log_obs_age", 0.331), ("acts_since_obs", 0.055), ("obs_had_shot", 0.521), ("last_err", -1.014), ("last_unverif", 0.274), ("last_popup", -1.354), ("repeat", -0.615), ("edge_close", -0.715), ("small_target", -0.600), ("label_empty", -0.369), ("label_dup", -0.682), ("loading_cue", -1.171), ("window_changed", -1.152), ("window_added", -1.472), ("elem_unknown", -0.661), ("check_pass", 1.273), ("check_fail", -3.502), ("prev_none", -0.236), ("sess_fail", -0.067), ("consec_fail", -0.090), ("sig_failed", -0.191), ("role_menu", -0.480), ("role_cell", -0.194), ("role_toggle", -0.032)]
        .iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

/// The hand-set prior (v1), kept for comparison: GUI_GATE_WEIGHTS=prior_v1.
pub fn prior_v1_weights() -> IndexMap<String, f64> {
    [("bias", 2.6), ("is_pixel", -0.9), ("is_token", 0.4), ("is_name", 0.3), ("is_keyboard", 0.2), ("no_obs", -2.5),
     ("token_stale", -3.2), ("log_obs_age", -0.15), ("acts_since_obs", -0.55), ("obs_had_shot", 0.1), ("last_err", -0.9),
     ("last_unverif", -0.3), ("last_popup", -1.2), ("repeat", -0.8), ("edge_close", -1.0), ("small_target", -0.6),
     ("label_empty", -0.4), ("label_dup", -0.9), ("loading_cue", -1.3), ("window_changed", -1.2), ("window_added", -1.5),
     ("elem_unknown", -0.7), ("check_pass", 1.6), ("check_fail", -3.5)]
        .iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

pub const PRIOR_CALIB: (f64, f64) = (0.9967, -0.04637);

/// Hand-set residual gains of the LEGACY value prior (see value.rs): not estimated from data.
fn prior_gains() -> IndexMap<String, f64> {
    crate::value::legacy_gains()
}

/// Prior seconds per option, used only when no measured cost exists (value::CostTable, `gui-gate costs`).
/// Options the gate resolves itself (check, heal, wait) cost well under a second; options that hand the step back
/// to the model cost a whole model turn (about 7 s measured once, model + relay); a person ~45 s.
fn default_costs() -> IndexMap<String, f64> {
    [("direct", 0.0), ("check", 0.2), ("heal", 0.3), ("wait", 1.0), ("look_light", 7.0), ("look_full", 9.0),
     ("review", 3.0), ("reason", 7.0), ("ask_human", 45.0), ("ask_help", 45.0)].iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

/// LEGACY pseudo-counterfactual features (value::legacy_after): kept for the legacy prior and old tests only.
pub fn after(f: &Feats, o: &str) -> Feats {
    crate::value::legacy_after(f, o)
}

/// What a failed step costs, in seconds: noticing it and redoing it takes about two model turns plus clean-up,
/// Signature "press_key|<pid>|<keys>" of a plain navigation key press (see is_nav_key).
pub fn nav_signature(sig: &str) -> bool {
    let parts: Vec<&str> = sig.split('|').collect();
    if parts.len() < 3 || !(parts[0] == "press_key" || parts[0] == "hotkey") {
        return false;
    }
    let ctx = ActionCtx { tool: parts[0].into(), args: serde_json::json!({}), signature: sig.into(), ..Default::default() };
    is_nav_key(&ctx)
}

/// A plain navigation key (arrows, Tab, Page Up/Down, Home/End), optionally with Shift/Ctrl.
pub fn is_nav_key(ctx: &ActionCtx) -> bool {
    if ctx.tool != "press_key" && ctx.tool != "hotkey" {
        return false;
    }
    let mut keys: Vec<String> = vec![];
    if let Some(k) = ctx.args.get("key").and_then(|k| k.as_str()) {
        keys.push(k.to_lowercase());
    }
    if let Some(a) = ctx.args.get("keys").and_then(|k| k.as_array()) {
        keys.extend(a.iter().filter_map(|k| k.as_str()).map(|k| k.to_lowercase()));
    }
    if keys.is_empty() && ctx.args.get("modifiers").is_none() {
        // replayed logs keep only the signature "press_key|pid|keys"
        if let Some(last) = ctx.signature.rsplit('|').next() {
            keys.extend(last.split('+').filter(|k| !k.is_empty()).map(|k| k.to_lowercase()));
        }
    }
    let mods: Vec<String> = ctx.args.get("modifiers").and_then(|m| m.as_array())
        .map(|a| a.iter().filter_map(|k| k.as_str()).map(|k| k.to_lowercase()).collect()).unwrap_or_default();
    let nav = regex::Regex::new(r"^(up|down|left|right|tab|page_?up|page_?down|pgup|pgdn|home|end|arrow_?(up|down|left|right))$").unwrap();
    let ok_mod = |k: &str| matches!(k, "shift" | "ctrl" | "control");
    !keys.is_empty() && keys.iter().any(|k| nav.is_match(k)) && keys.iter().all(|k| nav.is_match(k) || ok_mod(k)) && mods.iter().all(|m| ok_mod(m))
}

/// scaled by how hard the step is to undo and by how stuck the run already is (consecutive known failures).
pub fn fail_cost(ctx: &ActionCtx, f: &Feats, cfg: &GateConfig) -> f64 {
    let base = 2.0 * cfg.turn_cost + cfg.side_cost;
    let tool = ctx.tool.as_str();
    let keys = ctx.args.get("keys").or(ctx.args.get("key")).map(|k| pystr(Some(k)).to_lowercase()).unwrap_or_default();
    let label = ctx.target.the_label().to_lowercase();
    let commit = regex::Regex::new(r"^(ok|save|apply|send|submit|replace|yes|finish|done|confirm|delete|remove)\b").unwrap();
    let sev = if ctx.risk_level.is_some() {
        3.0
    } else if tool == "scroll" || (tool == "press_key" && regex::Regex::new(r"^(up|down|left|right|tab|escape|esc|page_?up|page_?down|home|end)$").unwrap().is_match(&keys)) {
        0.5
    } else if (tool == "press_key" && (keys == "return" || keys == "enter")) || (tool.contains("click") && commit.is_match(&label)) {
        2.0
    } else if matches!(tool, "type_text" | "set_value" | "hotkey") {
        1.5
    } else {
        1.0
    };
    base * sev * (1.0 + fget(f, "consec_fail")).powf(cfg.fail_exp)
}

#[derive(Clone, Debug)]
pub struct GateConfig {
    pub policy: String,
    pub lam: f64,
    pub fail_cost: f64,
    pub turn_cost: f64,
    pub side_cost: f64,
    pub grey_lo: f64,
    pub grey_hi: f64,
    pub risk_confirm: bool,
    pub risk_mode: String,
    pub decider: String,
    pub reviewer: bool, // an independent reviewer model is configured (GUI_GATE_REVIEWER_URL)
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
    /// exponent of the stuck factor (1+consec_fail)^e in the fail cost (0.9.5: 1.0, was 1.5)
    pub fail_exp: f64,
    /// auto-observe sends only the rows of a window's tree the agent has not seen yet (GUI_GATE_TREE_DIFF=0 turns it off)
    pub tree_diff: bool,
    /// the acting model cannot see images (GUI_GATE_TEXT_ONLY=1 / --text-only): no screenshots, tree-only advice
    pub text_only: bool,
    /// requested value model: auto (treatment when its weights are loaded, else legacy) | legacy | treatment
    pub value_model_req: String,
    /// active value model: "legacy" (hand-set prior) or "treatment" (value::TreatmentModel)
    pub value_model: String,
    pub treatment: Option<std::sync::Arc<crate::value::TreatmentModel>>,
    /// measured option costs (GUI_GATE_COSTS); None = the prior seconds in `costs`
    pub cost_table: Option<crate::value::CostTable>,
    /// value policy (GUI_GATE_VALUE_POLICY): learned | direct | observe_light | observe_full | extra_reason | threshold | heuristic | random
    pub value_policy: String,
    pub value_threshold: f64,
    pub value_random_rate: f64,
    pub value_seed: u64,
    /// how check is valued: voi (value of information) | legacy (forced probe before a hand-back)
    pub check_model: String,
    /// the operator's task file (--task-file / GUI_GATE_TASK_FILE): re-read when it changes; the newest version is used
    pub task_file: Option<String>,
}

impl Default for GateConfig {
    fn default() -> Self {
        GateConfig {
            policy: "gate".into(), lam: 1.0, fail_cost: 19.0, turn_cost: 7.0, side_cost: 5.0, grey_lo: 0.55, grey_hi: 0.9, risk_confirm: true,
            risk_mode: "auto".into(), decider: "formula".into(), reviewer: false, collect: false, human_channel: "auto".into(),
            human_timeout: 120.0, toolset: "core".into(), auto_observe: true, delivery: "auto".into(), task: String::new(),
            batch: "auto".into(), deny_consecutive: 3, deny_total: 8, costs: default_costs(), weights: prior_weights(),
            gains: prior_gains(), calib: Some(PRIOR_CALIB), model_name: "osworld_v3".into(), fail_exp: 1.0, tree_diff: true, text_only: false,
            value_model_req: "auto".into(), value_model: "legacy".into(), treatment: None, cost_table: None,
            value_policy: "learned".into(), value_threshold: 0.7, value_random_rate: 0.1, value_seed: 0, check_model: "legacy".into(),
            task_file: None,
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
        c.tree_diff = env_flag("GUI_GATE_TREE_DIFF", true);
        c.text_only = env_flag("GUI_GATE_TEXT_ONLY", false);
        if let Some(v) = env("GUI_GATE_DELIVERY") { c.delivery = v; }
        if let Some(v) = env("GUI_GATE_TASK") { c.task = v; }
        if c.task.is_empty() {
            if let Some(tf) = env("GUI_GATE_TASK_FILE") {
                if let Ok(t) = std::fs::read_to_string(&tf) { c.task = crate::server::parse_task_file(&t).0; }
                c.task_file = Some(tf);
            }
        }
        if let Some(v) = env("GUI_GATE_BATCH") { c.batch = v; }
        if let Some(v) = env("GUI_GATE_TURN_COST").and_then(|v| v.parse().ok()) { c.turn_cost = v; }
        if let Some(v) = env("GUI_GATE_FAIL_EXP").and_then(|v| v.parse::<f64>().ok()).filter(|v| (0.0..=3.0).contains(v)) { c.fail_exp = v; }
        if let Some(w) = env("GUI_GATE_WEIGHTS") {
            if w == "prior_v1" {
                c.weights = prior_v1_weights();
                c.calib = None;
                c.model_name = "prior_v1".into();
            } else {
                let _ = c.load_weights(&w);
            }
        }
        if let Some(v) = env("GUI_GATE_VALUE_WEIGHTS") {
            match crate::value::TreatmentModel::load(&v) {
                Ok(m) => c.treatment = Some(std::sync::Arc::new(m)),
                Err(e) => log_err(&format!("cannot load value weights {v}: {e} (legacy value prior stays active)")),
            }
        }
        if let Some(v) = env("GUI_GATE_COSTS") {
            match crate::value::CostTable::load(&v) {
                Ok(t) => c.cost_table = Some(t),
                Err(e) => log_err(&format!("cannot load measured costs {v}: {e} (prior seconds are used)")),
            }
        }
        if let Some(t) = c.cost_table.as_mut() {
            if let Some(v) = env("GUI_GATE_COST_TOKEN_S").and_then(|v| v.parse().ok()) { t.token_s_per_k = v; }
            if let Some(v) = env("GUI_GATE_COST_PERSON_W").and_then(|v| v.parse().ok()) { t.person_w = v; }
            if let Some(v) = env("GUI_GATE_COST_MIN_N").and_then(|v| v.parse().ok()) { t.min_n = v; }
        }
        if let Some(v) = env("GUI_GATE_VALUE_POLICY") {
            if crate::value::VALUE_POLICIES.contains(&v.as_str()) { c.value_policy = v; } else { log_err(&format!("unknown GUI_GATE_VALUE_POLICY {v}; using learned")); }
        }
        if let Some(v) = env("GUI_GATE_VALUE_THRESHOLD").and_then(|v| v.parse().ok()) { c.value_threshold = v; }
        if let Some(v) = env("GUI_GATE_VALUE_RANDOM_RATE").and_then(|v| v.parse().ok()) { c.value_random_rate = v; }
        if let Some(v) = env("GUI_GATE_VALUE_SEED").and_then(|v| v.parse().ok()) { c.value_seed = v; }
        if let Some(v) = env("GUI_GATE_VALUE_MODEL") { c.value_model_req = v.to_lowercase(); }
        c.resolve_value_model();
        if let Some(v) = env("GUI_GATE_CHECK_MODEL") { if v == "voi" || v == "legacy" { c.check_model = v; } }
        c
    }

    /// Which value model is active: treatment only when its weights are loaded (auto / treatment), else legacy.
    pub fn resolve_value_model(&mut self) {
        let want_treatment = matches!(self.value_model_req.as_str(), "auto" | "treatment");
        self.value_model = if want_treatment && self.treatment.is_some() { "treatment".into() } else { "legacy".into() };
        if self.value_model_req == "treatment" && self.treatment.is_none() {
            log_err("GUI_GATE_VALUE_MODEL=treatment but no treatment weights are loaded (GUI_GATE_VALUE_WEIGHTS); the legacy value prior is active");
        }
        // check is valued as information with the treatment model; the legacy prior keeps its forced probe
        self.check_model = if self.value_model == "treatment" { "voi".into() } else { "legacy".into() };
    }

    pub fn load_weights(&mut self, path: &str) -> Result<(), String> {
        let d: Value = serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        self.calib = None;
        for (field, map) in [("weights", &mut self.weights), ("gains", &mut self.gains), ("costs", &mut self.costs)] {
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
        if let Some(t) = d.get("treatment") {
            self.treatment = Some(std::sync::Arc::new(crate::value::TreatmentModel::from_json(t)?));
            self.resolve_value_model();
        }
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
            // medium risk (or every risk with risk_mode=reviewer): the gate's independent reviewer decides
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
    // moving through a list or text with arrow/Tab/Page keys repeats the same key by design: not a loop
    // (all logs: 117 such presses executed, 0 errors; the 6 that went to the reviewer were denied and re-sent)
    if fget(f, "repeat") >= 2.0 && fget(f, "last_unverif") != 0.0 && !is_nav_key(ctx) {
        return Some(Decision::new("review", 0.0, "same action sent repeatedly without a confirmed effect (possible loop)", "rule"));
    }
    if kind == "pixel" && (fget(f, "window_changed") != 0.0 || fget(f, "last_popup") != 0.0) && fget(f, "acts_since_obs") >= 1.0 {
        return Some(Decision::new("look_full", 0.0, "pixel target but windows/popups changed since the screenshot", "rule"));
    }
    if fget(f, "window_added") != 0.0 && fget(f, "acts_since_obs") >= 1.0 && kind != "keyboard" {
        return Some(Decision::new("look_light", 0.0, "a new window or dialog of this app appeared since your last read (list_windows, then read it)", "rule"));
    }
    None
}

/// Tier 1: the value step (value::choose): pick the option with the lowest expected cost in seconds,
///   E[cost(o)] = lam * cost(o) + (1 - P_o) * fail_cost,
/// with P_o from the active value model (the treatment-interaction model when trained weights are loaded, else the
/// hand-set legacy prior), cost(o) measured from logs when available, and check valued as information (voi).
pub fn choose(ctx: &ActionCtx, f: &Feats, cfg: &GateConfig, check_available: bool, check_done: bool) -> Decision {
    crate::value::choose(ctx, f, cfg, check_available, check_done)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::{history_feats, Target};

    fn ctx(tool: &str, kind: &str, label: &str) -> ActionCtx {
        ActionCtx { tool: tool.into(), args: serde_json::json!({}), target: Target { kind: kind.into(), label: label.into(), ..Default::default() },
                    signature: format!("{tool}|{label}"), risk: None, steps: vec![], risk_level: None, handoff: None }
    }
    fn feats() -> Feats {
        let mut f: Feats = FEATURE_NAMES.iter().map(|k| (*k, 0.0)).collect();
        f["bias"] = 1.0;
        f["is_token"] = 1.0;
        f
    }

    #[test]
    fn normal_step_goes_direct() {
        let c = choose(&ctx("click", "token", "Bold"), &feats(), &GateConfig::default(), true, false);
        assert_eq!(c.option, "direct", "{}", c.reason);
    }

    #[test]
    fn stuck_run_asks_a_person_only_when_one_is_reachable() {
        let mut f = feats();
        let prev: Vec<(String, bool, bool)> = (0..4).map(|_| ("click|Bold".to_string(), true, true)).collect();
        for (k, v) in history_feats(&prev, "click|Bold") {
            f[k] = v;
        }
        f["repeat"] = 2.0;
        f["last_err"] = 1.0;
        let mut cfg = GateConfig::default();
        cfg.human_channel = "auto".into();
        // osworld_v3 (0.9.6): repeated failures predict the next failure less than v2 assumed, so a person and a
        // full re-look are within ~2 s of each other here; either is fine, acting blind is not
        let d = choose(&ctx("click", "token", "Bold"), &f, &cfg, false, false);
        assert!(["ask_help", "look_full"].contains(&d.option.as_str()), "p={} {:?}", d.p_direct, d.scores);
        assert!(d.scores["ask_help"] < d.scores["direct"]);
        cfg.human_channel = "deny".into();
        assert_ne!(choose(&ctx("click", "token", "Bold"), &f, &cfg, false, false).option, "ask_help");
    }

    #[test]
    fn doubtful_step_goes_to_the_reviewer_never_back_to_the_agent() {
        let mut f = feats();
        f["label_dup"] = 1.0;
        f["sig_failed"] = 1.0;
        f["repeat"] = 1.0;
        f["consec_fail"] = 1.0;
        let mut cfg = GateConfig::default();
        cfg.reviewer = true;
        let with = choose(&ctx("click", "token", "Save"), &f, &cfg, false, false);
        assert_eq!(with.option, "review", "{}", with.reason);
        cfg.reviewer = false;
        let without = choose(&ctx("click", "token", "Save"), &f, &cfg, false, false);
        assert_ne!(without.option, "review");
        assert_ne!(without.option, "reason");
    }

    #[test]
    fn failing_commit_costs_more_than_scrolling() {
        let cfg = GateConfig::default();
        let f = feats();
        assert!(fail_cost(&ctx("click", "token", "Save"), &f, &cfg) > fail_cost(&ctx("scroll", "token", "list"), &f, &cfg) * 3.0);
    }

    #[test]
    fn a_fresh_read_clears_staleness() {
        let mut f = feats();
        f["window_added"] = 1.0;
        f["acts_since_obs"] = 3.0;
        let cfg = GateConfig::default();
        assert!(p_direct(&after(&f, "look_light"), &cfg) > p_direct(&f, &cfg) + 0.1);
    }

    #[test]
    fn one_failure_grows_the_fail_cost_linearly() {
        let mut f = feats();
        f["consec_fail"] = 1.0;
        let mut cfg = GateConfig::default();
        assert_eq!(cfg.fail_exp, 1.0);
        let c0 = fail_cost(&ctx("click", "token", "Bold"), &feats(), &cfg);
        let c1 = fail_cost(&ctx("click", "token", "Bold"), &f, &cfg);
        assert!((c1 / c0 - 2.0).abs() < 1e-9);
        cfg.fail_exp = 1.5;
        let c15 = fail_cost(&ctx("click", "token", "Bold"), &f, &cfg);
        assert!(c15 > c1 * 1.4);
    }

    #[test]
    fn repeated_arrow_keys_are_not_a_loop() {
        let mut f = feats();
        f["repeat"] = 2.0;
        f["last_unverif"] = 1.0;
        let mut c = ctx("press_key", "keyboard", "");
        c.args = serde_json::json!({"key": "Down"});
        assert!(is_nav_key(&c));
        let cfg = GateConfig::default();
        assert!(hard_rules(&c, &f, &cfg, false, false).map(|d| d.option != "review").unwrap_or(true));
        c.args = serde_json::json!({"key": "Return"});
        assert!(!is_nav_key(&c));
        assert_eq!(hard_rules(&c, &f, &cfg, false, false).unwrap().option, "review");
        c.args = serde_json::json!({"key": "down", "modifiers": ["shift"]});
        assert!(is_nav_key(&c));
        c.args = serde_json::json!({"key": "Tab", "modifiers": ["alt"]});
        assert!(!is_nav_key(&c));
        assert!(nav_signature("press_key|123|Down"));
        assert!(nav_signature("press_key|123|shift+down"));
        assert!(!nav_signature("press_key|123|Return"));
        assert!(!nav_signature("click|123|button|Down"));
    }

    #[test]
    fn settled_menu_click_after_wait_and_check_goes_direct() {
        // v094 ow11/ow12/ow14: menu click right after a popup, duplicate label, tree check passed
        let mut f = feats();
        for (k, v) in [("log_obs_age", 0.004), ("label_dup", 1.0), ("check_pass", 1.0), ("sess_fail", -1.609), ("role_menu", 1.0)] {
            f[k] = v;
        }
        let cfg = GateConfig::default();
        let c = ctx("click", "token", "Insert");
        f["last_popup"] = 1.0;
        let before = choose(&c, &f, &cfg, false, true);
        f["last_popup"] = 0.0;
        let after_wait = choose(&c, &f, &cfg, false, true);
        assert_eq!(after_wait.option, "direct", "{} / before: {}", after_wait.reason, before.option);
    }
}
