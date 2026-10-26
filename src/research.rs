//! Decision data contract, local net gain, escalation advice and observation freshness (v0.10.3, review 2026-10-26
//! sections 1, 2, 5 (gate_next_step) and 6). Nothing here changes a default decision: the gate keeps deciding exactly as
//! in acaba69 (`value::tests::default_path_output_is_unchanged`); this module only records what the decision was based
//! on, so that later estimation can separate "before the option" from "after the option", and offers advice.
//!
//! Decision record (every `ev: action` event, see docs/value-model-v2.md section 9):
//! * `decision_id`   - `d-<session>-<aid>`; unique per gated() call.
//! * `x_pre`         - the features at the moment the decision was first taken, BEFORE any gate-side option (check,
//!                     wait, heal) ran. Immutable: later recomputations never touch it. Training uses only `x_pre`.
//! * `x_post`        - the features the final decision used (identical to the legacy `features` key).
//! * `x_post_observed` / `assumed_resets` - after a wait the gate still clears `loading_cue` / `last_popup` for its own
//!                     re-decision (default behaviour, unchanged); `x_post_observed` keeps what was actually observed
//!                     after the wait and `assumed_resets` names the keys that were set by assumption, not observation.
//!                     `waited` is a separate flag: having waited is not "loading finished".
//! * `proposed`      - the option first proposed (rules / value model), before cheap options ran and before overrides.
//! * `candidate`     - the candidate action: tool, signature and a structural effect id (`effect_id`, not the
//!                     approval key of effect.rs: a cheap hash of tool, app, window, role, label and payload text).
//! * `chain`         - every processing step with its start / end in ms since the decision started (`t0_ms`, `t1_ms`),
//!                     its result and, for check, whether it re-bound the target (then the check includes a repair).
//! * `continues`     - for the first action after a hand-back (look_*, reason, review refusal, ask_help another way):
//!                     the hand-back's decision_id, whether this is the same candidate action / same effect, and
//!                     `model_switched` (the model sent a different action). Hand-back labels are attributed only when
//!                     the candidate and the effect are the same.
//! * `labels`        - three layers kept apart: `exec` (delivered / error / unknown), `local` (post-condition met /
//!                     not met / unknown, only when the driver reports an effect) and `task` (never written by the
//!                     gate: the task evaluator's result is joined offline). One never stands in for another.
use crate::features::{fget, ActionCtx, Feats};
use crate::state::{wkey, GateState};
use crate::util::*;
use serde_json::{json, Value};

/// The hand-back options: the step goes back to the acting model (or a person) and the next action continues it.
pub const HANDBACK_OPTIONS: &[&str] = &["look_light", "look_full", "reason", "review", "ask_help"];
/// Seconds within which the next action may continue a hand-back (same window as the trainer used before).
pub const CONTINUE_WINDOW_S: f64 = 180.0;

#[derive(Clone, Debug, Default)]
pub struct Handback {
    pub decision_id: String,
    pub option: String,
    pub sig: String,
    pub effect_id: String,
    pub t: f64,
}

/// Per-gate research state (one field in server::Inner, so the decision contract stays in one place).
#[derive(Clone, Debug, Default)]
pub struct Ledger {
    /// the decision of the gated() call in progress (set when its action event is built)
    pub current: Option<Handback>,
    /// the last hand-back that has not been continued yet
    pub pending: Option<Handback>,
}

impl Ledger {
    /// Called when an action event is built: link it to a pending hand-back (if any) and remember this decision.
    pub fn link(&mut self, decision_id: &str, option: &str, sig: &str, effect_id: &str, t: f64) -> Value {
        let cont = match self.pending.take() {
            Some(h) if t - h.t <= CONTINUE_WINDOW_S => {
                let same_c = h.sig == sig;
                let same_e = h.effect_id == effect_id;
                json!({"decision_id": h.decision_id, "option": h.option, "same_candidate": same_c, "same_effect": same_e,
                       "model_switched": !same_c, "gap_s": round_to(t - h.t, 3), "attributable": same_c && same_e})
            }
            _ => Value::Null,
        };
        self.current = Some(Handback { decision_id: decision_id.into(), option: option.into(), sig: sig.into(), effect_id: effect_id.into(), t });
        cont
    }

