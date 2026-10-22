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

/// What an option fixes beyond the features it resets (see `after`): the residual chance it turns a
/// would-be failure into a success. ask_help: a person does or explains the step.
fn prior_gains() -> IndexMap<String, f64> {
    [("check", 0.03), ("heal", 0.65), ("wait", 0.2), ("look_light", 0.15), ("look_full", 0.3), ("review", 0.35),
     ("ask_help", 0.9)]
        .iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

fn pixel_gain(o: &str) -> Option<f64> {
    match o {
        "look_light" => Some(0.05),
        "look_full" => Some(0.5),
        "check" => Some(0.0),
        _ => None,
    }
}

/// Seconds. Options the gate resolves itself (check, heal, wait) cost well under a second; options that hand
/// the step back to the model cost a whole model turn (about 7 s measured, model + relay); a person ~45 s.
fn default_costs() -> IndexMap<String, f64> {
    [("direct", 0.0), ("check", 0.2), ("heal", 0.3), ("wait", 1.0), ("look_light", 7.0), ("look_full", 9.0),
     ("review", 3.0), ("ask_human", 45.0), ("ask_help", 45.0)].iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

/// Counterfactual features: the state this option leaves behind, scored by the same predictor.
/// A fresh read clears staleness; waiting clears loading; re-thinking clears doubt about the target.
pub fn after(f: &Feats, o: &str) -> Feats {
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
        // a probe only tells; it fixes nothing by itself (in 332 logged probes it flagged 2 of 82 later failures)
        "check" => {}
        _ => {}
    }
    g
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
}

impl Default for GateConfig {
    fn default() -> Self {
        GateConfig {
            policy: "gate".into(), lam: 1.0, fail_cost: 19.0, turn_cost: 7.0, side_cost: 5.0, grey_lo: 0.55, grey_hi: 0.9, risk_confirm: true,
            risk_mode: "auto".into(), decider: "formula".into(), reviewer: false, collect: false, human_channel: "auto".into(),
            human_timeout: 120.0, toolset: "core".into(), auto_observe: true, delivery: "auto".into(), task: String::new(),
            batch: "auto".into(), deny_consecutive: 3, deny_total: 8, costs: default_costs(), weights: prior_weights(),
            gains: prior_gains(), calib: Some(PRIOR_CALIB), model_name: "osworld_v3".into(), fail_exp: 1.0, tree_diff: true, text_only: false,
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
                if let Ok(t) = std::fs::read_to_string(tf) { c.task = t.trim().to_string(); }
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
        c
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

/// Tier 1 (+ grey zone): pick the option with the lowest expected cost in seconds,
///   E[cost(o)] = lam * cost(o) + (1 - P_o) * fail_cost,
/// where P_o is the predictor's success probability on the state option o leaves behind (`after`), plus that
/// option's residual gain. One formula ranks acting now, cheap gate-side fixes, handing back to the model and
/// asking a person; a person becomes the cheapest option only once the run is clearly stuck.
pub fn choose(ctx: &ActionCtx, f: &Feats, cfg: &GateConfig, check_available: bool, check_done: bool) -> Decision {
    let p = p_direct(f, cfg);
    let pixel = ctx.target.kind == "pixel";
    let keyboard = matches!(ctx.target.kind.as_str(), "keyboard" | "none");
    // a pixel target is re-aimed on a screenshot: a tree-only look cannot fix it (except for a text-only client)
    let mut avail = if pixel && !cfg.text_only { vec!["direct", "look_full"] } else { vec!["direct", "look_light", "look_full"] };
    if cfg.reviewer && !pixel {
        // (a pixel target needs fresh pixels; the reviewer reads the element tree, not the screen)
        avail.push("review"); // the gate's own independent model checks the target; the acting model is not asked
    }
    if check_available && !check_done {
        avail.push("check");
    }
    if fget(f, "loading_cue") != 0.0 || fget(f, "last_popup") != 0.0 {
        avail.push("wait");
    }
    if keyboard {
        avail.retain(|o| *o != "review");
    }
    if cfg.human_channel != "deny" && fget(f, "consec_fail") >= 2.0 {
        avail.push("ask_help");
    }
    let cf = fail_cost(ctx, f, cfg);
    let mut scores: IndexMap<String, f64> = IndexMap::new();
    for o in &avail {
        let po = if *o == "direct" {
            p
        } else {
            let pa = p_direct(&after(f, o), cfg).max(p);
            let g = if pixel { pixel_gain(o).unwrap_or_else(|| cfg.gains.get(*o).copied().unwrap_or(0.0)) } else { cfg.gains.get(*o).copied().unwrap_or(0.0) };
            pa + g * (1.0 - pa)
        };
        scores.insert(o.to_string(), cfg.lam * cfg.costs.get(*o).copied().unwrap_or(0.0) + (1.0 - po) * cf);
    }
    let mut best = scores.iter().fold((String::new(), f64::INFINITY), |acc, (k, v)| if *v < acc.1 { (k.clone(), *v) } else { acc }).0;
    let mut tier = "model";
    // before handing the step back to the model (a whole turn), try the cheap gate-side probe once
    if check_available && !check_done && ["look_light", "look_full", "review"].contains(&best.as_str()) {
        best = "check".into();
        tier = "grey";
    }
    let mut sorted: Vec<(&String, &f64)> = scores.iter().collect();
    sorted.sort_by(|a, b| a.1.partial_cmp(b.1).unwrap());
    let top: Vec<String> = sorted.iter().take(3).map(|(k, v)| format!("{k}={v:.1}s")).collect();
    let stuck = if fget(f, "consec_fail") >= 1.0 { format!("; {} failed in a row", fget(f, "consec_fail")) } else { String::new() };
    let reason = format!("P(direct)={p:.2}{stuck}; expected cost {}", top.join(", "));
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