    /// Called when gated() returns: a decision that handed the step back (not executed) becomes the pending one.
    pub fn finish(&mut self, executed: bool) {
        if let Some(c) = self.current.take() {
            if !executed && HANDBACK_OPTIONS.contains(&c.option.as_str()) {
                self.pending = Some(c);
            } else if executed {
                self.pending = None;
            }
        }
    }
}

pub fn decision_id(session: &str, aid: u64) -> String {
    format!("d-{}-{aid}", if session.is_empty() { "x" } else { session })
}

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Structural effect id of a candidate action (research label only, never an authorization key): tool, app, window,
/// role, label and the payload text / keys / value. Two sends of the same action on the same element agree.
pub fn effect_id(ctx: &ActionCtx) -> String {
    let a = &ctx.args;
    let payload = ["text", "value", "key", "keys", "modifiers", "command", "direction", "amount"].iter()
        .map(|k| a.get(*k).map(|v| v.to_string()).unwrap_or_default()).collect::<Vec<_>>().join("\u{1f}");
    let t = &ctx.target;
    let raw = format!("{}|{}|{}|{}|{}|{}", ctx.tool, t.pid.map(|p| p.to_string()).unwrap_or_default(),
                      t.window_id.map(|w| w.to_string()).unwrap_or_default(), t.the_role(), t.the_label(), payload);
    format!("e{:016x}", fnv(&raw))
}

pub fn feats_json(f: &Feats) -> Value {
    Value::Object(f.iter().map(|(k, v)| (k.to_string(), json!(v))).collect())
}

/// One processing step of the chain, timed relative to the decision start.
pub fn chain_step(option: &str, t0_ms: f64, t1_ms: f64, extra: Value) -> Value {
    let mut v = json!({"option": option, "t0_ms": round_to(t0_ms, 1), "t1_ms": round_to(t1_ms, 1)});
    if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
        for (k, x) in e {
            o.insert(k.clone(), x.clone());
        }
    }
    v
}

/// Execution and local-progress labels of an executed action (the task label is never written by the gate).
/// exec: "delivered" | "error" | "unknown" (outcome unknown: driver died / timed out / unparseable).
/// local: "met" (the driver verified the effect) | "not_met" (no effect, or an error) | "unknown" (unverifiable, or no
/// effect report). It is a post-condition of the step only where the driver could check one.
pub fn exec_labels(res: &Value, unknown: bool) -> Value {
    let sc = res.get("structuredContent").cloned().unwrap_or(Value::Null);
    let err = res.get("isError").and_then(|x| x.as_bool()).unwrap_or(false);
    let effect = sc.get("effect").and_then(|x| x.as_str()).unwrap_or("");
    let exec = if unknown { "unknown" } else if err { "error" } else { "delivered" };
    let local = match (exec, effect) {
        ("unknown", _) => "unknown",
        ("error", _) => "not_met",
        (_, "none") => "not_met",
        (_, "verified") | (_, "changed") | (_, "ok") => "met",
        _ => "unknown",
    };
    json!({"exec": exec, "local": local, "local_src": if effect.is_empty() { Value::Null } else { json!(format!("driver effect={effect}")) },
           "task": Value::Null, "task_note": "joined offline from the task evaluator; never copied onto steps as an action label"})
}

// ---------------------------------------------------------------- local net gain G_o

/// Price parameters of the local net gain. lam: seconds of wall time; gamma: weight of a person's active second;
/// eta: the intervention-budget price per optional intervention (in seconds), separate from lam because one check and
/// one look_full differ ~100x in time but each spends one intervention of the budget.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prices {
    pub lam: f64,
    pub gamma: f64,
    pub eta: f64,
}

/// G_o = C_recover·Δp_o − λ·Δt_o − γ·ΔH_o − η·1[o ≠ direct]
/// Δp_o: P(step succeeds | o) − P(step succeeds | direct); Δt_o: EXTRA wall seconds before the step is attempted (the
/// option's own time; never the recovery time after a failure, which C_recover·Δp_o already prices); ΔH_o: extra person
/// seconds. Positive: the option is worth it.
pub fn local_gain(o: &str, c_recover: f64, dp: f64, dt_extra: f64, dh: f64, p: Prices) -> f64 {
    let ind = if o == "direct" { 0.0 } else { 1.0 };
    c_recover * dp - p.lam * dt_extra - p.gamma * dh - p.eta * ind
}

// ---------------------------------------------------------------- escalation (advice only)

/// Consecutive known failures of the session, uncapped (the consec_fail FEATURE is capped at 3).
pub fn consecutive_failures(st: &GateState) -> f64 {
    st.actions.iter().rev().take_while(|a| a.known_fail()).count() as f64
}

/// Consecutive failures -> switch strategy -> ask for help -> stop. ADVICE ONLY: it is logged and shown to the client;
/// it never changes a decision of the gate, and safety-required approvals (risk rules, hand-offs) never pass through it.
#[derive(Clone, Debug, PartialEq)]
pub struct Escalation {
    pub stage: &'static str,
    pub reason: String,
}

pub const ESCALATE_SWITCH: f64 = 2.0;
pub const ESCALATE_HELP: f64 = 4.0;
pub const ESCALATE_STOP: f64 = 6.0;

pub fn escalation(consec_fail: f64, loop_level: u8, person_reachable: bool) -> Escalation {
    let c = consec_fail;
    if c >= ESCALATE_STOP || (loop_level >= 3 && !person_reachable) {
        return Escalation { stage: "stop", reason: format!("{c} failures in a row (loop level {loop_level}): stop this subgoal and report what blocks it") };
    }
    if c >= ESCALATE_HELP || loop_level >= 2 {
        return if person_reachable {
            Escalation { stage: "ask_help", reason: format!("{c} failures in a row (loop level {loop_level}): ask a person (optional help, valued like any optional ask)") }
        } else {
            Escalation { stage: "stop", reason: format!("{c} failures in a row (loop level {loop_level}) and no person is reachable: stop this subgoal") }
        };
    }
    if c >= ESCALATE_SWITCH || loop_level >= 1 {
        return Escalation { stage: "switch_strategy", reason: format!("{c} failures in a row: try a clearly different way (keyboard, another menu path, another element)") };
    }
    Escalation { stage: "continue", reason: String::new() }
}

impl Escalation {
    pub fn json(&self) -> Value {
        json!({"stage": self.stage, "reason": self.reason, "advisory": true})
    }
}

// ---------------------------------------------------------------- observation freshness (shared)

/// What the gate knows about how fresh its view of a window is. Shared by auto-observe's duplicate-read check and
/// gate_next_step so that a tree attached to the last action's result is not read again before the next turn.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Freshness {
    pub has_obs: bool,
    pub age_s: f64,
    pub acts_since: usize,
    pub had_shot: bool,
    pub last_err: bool,
    pub last_unverif: bool,
    pub last_popup: bool,
    pub loading_cue: bool,
    /// the auto-observe that came back with the last action (same action id, < 20 s, not truncated)
    pub attached_with_last_action: bool,
    pub attached_had_shot: bool,
}

/// Seconds after which an observation is old for next-step advice even when nothing was done since.
pub const STALE_AFTER_S: f64 = 60.0;

/// The auto-observe record `last_auto[pid]` still describes the window: taken after the current action, < 20 s, a full
/// snapshot (the rule of the duplicate-read check).
pub fn auto_record_fresh(rec: &Value, aid: u64, now_t: f64) -> bool {
    as_i64(rec.get("aid")) == Some(aid as i64) && now_t - rec.get("t").and_then(|x| x.as_f64()).unwrap_or(0.0) <= 20.0
        && !truthy(rec.get("truncated")) && truthy(rec.get("snap"))
}

pub fn freshness(st: &GateState, pid: Option<i64>, wid: Option<i64>, auto: Option<&Value>, aid: u64, now_t: f64) -> Freshness {
    let mut fr = Freshness::default();
    let wk = if pid.is_some() { wkey(pid, wid) } else { None };
    let latest = match (pid, wid) {
        (Some(_), _) => st.latest_for(pid, wid),
        (None, Some(w)) => st.obs.iter().rposition(|o| o.window_id == Some(w)),
        (None, None) => st.latest_by_window.values().max().copied(),
    };
    if let Some(o) = latest {
        let ob = st.get(o);
        fr.has_obs = true;
        fr.age_s = (now_t - ob.t).max(0.0);
        fr.acts_since = st.actions_since(ob.t, wk.as_deref());
        fr.had_shot = ob.with_screenshot;
        fr.loading_cue = ob.loading_cue;
    }
    if let Some(la) = st.actions.last() {
        fr.last_err = la.is_error;
        fr.last_unverif = la.unverifiable;
        fr.last_popup = la.popup;
    }
    if let Some(r) = auto {
        if auto_record_fresh(r, aid, now_t) && (wid.is_none() || !truthy(r.get("wid")) || as_i64(r.get("wid")) == wid) {
            fr.attached_with_last_action = true;
            fr.attached_had_shot = truthy(r.get("shot"));
        }
    }
    fr
}

/// gate_next_step's own rule (no action model, no fictitious click): what to attach before the next model turn.
/// Returns (observe, reason). `pixel_app`: the agent aims by x/y in this app (a screenshot is useful).
pub fn next_step_rule(fr: &Freshness, text_only: bool, pixel_app: bool) -> (&'static str, String) {
    let look = |why: String| if pixel_app && !text_only { ("tree+screenshot", why) } else { ("tree", why) };
    if !fr.has_obs {
        return if text_only { ("tree", "the gate has no observation of this window yet".into()) } else { ("tree+screenshot", "the gate has no observation of this window yet".into()) };
    }
    if fr.attached_with_last_action && (fr.attached_had_shot || !pixel_app || text_only) {
        return ("none", "a fresh read came back with the last action's result".into());
    }
    if fr.acts_since == 0 && fr.age_s <= STALE_AFTER_S && !fr.loading_cue {
        return ("none", format!("nothing was done since the last read ({:.0} s ago)", fr.age_s));
    }
    if fr.loading_cue {
        return look("the window was still loading at the last read".into());
    }
    if fr.last_err || fr.last_popup {
        return look("the last action failed or opened a popup".into());
    }
    if fr.acts_since > 0 && fr.last_unverif {
        return look(format!("{} action(s) since the last read, the last one unverified", fr.acts_since));
    }
    if fr.acts_since >= 3 {
        return look(format!("{} actions since the last read", fr.acts_since));
    }
    if fr.age_s > STALE_AFTER_S {
        return ("tree", format!("the last read is {:.0} s old", fr.age_s));
    }
    ("none", format!("{} verified action(s) since the last read", fr.acts_since))
}

/// The features of a decision that a wait leaves as assumptions (default path: cleared for the re-decision).
pub const WAIT_ASSUMED: &[&str] = &["loading_cue", "last_popup"];

/// The observed values of the wait-assumed keys (before the default path clears them).
pub fn observed_after_wait(f: &Feats) -> Value {
    Value::Object(WAIT_ASSUMED.iter().map(|k| (k.to_string(), json!(fget(f, k)))).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::Target;

    #[test]
    fn handback_links_only_the_same_candidate_and_effect() {
        let mut l = Ledger::default();
        assert!(l.link("d-a-1", "look_full", "click|1|Save", "e1", 10.0).is_null());
        l.finish(false);
        assert!(l.pending.is_some(), "a hand-back that was not executed is pending");
        let c = l.link("d-a-2", "direct", "click|1|Save", "e1", 20.0);
        assert_eq!(c["decision_id"], "d-a-1");
        assert_eq!(c["attributable"], true);
        assert_eq!(c["model_switched"], false);
        l.finish(true);
        assert!(l.pending.is_none());
        // the model switched to another action: linked, but not attributable
        l.link("d-a-3", "look_light", "click|1|Save", "e1", 30.0);
        l.finish(false);
        let c = l.link("d-a-4", "direct", "press_key|1|ctrl+s", "e9", 35.0);
        assert_eq!(c["model_switched"], true);
        assert_eq!(c["attributable"], false);
        l.finish(true);
        // too late: not a continuation
        l.link("d-a-5", "reason", "x", "e", 40.0);
        l.finish(false);
        assert!(l.link("d-a-6", "direct", "x", "e", 40.0 + CONTINUE_WINDOW_S + 1.0).is_null());
        // an executed direct step or a safety refusal is never pending
        l.finish(false);
        assert!(l.pending.is_none(), "direct (not executed, e.g. dispatch refusal) is not a hand-back");
    }

    #[test]
    fn local_gain_does_not_count_recovery_time_twice() {
        let p = Prices { lam: 1.0, gamma: 1.0, eta: 0.0 };
        // Δp = 0.1 of a 40 s recovery = 4 s gained; the look costs 9 s of extra time before the step
        assert!((local_gain("look_full", 40.0, 0.1, 9.0, 0.0, p) - (-5.0)).abs() < 1e-12);
        assert_eq!(local_gain("direct", 40.0, 0.0, 0.0, 0.0, Prices { lam: 1.0, gamma: 1.0, eta: 3.0 }), 0.0);
        // eta prices the intervention itself, independent of its time
        assert!((local_gain("check", 40.0, 0.1, 0.2, 0.0, Prices { lam: 1.0, gamma: 1.0, eta: 3.0 }) - (4.0 - 0.2 - 3.0)).abs() < 1e-12);
        // person seconds are weighted by gamma, not lambda
        assert!((local_gain("ask_help", 40.0, 0.5, 0.0, 30.0, Prices { lam: 1.0, gamma: 2.0, eta: 0.0 }) - (20.0 - 60.0)).abs() < 1e-12);
    }

    #[test]
    fn escalation_ladder_is_advice() {
        assert_eq!(escalation(0.0, 0, true).stage, "continue");
        assert_eq!(escalation(2.0, 0, true).stage, "switch_strategy");
        assert_eq!(escalation(4.0, 0, true).stage, "ask_help");
        assert_eq!(escalation(4.0, 0, false).stage, "stop");
        assert_eq!(escalation(1.0, 2, true).stage, "ask_help");
        assert_eq!(escalation(6.0, 0, true).stage, "stop");
        assert_eq!(escalation(0.0, 0, true).json()["advisory"], true);
    }

    #[test]
    fn next_step_rule_uses_freshness_not_an_action_model() {
        let mut fr = Freshness::default();
        assert_eq!(next_step_rule(&fr, false, false).0, "tree+screenshot");
        assert_eq!(next_step_rule(&fr, true, false).0, "tree");
        fr.has_obs = true;
        fr.attached_with_last_action = true;
        assert_eq!(next_step_rule(&fr, false, false).0, "none", "a tree just came back with the action result");
        fr.attached_with_last_action = false;
        fr.acts_since = 1;
        fr.last_unverif = true;
        assert_eq!(next_step_rule(&fr, false, false).0, "tree");
        assert_eq!(next_step_rule(&fr, false, true).0, "tree+screenshot");
        fr.last_unverif = false;
        assert_eq!(next_step_rule(&fr, false, false).0, "none");
        fr.acts_since = 0;
        fr.age_s = 600.0;
        assert_eq!(next_step_rule(&fr, false, false).0, "tree");
    }

    #[test]
    fn effect_id_and_labels() {
        let mut c = ActionCtx { tool: "type_text".into(), args: json!({"text": "a"}), target: Target { kind: "token".into(), label: "Name".into(), pid: Some(1), ..Default::default() }, ..Default::default() };
        let a = effect_id(&c);
        assert_eq!(a, effect_id(&c));
        c.args = json!({"text": "b"});
        assert_ne!(a, effect_id(&c), "another payload is another effect");
        let ok = json!({"structuredContent": {"effect": "verified"}});
        assert_eq!(exec_labels(&ok, false)["local"], "met");
        assert_eq!(exec_labels(&ok, true)["exec"], "unknown");
        let err = json!({"isError": true, "structuredContent": {}});
        assert_eq!(exec_labels(&err, false)["exec"], "error");
        let unv = json!({"structuredContent": {"effect": "unverifiable"}});
        assert_eq!(exec_labels(&unv, false)["local"], "unknown");
        assert!(exec_labels(&unv, false)["task"].is_null(), "the gate never writes a task label onto a step");
    }
}
