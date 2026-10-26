//! Hand a decision to a person.
//!
//! Channels, tried in this order when channel="auto":
//!   elicit   ask the MCP client's user. A 2026-07-28 client (per-request `_meta`) that declares `elicitation` gets
//!            a Multi Round-Trip Request: the tools/call answers `resultType: "input_required"` with one
//!            `elicitation/create` input request and an opaque one-time `requestState`; the client asks its user
//!            and retries the same call with `inputResponses` + `requestState`. A legacy client (initialize
//!            declared elicitation) gets the old server-to-client `elicitation/create` request.
//!   desktop  a small always-on-top dialog on the machine's own display: zenity / kdialog / GTK via python3 on
//!            Linux, osascript on macOS, PowerShell WinForms on Windows (all ship with the OS or the desktop)
//!   deny     unattended runs (benchmarks): every request is answered "deny" at once
//!   chat     nothing the gate can show itself: the refusal tells the agent to ask its user
//! ask() returns {"answer": "allow"|"deny"|"timeout"|<text>|"input_required", "channel": ..., "ms": ...}.
//! "input_required" means: the request was handed to the client as an MRTR input request (see `CallCtx`); the
//! caller must not act on it, the call's reply is replaced by the input_required result.
//!
//! Kinds (the form the person sees):
//!   approve   allow / deny an action the gate stopped              -> "allow" | "deny"
//!   answer    a short free-text answer                             -> text | "deny"
//!   takeover  the person does a step themselves, then continues    -> "allow" (done) | "deny"
//!   ask_help  the agent is stuck: the person does the step, or tells it to try another way
//!                                                                  -> "allow" (did it) | "deny" (+ note)
//! A timeout, a declined / cancelled form or a closed dialog is never an approval. Every answer also carries an
//! `outcome` saying WHY it ended: accepted | denied (the person chose no) | declined / cancelled (the client's form was
//! declined or dismissed) | timeout | window_closed | channel_unavailable | unattended (channel "deny"). All but
//! accepted are refusals; the reason is logged and returned (human_outcome) so a timeout is not read as "the user
//! objects" and a closed window is not read as "no".
//! An MRTR answer is bound to the call (tool + arguments) AND to the structured effect the gate asked about: when the
//! target, window or payload changed before the retry arrived, the answer is not used and the person is asked again.
use crate::util::*;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub type Elicit = Arc<dyn Fn(Value, f64) -> Option<Value> + Send + Sync>;
pub type CanElicit = Arc<dyn Fn() -> bool + Send + Sync>;

pub const META_KEY: &str = "io.github.cszf1/gui-gate";
pub const KINDS: &[&str] = &["approve", "answer", "takeover", "ask_help"];
/// grace on top of the person's timeout for the client's retry to arrive
const MRTR_GRACE_S: f64 = 5.0;

// ------------------------------------------------------------------ per-call MRTR context
/// What the current tools/call (one thread per call) brings for the person channel.
#[derive(Default, Clone)]
pub struct CallCtx {
    /// a 2026-07-28 client that declared `elicitation`: ask through an MRTR input request
    pub mrtr: bool,
    /// tool name + argument digest of this call (a pending request is bound to it)
    pub binding: String,
    /// the person's answer this retry carries (kind, mapped answer, note, effect asked about, outcome)
    pub answer: Option<(String, String, String, String, String)>,
    /// set by ask(): the input request this call must answer with instead of its normal result
    pub request: Option<Value>,
    /// a gate-built call inside run_actions: never park it as an MRTR request (the batch would re-run on retry)
    pub in_batch: bool,
    /// id (the requestState nonce) of the person request this retry answers; "" when none
    pub req: String,
    /// the answer this retry carries was used by ask_kind (else it is logged as unused when the call ends)
    pub used: bool,
    /// why the carried answer was dropped without being used ("effect_changed"), when it was
    pub dropped: Option<&'static str>,
    /// v0.10.3: what a request parked by this call is about (gated() sets it before asking): the "kind|effect" key
    /// the person is not asked again about after no answer, and the operator task version it was asked under
    pub ask_key: String,
    pub task_version: u64,
}

thread_local! {
    static CALL: RefCell<CallCtx> = RefCell::new(CallCtx::default());
}

pub fn set_call(c: CallCtx) {
    CALL.with(|x| *x.borrow_mut() = c);
}
pub fn take_call() -> CallCtx {
    CALL.with(|x| std::mem::take(&mut *x.borrow_mut()))
}
/// The MRTR input request ask() parked for this call, if any (consumed); the rest of the call context stays.
pub fn take_request() -> Option<Value> {
    CALL.with(|x| x.borrow_mut().request.take())
}
/// The call ended before it reached the person request its retry answers (e.g. frozen): say why, for the log.
pub fn mark_dropped(why: &'static str) {
    CALL.with(|x| { let mut c = x.borrow_mut(); if c.answer.is_some() && !c.used { c.dropped = Some(why); } });
}
pub fn call_mrtr() -> bool {
    CALL.with(|x| x.borrow().mrtr)
}
pub fn set_in_batch(on: bool) {
    CALL.with(|x| x.borrow_mut().in_batch = on);
}
/// The note the person added to an ask_help answer (consumed).
pub fn answer_note() -> String {
    CALL.with(|x| x.borrow().answer.as_ref().map(|a| a.2.clone()).unwrap_or_default())
}

/// gated() names what the next person request of this call is about (kept with the parked request so its end -
/// also an expiry without retry - is recorded against the same effect and task version).
pub fn set_ask_ctx(ask_key: &str, task_version: u64) {
    CALL.with(|x| { let mut c = x.borrow_mut(); c.ask_key = ask_key.to_string(); c.task_version = task_version; });
}

/// The person's answer this MRTR retry carries, when it is NOT an approval: (kind, answer, outcome, request id).
pub fn carried_non_allow() -> Option<(String, String, String, String)> {
    CALL.with(|x| { let c = x.borrow(); c.answer.as_ref().filter(|a| a.1 != "allow").map(|a| (a.0.clone(), a.1.clone(), a.4.clone(), c.req.clone())) })
}

/// The kind of the person answer this retry carries, if any.
pub fn carried_kind() -> Option<String> {
    CALL.with(|x| x.borrow().answer.as_ref().map(|a| a.0.clone()))
}

pub fn mark_used() {
    CALL.with(|x| x.borrow_mut().used = true);
}

/// Tool name + arguments without the gate's own keys: what a pending person request is bound to.
pub fn binding_of(tool: &str, args: &Value) -> String {
    let mut a = args.clone();
    if let Some(o) = a.as_object_mut() {
        for k in ["gate_confirm", "gate_ack", "inputResponses", "requestState"] {
            o.remove(k);
        }
    }
    format!("{tool}|{}", a)
}

/// What a person's MRTR answer is bound to besides the call: the structured effect asked about and, when the gate
/// passes it, the task context it was asked under (`task_binding`: operator task version + reset epoch). An answer
/// whose binding differs from the request being decided now is never reused.
pub fn answer_binding(meta: &Value) -> String {
    let mut eff = meta.get("effect").map(|e| e.to_string()).unwrap_or_default();
    // v0.10.3: and the business premise shown with it (an approve form): other rows now = another question
    if let Some(d) = meta.get("premise").and_then(|p| p.get("digest")).and_then(|d| d.as_str()) {
        eff.push_str(&format!("#premise={d}"));
    }
    match meta.get("task_binding").and_then(|v| v.as_str()) {
        Some(tb) if !tb.is_empty() => format!("{eff}#task={tb}"),
        _ => eff,
    }
}

/// A person request the gate handed to the client and has not had answered yet.
#[derive(Clone)]
pub struct Pending {
    pub binding: String,
    pub kind: String,
    pub deadline: f64,
    /// the structured effect (JSON text) the person is asked about; "" when the request has none
    pub effect: String,
    /// when it was parked (person waiting time = answer time - this)
    pub asked: f64,
    /// voided by a reset while the person may still be answering: it stays open (input stays frozen) until the
    /// person answers or it times out, and whatever the answer is, it approves nothing
    pub voided: bool,
    /// its expiry without a retry was already logged (sweep_expired)
    pub reported: bool,
    /// v0.10.3: "kind|effect" (not asked again after no answer) and the task version it was asked under
    pub ask_key: String,
    pub task_version: u64,
}

/// The request id of a requestState: its nonce (logged with every person event, never the signed state itself).
pub fn req_id(state: &str) -> String {
    state.split('.').nth(2).unwrap_or("").to_string()
}

thread_local! {
    static WAITED: RefCell<f64> = const { RefCell::new(0.0) };
    static ACTIVE: RefCell<Option<f64>> = const { RefCell::new(None) };
    static VOIDED: RefCell<Option<(String, Pending)>> = const { RefCell::new(None) };
}

/// Interaction seconds the CLIENT measured for the answer this retry carries (`_meta[META_KEY].active_ms` of the
/// ElicitResult): evidence of active time. None = no evidence (the waiting time is only an upper bound).
pub fn active_s() -> Option<f64> {
    ACTIVE.with(|a| *a.borrow())
}

/// A voided request (reset) that a retry just consumed: (request id, request) - its single end is "void".
pub fn take_voided() -> Option<(String, Pending)> {
    VOIDED.with(|v| v.borrow_mut().take())
}

/// Seconds the person took on the MRTR request this call's retry answered (0 when none).
pub fn waited_s() -> f64 {
    WAITED.with(|w| *w.borrow())
}

/// The gate's table of MRTR requests: one-time, unguessable ids, bound to the call and its deadline.
#[derive(Default)]
pub struct PendingTable {
    map: Mutex<HashMap<String, Pending>>,
}

pub enum Resolved {
    /// (kind, mapped answer, note, effect asked about, outcome)
    Answer(String, String, String, String, String),
    /// the retry came back after the deadline: a timeout, never an approval (kind, effect)
    Late(String, String),
    Invalid(&'static str),
}

/// This gate process: a random instance id and a random signing key, both new on every start. A requestState is
/// `gg1.<instance>.<nonce>.<hmac>`; one minted by an earlier process (before a restart) is rejected.
pub fn instance() -> &'static (String, String) {
    static I: OnceLock<(String, String)> = OnceLock::new();
    I.get_or_init(|| (rand_hex(8), rand_hex(32)))
}

fn sign_state(inst: &str, nonce: &str, binding: &str) -> String {
    let key = &instance().1;
    hmac_sha256(key.as_bytes(), format!("{inst}.{nonce}|{binding}").as_bytes())[..32].to_string()
}

/// Dialog helper processes the gate itself started (zenity, kdialog, osascript, PowerShell, python3 GTK): no action
/// may ever target their windows.
pub fn dialog_pids() -> &'static Mutex<std::collections::HashSet<i64>> {
    static D: OnceLock<Mutex<std::collections::HashSet<i64>>> = OnceLock::new();
    D.get_or_init(Default::default)
}

pub fn is_dialog_pid(pid: i64) -> bool {
    dialog_pids().lock().map(|d| d.contains(&pid)).unwrap_or(false)
}

/// App names of the helpers the gate shows its dialogs with.
pub const DIALOG_APPS: &[&str] = &["zenity", "kdialog", "osascript"];

impl PendingTable {
    pub fn park(&self, p: Pending) -> String {
        let (inst, _) = instance();
        let nonce = rand_hex(12);
        let id = format!("gg1.{inst}.{nonce}.{}", sign_state(inst, &nonce, &p.binding));
        let mut m = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let t = now();
        m.retain(|_, v| v.deadline + 600.0 > t); // forget long-dead requests
        m.insert(id.clone(), p);
        id
    }

    /// Consume a requestState from a retry and map the person's ElicitResult.
    pub fn resolve(&self, state: &str, binding: &str, responses: Option<&Value>) -> Resolved {
        let parts: Vec<&str> = state.split('.').collect();
        if parts.len() != 4 || parts[0] != "gg1" {
            return Resolved::Invalid("malformed requestState");
        }
        if parts[1] != instance().0 {
            return Resolved::Invalid("requestState is from an earlier gate process (the gate restarted); every earlier approval is void");
        }
        // Only the person's own retry (the right call, carrying an answer) or the timeout ends a request. A wrong call
        // or an answer-less retry burns the approval (it can never approve afterwards) but does NOT withdraw the
        // question: input stays frozen until the person answers or it times out, so nobody else can take it away.
        let p = {
            let mut m = self.map.lock().unwrap_or_else(|e| e.into_inner());
            let Some(p) = m.get_mut(state) else { return Resolved::Invalid("unknown or already used requestState") };
            if sign_state(parts[1], parts[2], &p.binding) != parts[3] {
                return Resolved::Invalid("requestState signature does not match");
            }
            if p.binding != binding {
                p.voided = true;
                return Resolved::Invalid("requestState belongs to a different tool call");
            }
            if responses.and_then(|r| r.get("human")).is_none() && now() <= p.deadline + MRTR_GRACE_S {
                p.voided = true;
                return Resolved::Invalid("the retry carries no answer from the person");
            }
            m.remove(state).unwrap()
        };
        if p.voided {
            VOIDED.with(|v| *v.borrow_mut() = Some((req_id(state), p.clone())));
            return Resolved::Invalid("this request was voided (gate_reset, or a wrong / answer-less retry); the answer approves nothing");
        }
        WAITED.with(|w| *w.borrow_mut() = (now() - p.asked).max(0.0));
        let act = responses.and_then(|r| r.get("human")).and_then(|h| h.get("_meta")).and_then(|m| m.get(META_KEY))
            .and_then(|m| m.get("active_ms")).and_then(|v| v.as_f64()).filter(|v| v.is_finite() && *v >= 0.0).map(|ms| ms / 1000.0);
        ACTIVE.with(|a| *a.borrow_mut() = act);
        if now() > p.deadline + MRTR_GRACE_S {
            return Resolved::Late(p.kind, p.effect);
        }
        let r = responses.and_then(|r| r.get("human"));
        let (ans, note, outcome) = map_elicit_o(&p.kind, r);
        Resolved::Answer(p.kind, ans, note, p.effect, outcome)
    }

    pub fn len(&self) -> usize {
        self.map.lock().map(|m| m.len()).unwrap_or(0)
    }

    /// Requests a person may still be answering (not past their deadline + grace).
    pub fn live(&self) -> usize {
        let t = now();
        self.map.lock().map(|m| m.values().filter(|p| p.deadline + MRTR_GRACE_S > t).count()).unwrap_or(0)
    }

    /// A reset: every parked request is void (its answer approves nothing). Requests a person may still be answering
    /// are NOT ended by it: they stay open, and input stays frozen, until the person answers or the request times out.
    /// Neither the agent nor a reset can withdraw a question that is in front of a person.
    pub fn void_all(&self) -> usize {
        self.void_live().0
    }

    /// void_all, plus the requests this call newly voided (request id, request) so each gets one `void` record.
    /// Requests already dead (past deadline + grace) are forgotten: nobody is answering them any more (the caller
    /// sweeps them first, so their timeout is logged).
    pub fn void_live(&self) -> (usize, Vec<(String, Pending)>) {
        let mut m = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let t = now();
        let n = m.len();
        m.retain(|_, p| p.deadline + MRTR_GRACE_S > t);
        let mut v = vec![];
        for (k, p) in m.iter_mut() {
            if !p.voided {
                p.voided = true;
                v.push((req_id(k), p.clone()));
            }
        }
        v.sort_by(|a, b| a.1.asked.total_cmp(&b.1.asked));
        (n, v)
    }

    /// Remove and return every parked request (gate exit: the process ends, nobody can answer any more):
    /// (request id, request).
    pub fn drain(&self) -> Vec<(String, Pending)> {
        let mut m = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let mut v: Vec<(String, Pending)> = m.drain().map(|(k, p)| (req_id(&k), p)).collect();
        v.sort_by(|a, b| a.1.asked.total_cmp(&b.1.asked));
        v
    }

    /// Requests past deadline + grace whose retry never came, not reported before: marked reported and returned
    /// (request id, request). Voided ones are included: their timeout is what ends the freeze. They stay in the
    /// table so a very late retry still resolves (a timeout, or void), never an approval.
    pub fn sweep_expired(&self) -> Vec<(String, Pending)> {
        let t = now();
        let mut m = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let mut v = vec![];
        for (k, p) in m.iter_mut() {
            if !p.reported && p.deadline + MRTR_GRACE_S < t {
                p.reported = true;
                v.push((req_id(k), p.clone()));
            }
        }
        v
    }
}

/// The form for each kind (MCP restricted JSON Schema: flat object of primitives).
pub fn schema_for(kind: &str) -> Value {
    match kind {
        "answer" => json!({"type": "object", "properties": {"answer": {"type": "string", "title": "Your answer"}}, "required": ["answer"]}),
        "takeover" => json!({"type": "object", "properties": {"done": {"type": "boolean", "title": "I did it, continue",
            "description": "Do the step yourself in the app, then confirm here."}}, "required": ["done"]}),
        "ask_help" => json!({"type": "object", "properties": {
            "choice": {"type": "string", "title": "What now?", "enum": ["did_it", "hint", "try_other"],
                       "enumNames": ["Do it for me: I did this step myself, continue", "Give a hint (type it below), let the agent try again", "Let the agent try another way"]},
            "note": {"type": "string", "title": "A hint for the agent (optional)"}}, "required": ["choice"]}),
        _ => json!({"type": "object", "properties": {"decision": {"type": "string", "title": "Allow this action?",
            "enum": ["allow", "deny"], "enumNames": ["Allow", "Deny"]}}, "required": ["decision"]}),
    }
}

/// schema_for plus 2-4 choices for a clarifying question (the person can still type their own answer).
pub fn schema_with_options(kind: &str, options: Option<&Value>) -> Value {
    let mut sc = schema_for(kind);
    if kind == "answer" {
        if let Some(o) = options.and_then(|o| o.as_array()).filter(|o| (2..=4).contains(&o.len())) {
            let val = |x: &Value| if x.is_object() { pystr(x.get("value")) } else { pystr(Some(x)) };
            let lab = |x: &Value| if x.is_object() { pystr(x.get("label")) } else { pystr(Some(x)) };
            sc["properties"]["choice"] = json!({"type": "string", "title": "Pick one", "enum": o.iter().map(val).collect::<Vec<_>>(),
                                                "enumNames": o.iter().map(lab).collect::<Vec<_>>()});
            sc["properties"]["answer"]["title"] = json!("Or type your own answer");
            sc["required"] = json!([]);
        }
    }
    sc
}

/// ElicitResult -> (answer, note). Anything but an accepted, positive form is a refusal.
pub fn map_elicit(kind: &str, r: Option<&Value>) -> (String, String) {
    let (a, n, _) = map_elicit_o(kind, r);
    (a, n)
}

/// How a refusal reads in the agent's message (all of them are refusals).
pub fn outcome_words(outcome: &str, timeout: f64) -> String {
    match outcome {
        "timeout" => format!("did not answer within {} s (no answer; not a refusal by the person)", timeout as i64),
        "expired" => "never saw the question: it expired while another question was open (no answer)".into(),
        "cancelled" => "cancelled the request".into(),
        "window_closed" => "closed the dialog without answering".into(),
        "channel_unavailable" => "could not be reached".into(),
        "no_response" => "gave no answer".into(),
        "unattended" => "declined (unattended session: every request is refused)".into(),
        _ => "declined".into(),
    }
}

/// map_elicit plus the outcome (why it ended).
pub fn map_elicit_o(kind: &str, r: Option<&Value>) -> (String, String, String) {
    let Some(r) = r else { return ("deny".into(), String::new(), "no_response".into()) };
    match s(r, "action") {
        "accept" => {}
        "decline" => return ("deny".into(), String::new(), refusal_outcome(r, "declined")),
        "cancel" => return ("deny".into(), String::new(), refusal_outcome(r, "cancelled")),
        _ => return ("deny".into(), String::new(), "no_response".into()),
    }
    let (a, n) = map_accepted(kind, r);
    // ask_help "hint" / "try_other" is an answer (the person tells the agent to go on another way), not a "no":
    // it must not read as a refusal (a client would tell the model not to try another way, the opposite of the hint)
    // (a conflicting / ambiguous form stays a refusal: `{choice: "hint", done: true}` is not "try another way")
    let redirected = kind == "ask_help" && a == "deny"
        && !conflicting_form(r.get("content").unwrap_or(&Value::Null))
        && matches!(r.get("content").and_then(|c| c.get("choice")).and_then(|v| v.as_str()), Some("hint") | Some("try_other"));
    let o = if a == "deny" && !redirected { "denied" } else { "accepted" };
    (a, n, o.into())
}

/// Why a declined / cancelled form ended, when the client says so: the reason in the ElicitResult's
/// `_meta["io.github.cszf1/gui-gate"].reason` (or a top-level `reason`). A client popup that timed out, expired in its
/// queue, was closed or could not be shown is NOT the person saying no: it is recorded as no answer with its cause.
fn refusal_outcome(r: &Value, dflt: &str) -> String {
    let why = r.get("_meta").and_then(|m| m.get(META_KEY)).and_then(|m| m.get("reason")).or_else(|| r.get("reason"));
    match why.and_then(|v| v.as_str()).unwrap_or("") {
        "timed_out" | "timeout" | "no_answer" => "timeout".into(),
        "expired" => "expired".into(),
        "closed" | "window_closed" => "window_closed".into(),
        "cancelled" => "cancelled".into(),
        "unavailable" | "channel_unavailable" => "channel_unavailable".into(),
        "declined" => "declined".into(),
        _ => dflt.into(),
    }
}

/// Does an accepted form ALSO carry a refusal or a value that is not one of the form's own? Then the answer is
/// ambiguous, and an ambiguous answer is a refusal (fail closed): `{allow: true, decision: "deny"}`, a `done: true`
/// next to `decision: "deny"`, a non-boolean `allow`, a decision that is not exactly "allow"/"deny" ...
fn conflicting_form(c: &Value) -> bool {
    let Some(o) = c.as_object() else { return !c.is_null() };
    let mut yes = false;
    let mut no = false;
    for (k, v) in o {
        match k.as_str() {
            "decision" => match v.as_str() { Some("allow") => yes = true, Some("deny") => no = true, _ => return true },
            "allow" | "done" => match v.as_bool() { Some(true) => yes = true, Some(false) => no = true, None => return true },
            "choice" => match v.as_str() {
                Some("deny") | Some("try_other") | Some("hint") => no = true,
                Some("did_it") | Some("allow") => yes = true,
                Some(_) => {}
                None => return true,
            },
            _ => {}
        }
    }
    yes && no
}

fn map_accepted(kind: &str, r: &Value) -> (String, String) {
    let c = r.get("content").cloned().unwrap_or(json!({}));
    if kind != "answer" && conflicting_form(&c) {
        // a form that says yes and no at once (or a value outside the form) never approves anything
        let note = if kind == "ask_help" { trunc(pystr(c.get("note")).trim(), 300).to_string() } else { String::new() };
        return ("deny".into(), note);
    }
    match kind {
        "answer" => {
            let typed = pystr(c.get("answer")).trim().to_string();
            let a = if typed.is_empty() { pystr(c.get("choice")).trim().to_string() } else { typed };
            (if a.is_empty() { "deny".into() } else { a }, String::new())
        }
        "takeover" => (if c.get("done").and_then(|v| v.as_bool()) == Some(true) { "allow".into() } else { "deny".into() }, String::new()),
        "ask_help" => {
            let note = trunc(pystr(c.get("note")).trim(), 300).to_string();
            (if s(&c, "choice") == "did_it" { "allow".into() } else { "deny".into() }, note)
        }
        // approve: the enum form, or the pre-2026 boolean form some clients still send (never both disagreeing:
        // conflicting_form above already refused that)
        _ => (if c.get("decision").and_then(|v| v.as_str()) == Some("allow") || c.get("allow").and_then(|v| v.as_bool()) == Some(true) { "allow".into() } else { "deny".into() },
              String::new()),
    }
}

/// What a native dialog printed -> (answer, outcome). Only the dialog's own buttons count; a yes/no dialog's output
/// that is none of them is ambiguous and a refusal (fail closed). Only an "answer" dialog has typed text.
pub fn map_desktop(kind: &str, raw: &str) -> (String, &'static str) {
    match raw {
        "allow" => ("allow".into(), "accepted"),
        "deny" => ("deny".into(), "denied"),
        "timeout" => ("timeout".into(), "timeout"),
        "closed" => ("deny".into(), "window_closed"),
        "unavailable" => ("timeout".into(), "channel_unavailable"),
        _ if kind == "answer" => (raw.to_string(), "accepted"),
        _ => ("deny".into(), "denied"),
    }
}

// ------------------------------------------------------------------ desktop dialogs
const GTK_DIALOG: &str = r#"
import json, sys
import gi
gi.require_version("Gtk", "3.0")
from gi.repository import Gtk, GLib
q = json.loads(sys.argv[1])
d = Gtk.Dialog(title=q.get("title", "gui-gate: a person is needed"))
d.set_keep_above(True); d.set_modal(True); d.set_default_size(460, 160)
box = d.get_content_area(); box.set_spacing(8); box.set_border_width(12)
lab = Gtk.Label(label=q["text"]); lab.set_line_wrap(True); lab.set_xalign(0); box.add(lab)
entry = None
if q.get("want_text"):
    entry = Gtk.Entry(); box.add(entry)
d.add_button(q.get("ok", "Allow"), 1); d.add_button(q.get("no", "Deny"), 2)
# Enter / a stray keystroke never approves: the default button is the refusing one (a typed answer is sent with Enter)
d.set_default_response(1 if entry is not None else 2)
if entry is not None:
    entry.set_activates_default(True)
GLib.timeout_add_seconds(int(q.get("timeout", 120)), lambda: (d.response(3), False)[1])
d.show_all(); d.present()
if entry is None:
    d.get_widget_for_response(2).grab_focus()
r = d.run()
out = {1: "allow", 2: "deny", 3: "timeout"}.get(r, "closed")
if entry is not None and r == 1:
    print(json.dumps({"answer": entry.get_text(), "typed": True}))
else:
    print(json.dumps({"answer": out}))
"#;

/// Windows: a TopMost WinForms window (PowerShell ships with Windows). Prints allow|deny|timeout|<text>.
const PS_DIALOG: &str = r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$q = [Console]::In.ReadToEnd() | ConvertFrom-Json
$f = New-Object System.Windows.Forms.Form
$f.Text = $q.title; $f.TopMost = $true; $f.Width = 520; $f.Height = 240; $f.StartPosition = 'CenterScreen'
$f.FormBorderStyle = 'FixedDialog'; $f.MaximizeBox = $false; $f.MinimizeBox = $false
$l = New-Object System.Windows.Forms.Label
$l.Text = $q.text; $l.Left = 12; $l.Top = 12; $l.Width = 480; $l.Height = 110
$f.Controls.Add($l)
$t = $null
if ($q.want_text) { $t = New-Object System.Windows.Forms.TextBox; $t.Left = 12; $t.Top = 125; $t.Width = 480; $f.Controls.Add($t) }
$script:ans = 'closed'
$ok = New-Object System.Windows.Forms.Button; $ok.Text = $q.ok; $ok.Left = 270; $ok.Top = 160; $ok.Width = 110
$ok.Add_Click({ if ($t -ne $null) { $script:ans = 'text:' + $t.Text } else { $script:ans = 'allow' }; $f.Close() })
$no = New-Object System.Windows.Forms.Button; $no.Text = $q.no; $no.Left = 385; $no.Top = 160; $no.Width = 110
$no.Add_Click({ $script:ans = 'deny'; $f.Close() })
$f.Controls.Add($ok); $f.Controls.Add($no)
# Enter never approves: the refusing button has the focus and is the AcceptButton (a typed answer is sent with Enter)
if ($t -ne $null) { $f.AcceptButton = $ok; $f.ActiveControl = $t } else { $f.AcceptButton = $no; $f.ActiveControl = $no }
$tm = New-Object System.Windows.Forms.Timer; $tm.Interval = [int]($q.timeout * 1000)
$tm.Add_Tick({ $script:ans = 'timeout'; $tm.Stop(); $f.Close() }); $tm.Start()
$f.Add_Shown({ $f.Activate() })
[void]$f.ShowDialog()
[Console]::Out.Write($script:ans)
"#;

pub struct Human {
    pub channel: String,
    pub timeout: f64,
    elicit: Option<Elicit>,
    can_elicit: CanElicit,
    pub pending: PendingTable,
    /// questions open right now on a synchronous channel (legacy elicitation, desktop dialog)
    open: AtomicUsize,
}

struct OpenGuard<'a>(&'a AtomicUsize);
impl Drop for OpenGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn which(bin: &str) -> bool {
    let exts: &[&str] = if cfg!(windows) { &["", ".exe", ".cmd"] } else { &[""] };
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| exts.iter().any(|e| d.join(format!("{bin}{e}")).is_file()))).unwrap_or(false)
}

fn desktop_tool() -> Option<&'static str> {
    static T: OnceLock<Option<&'static str>> = OnceLock::new();
    *T.get_or_init(|| {
        if let Some(t) = env("GUI_GATE_DIALOG") {
            // tests / packagers pin the tool; "none" disables the desktop channel
            return match t.as_str() { "zenity" => Some("zenity"), "kdialog" => Some("kdialog"), "gtk" => Some("gtk"),
                                      "osascript" => Some("osascript"), "powershell" => Some("powershell"), _ => None };
        }
        if cfg!(target_os = "macos") {
            return if which("osascript") || std::path::Path::new("/usr/bin/osascript").exists() { Some("osascript") } else { None };
        }
        if cfg!(windows) {
            return if which("powershell") { Some("powershell") } else { None };
        }
        if which("zenity") {
            return Some("zenity");
        }
        if which("kdialog") {
            return Some("kdialog");
        }
        let ok = Command::new("python3").args(["-c", "import gi; gi.require_version('Gtk','3.0'); from gi.repository import Gtk"])
            .stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
        if ok { Some("gtk") } else { None }
    })
}

/// Is there a display a dialog can appear on?
fn has_display() -> bool {
    cfg!(target_os = "macos") || cfg!(windows) || env("DISPLAY").is_some() || env("WAYLAND_DISPLAY").is_some()
}

fn run_with_timeout(mut cmd: Command, timeout: f64, stdin: Option<String>) -> Option<(i32, String)> {
    if stdin.is_some() {
        cmd.stdin(Stdio::piped());
    }
    // its own process group, registered with the gate's child list: when the gate ends (client gone, signal) an open
    // dialog is killed with it instead of staying on screen with buttons that no longer do anything
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let dpid = child.id() as i64;
    dialog_pids().lock().unwrap_or_else(|e| e.into_inner()).insert(dpid);
    crate::proc::register(child.id());
    struct Unreg(i64);
    impl Drop for Unreg {
        fn drop(&mut self) {
            dialog_pids().lock().unwrap_or_else(|e| e.into_inner()).remove(&self.0);
            crate::proc::unregister(self.0 as u32);
        }
    }
    let _unreg = Unreg(dpid);
    if let (Some(inp), Some(mut w)) = (stdin, child.stdin.take()) {
        let _ = std::io::Write::write_all(&mut w, inp.as_bytes());
    }
    let mut out = child.stdout.take()?;
    let rd = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut out, &mut s);
        s
    });
    let t0 = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(st)) => return Some((st.code().unwrap_or(-1), rd.join().unwrap_or_default())),
            Ok(None) if t0.elapsed().as_secs_f64() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => return None,
        }
    }
}

/// Button labels per kind (positive, negative).
pub fn labels(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "answer" => ("Send", "Cancel"),
        "takeover" => ("I did it, continue", "No"),
        "ask_help" => ("I did this step, continue", "Try another way"),
        _ => ("Allow", "Deny"),
    }
}

fn esc_applescript(t: &str) -> String {
    t.replace('\\', "\\\\").replace('"', "\\\"")
}

/// The osascript program for a dialog (macOS). `display dialog ... giving up after T` returns "gave up:true".
pub fn applescript(text: &str, title: &str, kind: &str, timeout: f64) -> String {
    let (ok, no) = labels(kind);
    let ans = if kind == "answer" { " default answer \"\"" } else { "" };
    // Return never approves: the default button is the refusing one, except for a typed answer (sent with Return).
    // A typed answer comes back as "text:<answer>" so it can never be mistaken for allow / deny / timeout.
    let dflt = if kind == "answer" { ok } else { no };
    format!("try\nset r to display dialog \"{}\"{ans} with title \"{}\" buttons {{\"{no}\", \"{ok}\"}} default button \"{dflt}\" giving up after {}\n\
             if gave up of r then return \"timeout\"\nif button returned of r is \"{ok}\" then\n{}\nend if\nreturn \"deny\"\non error\nreturn \"deny\"\nend try",
            esc_applescript(text), esc_applescript(title), timeout.max(1.0) as i64,
            if kind == "answer" { "return \"text:\" & (text returned of r)" } else { "return \"allow\"" })
}

/// zenity arguments (Linux). A yes/no question defaults to the refusing button (--default-cancel): Enter or a stray
/// keystroke while the dialog takes focus never approves.
pub fn zenity_args(text: &str, title: &str, kind: &str, timeout: f64) -> Vec<String> {
    let (ok, no) = labels(kind);
    let mut a = vec![format!("--title={title}"), format!("--timeout={}", timeout as i64)];
    if kind == "answer" {
        a.extend(["--entry".to_string(), format!("--text={text}"), format!("--ok-label={ok}"), format!("--cancel-label={no}")]);
    } else {
        a.extend(["--question".to_string(), "--default-cancel".to_string(), format!("--text={text}"), format!("--ok-label={ok}"), format!("--cancel-label={no}")]);
    }
    a
}

/// A typed answer: empty = nothing given (denied), otherwise accepted as text.
fn typed(out: &str) -> (String, String) {
    let o = out.trim().to_string();
    if o.is_empty() { ("deny".into(), "denied".into()) } else { (o, "accepted".into()) }
}

/// What a desktop dialog helper's exit (None = killed at our timeout) means: (answer, outcome). The answer is
/// "allow" | "deny" | "timeout" | typed text; a typed text never collides with those words (it comes from the text
/// channel of the helper, not from its button channel). Anything unexpected is a refusal, never an approval.
pub fn dialog_result(tool: &str, res: Option<(i32, String)>, want_text: bool) -> (String, String) {
    let pair = |a: &str, o: &str| (a.to_string(), o.to_string());
    let Some((code, out)) = res else { return pair("timeout", "timeout") };
    match tool {
        // zenity: 0 OK, 1 Cancel (or the window closed: zenity does not tell them apart), 5 its own timeout
        "zenity" | "kdialog" => match code {
            0 if want_text => typed(&out),
            0 => pair("allow", "accepted"),
            1 => pair("deny", "denied"),
            5 if tool == "zenity" => pair("timeout", "timeout"),
            _ => pair("deny", "window_closed"),
        },
        "osascript" | "powershell" => {
            if code != 0 {
                return pair("deny", "channel_unavailable"); // the helper failed: nobody answered
            }
            let o = out.trim();
            if let Some(t) = o.strip_prefix("text:") {
                return if want_text { typed(t) } else { pair("deny", "denied") };
            }
            match o {
                "allow" if !want_text => pair("allow", "accepted"),
                "deny" => pair("deny", "denied"),
                "timeout" => pair("timeout", "timeout"),
                "closed" => pair("deny", "window_closed"),
                _ => pair("deny", "denied"),
            }
        }
        _ => {
            // the GTK helper prints one JSON line {"answer": allow|deny|timeout|closed} or {"answer": text, "typed": true}
            let v = out.lines().last().and_then(|l| serde_json::from_str::<Value>(l).ok());
            let Some(v) = v else { return pair("deny", "channel_unavailable") };
            let a = pystr(v.get("answer"));
            if v.get("typed").and_then(|t| t.as_bool()) == Some(true) {
                return if want_text { typed(&a) } else { pair("deny", "denied") };
            }
            match a.as_str() {
                "allow" if !want_text => pair("allow", "accepted"),
                "timeout" => pair("timeout", "timeout"),
                "closed" => pair("deny", "window_closed"),
                _ => pair("deny", "denied"),
            }
        }
    }
}

/// The JSON a PowerShell dialog reads on stdin (Windows).
pub fn ps_input(text: &str, title: &str, kind: &str, timeout: f64) -> Value {
    let (ok, no) = labels(kind);
    json!({"text": text, "title": title, "ok": ok, "no": no, "timeout": timeout.max(1.0), "want_text": kind == "answer"})
}

impl Human {
    pub fn new(channel: &str, timeout: f64, elicit: Option<Elicit>, can_elicit: CanElicit) -> Self {
        Human { channel: channel.into(), timeout, elicit, can_elicit, pending: PendingTable::default(), open: AtomicUsize::new(0) }
    }

    /// A person is being asked right now (an MRTR request is parked, or a dialog / legacy elicitation is open):
    /// the gate sends no automatic input until it ends, so the agent can never operate the approval itself.
    pub fn person_answering(&self) -> bool {
        self.open.load(Ordering::SeqCst) > 0 || self.pending.live() > 0
    }

    /// Questions open right now on a synchronous channel (desktop dialog, legacy elicitation).
    pub fn open_now(&self) -> usize {
        self.open.load(Ordering::SeqCst)
    }

    pub fn resolve_channel(&self) -> String {
        if self.channel != "auto" {
            return self.channel.clone();
        }
        if self.elicit.is_some() && (self.can_elicit)() {
            return "elicit".into();
        }
        if has_display() && desktop_tool().is_some() {
            return "desktop".into();
        }
        "chat".into()
    }

    /// Show a native dialog and wait: (answer, outcome), see dialog_result.
    fn desktop(&self, text: &str, kind: &str, title: &str) -> (String, String) {
        let t = self.timeout;
        let want_text = kind == "answer";
        let (ok, no) = labels(kind);
        match desktop_tool() {
            Some("zenity") => {
                let mut c = Command::new("zenity");
                c.args(zenity_args(text, title, kind, t));
                dialog_result("zenity", run_with_timeout(c, t + 10.0, None), want_text)
            }
            Some("kdialog") => {
                let mut c = Command::new("kdialog");
                c.arg("--title").arg(title);
                if want_text { c.arg("--inputbox").arg(text); } else { c.arg("--yesno").arg(text).arg("--yes-label").arg(ok).arg("--no-label").arg(no); }
                dialog_result("kdialog", run_with_timeout(c, t, None), want_text)
            }
            Some("osascript") => {
                let mut c = Command::new("osascript");
                c.arg("-e").arg(applescript(text, title, kind, t));
                dialog_result("osascript", run_with_timeout(c, t + 10.0, None), want_text)
            }
            Some("powershell") => {
                let mut c = Command::new("powershell");
                c.args(["-NoProfile", "-NonInteractive", "-STA", "-ExecutionPolicy", "Bypass", "-Command", PS_DIALOG]);
                dialog_result("powershell", run_with_timeout(c, t + 15.0, Some(ps_input(text, title, kind, t).to_string())), want_text)
            }
            Some(_) => {
                let q = json!({"text": text, "want_text": want_text, "timeout": t, "title": title, "ok": ok, "no": no});
                let mut c = Command::new("python3");
                c.args(["-c", GTK_DIALOG, &q.to_string()]);
                dialog_result("gtk", run_with_timeout(c, t + 10.0, None), want_text)
            }
            None => ("timeout".into(), "channel_unavailable".into()),
        }
    }

    /// Back-compat: want_text=true is kind "answer", otherwise "approve".
    pub fn ask(&self, text: &str, want_text: bool, title: &str) -> Value {
        self.ask_kind(if want_text { "answer" } else { "approve" }, text, title, json!({}))
    }

    /// Ask a person. `meta` (tool, target, reason ...) rides along to the client in the MRTR request.
    pub fn ask_kind(&self, kind: &str, text: &str, title: &str, meta: Value) -> Value {
        let kind = if KINDS.contains(&kind) { kind } else { "approve" };
        let ch = self.resolve_channel();
        let t0 = Instant::now();
        let mut outcome = String::new();
        let _open = if ch == "desktop" || (ch == "elicit" && !call_mrtr()) {
            self.open.fetch_add(1, Ordering::SeqCst);
            Some(OpenGuard(&self.open))
        } else {
            None
        };
        let ans: String = match ch.as_str() {
            "elicit" if call_mrtr() => {
                // a retry that carries the person's answer to this very request
                // bound to the effect AND to the task it was asked under (operator task version + session): an answer
                // given about an older task is not used, the person is asked again about the current one
                let effect_now = answer_binding(&meta);
                let mut stale = false;
                if let Some((k, a, _, eff, outcome)) = CALL.with(|x| x.borrow().answer.clone()) {
                    if k == kind && eff == effect_now {
                        let req = CALL.with(|x| { let mut c = x.borrow_mut(); c.used = true; c.req.clone() });
                        return json!({"answer": a, "channel": "elicit", "mrtr": true, "ms": round_to(waited_s() * 1000.0, 1), "outcome": outcome, "kind": kind, "req": req,
                                      "active_ms": active_s().map(|x| round_to(x * 1000.0, 1))});
                    }
                    if k == kind {
                        // the target / window / payload changed since the person answered: that answer is not reused
                        stale = true;
                        CALL.with(|x| { let mut c = x.borrow_mut(); c.answer = None; c.dropped = Some("effect_changed"); });
                    }
                }
                if CALL.with(|x| x.borrow().in_batch) {
                    // a batch would be re-run from its first step on retry: the step must be sent on its own
                    return json!({"answer": "batch", "channel": "elicit", "mrtr": true, "ms": 0.0});
                }
                let (binding, ask_key, task_version) = CALL.with(|x| { let c = x.borrow(); (c.binding.clone(), c.ask_key.clone(), c.task_version) });
                let deadline = now() + self.timeout;
                let state = self.pending.park(Pending { binding, kind: kind.into(), deadline, effect: effect_now, asked: now(), voided: false, reported: false, ask_key, task_version });
                let mut m = if meta.is_object() { meta } else { json!({}) };
                if stale {
                    m["asked_again"] = json!("the action's target, or the task it was asked under, changed since the earlier answer");
                }
                m["kind"] = json!(kind);
                m["title"] = json!(title);
                m["timeout_s"] = json!(self.timeout);
                m["deadline_ms"] = json!((deadline * 1000.0) as i64);
                let req = json!({"resultType": "input_required",
                    "inputRequests": {"human": {"method": "elicitation/create", "params": {
                        "mode": "form", "message": text, "requestedSchema": schema_with_options(kind, m.get("options")), "_meta": {META_KEY: m}}}},
                    "requestState": state});
                CALL.with(|x| x.borrow_mut().request = Some(req));
                return json!({"answer": "input_required", "channel": "elicit", "mrtr": true, "ms": 0.0, "stale_answer": stale, "kind": kind, "req": req_id(&state)});
            }
            "elicit" if self.elicit.is_some() => {
                let mut params = json!({"message": text, "requestedSchema": schema_for(kind)});
                if kind == "approve" {
                    // legacy clients: the pre-2026 boolean form older clients render
                    params["requestedSchema"] = json!({"type": "object", "properties": {"allow": {"type": "boolean", "title": "Allow this action?"}}, "required": ["allow"]});
                }
                params["_meta"] = json!({META_KEY: {"kind": kind, "title": title, "timeout_s": self.timeout}});
                match (self.elicit.as_ref().unwrap())(params, self.timeout) {
                    None => {
                        outcome = "timeout".into();
                        "timeout".into()
                    }
                    Some(r) => {
                        let (a, note, o) = map_elicit_o(kind, Some(&r));
                        outcome = o;
                        if !note.is_empty() {
                            CALL.with(|x| { let mut c = x.borrow_mut(); c.answer = Some((kind.into(), a.clone(), note.clone(), String::new(), outcome.clone())); });
                        }
                        a
                    }
                }
            }
            "deny" => {
                outcome = "unattended".into();
                "deny".into()
            }
            "desktop" => {
                let (a, o) = self.desktop(text, kind, if title.is_empty() { "gui-gate: a person is needed" } else { title });
                // fail closed (v2-sec): on a yes/no dialog only its own buttons count; anything else is a refusal
                // that is never read as "try another way" either
                let odd = kind != "answer" && !matches!(a.as_str(), "allow" | "deny" | "timeout");
                let a = if odd { "deny".to_string() } else { a };
                // ask_help's second button is "Try another way": an answer, not a refusal (as in the MRTR form)
                outcome = if odd { "denied".into() } else if kind == "ask_help" && a == "deny" && o == "denied" { "accepted".into() } else { o };
                a
            }
            _ => return json!({"answer": "chat", "channel": "chat", "ms": 0.0, "outcome": "channel_unavailable"}),
        };
        if outcome.is_empty() {
            outcome = if ans == "allow" { "accepted".into() } else { "denied".into() };
        }
        json!({"answer": ans, "channel": ch, "ms": round_to(t0.elapsed().as_secs_f64() * 1000.0, 1), "outcome": outcome, "kind": kind})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deny_channel_answers_at_once() {
        let h = Human::new("deny", 30.0, None, Arc::new(|| false));
        let r = h.ask("approve?", false, "");
        assert_eq!(r["answer"], "deny");
        assert_eq!(r["channel"], "deny");
        assert!(r["ms"].as_f64().unwrap() < 1000.0);
    }

    #[test]
    fn mrtr_parks_one_time_bound_requests() {
        let h = Human::new("elicit", 30.0, Some(Arc::new(|_, _| None)), Arc::new(|| true));
        set_call(CallCtx { mrtr: true, binding: "click|{}".into(), ..Default::default() });
        let r = h.ask_kind("takeover", "do it", "t", json!({"tool": "click"}));
        assert_eq!(r["answer"], "input_required");
        let c = take_call();
        let req = c.request.unwrap();
        assert_eq!(req["resultType"], "input_required");
        let p = &req["inputRequests"]["human"]["params"];
        assert_eq!(p["mode"], "form");
        assert_eq!(p["requestedSchema"]["properties"]["done"]["type"], "boolean");
        assert_eq!(p["_meta"][META_KEY]["kind"], "takeover");
        let st = req["requestState"].as_str().unwrap().to_string();
        assert!(st.len() >= 32);
        // wrong binding: invalid (and consumed)
        assert!(matches!(h.pending.resolve(&st, "click|{\"x\":1}", None), Resolved::Invalid(_)));
        assert!(matches!(h.pending.resolve(&st, "click|{}", None), Resolved::Invalid(_)));
        // a fresh one, answered
        set_call(CallCtx { mrtr: true, binding: "click|{}".into(), ..Default::default() });
        h.ask_kind("takeover", "do it", "t", json!({}));
        let st = take_call().request.unwrap()["requestState"].as_str().unwrap().to_string();
        let resp = json!({"human": {"action": "accept", "content": {"done": true}}});
        match h.pending.resolve(&st, "click|{}", Some(&resp)) {
            Resolved::Answer(k, a, _, _, o) => { assert_eq!(k, "takeover"); assert_eq!(a, "allow"); assert_eq!(o, "accepted"); }
            _ => panic!("expected an answer"),
        }
        assert!(matches!(h.pending.resolve(&st, "click|{}", Some(&resp)), Resolved::Invalid(_)), "one-time");
    }

    #[test]
    fn late_answer_is_a_timeout() {
        let h = Human::new("elicit", 0.0, Some(Arc::new(|_, _| None)), Arc::new(|| true));
        let st = h.pending.park(Pending { binding: "b".into(), kind: "approve".into(), deadline: now() - MRTR_GRACE_S - 1.0, effect: String::new(), asked: now(), voided: false, reported: false, ask_key: String::new(), task_version: 0 });
        let resp = json!({"human": {"action": "accept", "content": {"decision": "allow"}}});
        assert!(matches!(h.pending.resolve(&st, "b", Some(&resp)), Resolved::Late(_, _)));
    }

    #[test]
    fn only_positive_accepted_forms_count() {
        assert_eq!(map_elicit("approve", Some(&json!({"action": "accept", "content": {"decision": "allow"}}))).0, "allow");
        assert_eq!(map_elicit("approve", Some(&json!({"action": "accept", "content": {"allow": true}}))).0, "allow");
        assert_eq!(map_elicit("approve", Some(&json!({"action": "decline", "content": {"decision": "allow"}}))).0, "deny");
        assert_eq!(map_elicit("approve", Some(&json!({"action": "cancel"}))).0, "deny");
        assert_eq!(map_elicit("approve", None).0, "deny");
        assert_eq!(map_elicit("answer", Some(&json!({"action": "accept", "content": {"answer": " a.txt "}}))).0, "a.txt");
        assert_eq!(map_elicit("answer", Some(&json!({"action": "accept", "content": {"answer": ""}}))).0, "deny");
        assert_eq!(map_elicit("takeover", Some(&json!({"action": "accept", "content": {"done": false}}))).0, "deny");
        let (a, n) = map_elicit("ask_help", Some(&json!({"action": "accept", "content": {"choice": "try_other", "note": "use Ctrl+S"}})));
        assert_eq!((a.as_str(), n.as_str()), ("deny", "use Ctrl+S"));
        assert_eq!(map_elicit("ask_help", Some(&json!({"action": "accept", "content": {"choice": "did_it"}}))).0, "allow");
    }

    #[test]
    fn outcomes_say_why_a_request_ended() {
        let o = |r: Value| map_elicit_o("approve", Some(&r)).2;
        assert_eq!(o(json!({"action": "accept", "content": {"decision": "allow"}})), "accepted");
        assert_eq!(o(json!({"action": "accept", "content": {"decision": "deny"}})), "denied");
        assert_eq!(o(json!({"action": "decline"})), "declined");
        assert_eq!(o(json!({"action": "cancel"})), "cancelled");
        assert_eq!(map_elicit_o("approve", None).2, "no_response");
        // the client's reason (gui-pi: ElicitResult _meta) says why the form ended: a timeout is no answer, not "declined"
        let m = |a: &str, why: &str| json!({"action": a, "_meta": {META_KEY: {"reason": why}}});
        assert_eq!(o(m("decline", "timed_out")), "timeout");
        assert_eq!(o(m("decline", "expired")), "expired");
        assert_eq!(o(m("cancel", "closed")), "window_closed");
        assert_eq!(o(m("cancel", "cancelled")), "cancelled");
        assert_eq!(o(m("decline", "unavailable")), "channel_unavailable");
        assert_eq!(o(m("decline", "declined")), "declined");
        assert_eq!(o(m("decline", "whatever")), "declined");
        assert_eq!(o(json!({"action": "decline", "reason": "timed_out"})), "timeout");
        for (a, why) in [("decline", "timed_out"), ("decline", "expired"), ("cancel", "closed")] {
            assert_eq!(map_elicit_o("approve", Some(&m(a, why))).0, "deny", "never an approval");
            assert!(crate::server::NO_ANSWER_OUTCOMES.contains(&o(m(a, why)).as_str()));
        }
        // every one but accepted is a refusal
        for r in [json!({"action": "decline", "content": {"decision": "allow"}}), json!({"action": "cancel"}), json!({"action": "?"})] {
            assert_eq!(map_elicit_o("approve", Some(&r)).0, "deny");
        }
        let h = Human::new("deny", 30.0, None, Arc::new(|| false));
        assert_eq!(h.ask("x", false, "")["outcome"], "unattended");
        assert!(outcome_words("timeout", 120.0).contains("120 s"));
        assert_eq!(outcome_words("window_closed", 1.0), "closed the dialog without answering");
    }

    #[test]
    fn an_answer_about_another_effect_is_not_reused() {
        let h = Human::new("elicit", 30.0, Some(Arc::new(|_, _| None)), Arc::new(|| true));
        let e1 = json!({"hazard": "delete", "resources": ["path:/a"]});
        let e2 = json!({"hazard": "delete", "resources": ["path:/b"]});
        set_call(CallCtx { mrtr: true, binding: "k".into(), answer: Some(("approve".into(), "allow".into(), String::new(), e1.to_string(), "accepted".into())), ..Default::default() });
        let r = h.ask_kind("approve", "q", "t", json!({"effect": e1}));
        assert_eq!(r["answer"], "allow", "the same effect: the answer counts");
        take_call();
        set_call(CallCtx { mrtr: true, binding: "k".into(), answer: Some(("approve".into(), "allow".into(), String::new(), e1.to_string(), "accepted".into())), ..Default::default() });
        let r = h.ask_kind("approve", "q", "t", json!({"effect": e2}));
        assert_eq!(r["answer"], "input_required", "the target changed: asked again");
        assert_eq!(r["stale_answer"], true);
        let c = take_call();
        let st = c.request.unwrap()["requestState"].as_str().unwrap().to_string();
        // the new request is bound to the new effect
        match h.pending.resolve(&st, "k", Some(&json!({"human": {"action": "accept", "content": {"decision": "allow"}}}))) {
            Resolved::Answer(_, a, _, eff, _) => { assert_eq!(a, "allow"); assert_eq!(eff, e2.to_string()); }
            _ => panic!("expected an answer"),
        }
    }

    #[test]
    fn conflicting_or_ambiguous_forms_are_refusals() {
        let a = |k: &str, c: Value| map_elicit(k, Some(&json!({"action": "accept", "content": c}))).0;
        // the regression: allow:true next to decision:deny used to approve
        assert_eq!(a("approve", json!({"allow": true, "decision": "deny"})), "deny");
        assert_eq!(a("approve", json!({"decision": "deny", "allow": true})), "deny");
        assert_eq!(a("approve", json!({"decision": "allow", "allow": false})), "deny");
        assert_eq!(a("approve", json!({"decision": "allow", "allow": "true"})), "deny");
        assert_eq!(a("approve", json!({"decision": "Allow"})), "deny");
        assert_eq!(a("approve", json!({"decision": ["allow"]})), "deny");
        assert_eq!(a("approve", json!({"allow": 1})), "deny");
        assert_eq!(a("approve", json!({"decision": "allow", "choice": "deny"})), "deny");
        assert_eq!(a("approve", json!("allow")), "deny");
        assert_eq!(map_elicit_o("approve", Some(&json!({"action": "accept", "content": {"allow": true, "decision": "deny"}}))).2, "denied");
        assert_eq!(a("takeover", json!({"done": true, "decision": "deny"})), "deny");
        assert_eq!(a("takeover", json!({"done": "yes"})), "deny");
        assert_eq!(a("ask_help", json!({"choice": "did_it", "done": false})), "deny");
        assert_eq!(a("ask_help", json!({"choice": "did_it", "allow": false})), "deny");
        // unambiguous forms are unchanged
        assert_eq!(a("approve", json!({"decision": "allow"})), "allow");
        assert_eq!(a("approve", json!({"allow": true})), "allow");
        assert_eq!(a("approve", json!({"decision": "allow", "allow": true})), "allow");
        assert_eq!(a("takeover", json!({"done": true})), "allow");
        assert_eq!(a("ask_help", json!({"choice": "did_it", "note": "done"})), "allow");
        let (h, n) = map_elicit("ask_help", Some(&json!({"action": "accept", "content": {"choice": "hint", "note": "Ctrl+S"}})));
        assert_eq!((h.as_str(), n.as_str()), ("deny", "Ctrl+S"));
    }

    #[test]
    fn native_dialog_output_must_be_a_button() {
        assert_eq!(map_desktop("approve", "allow"), ("allow".to_string(), "accepted"));
        assert_eq!(map_desktop("approve", "deny"), ("deny".to_string(), "denied"));
        assert_eq!(map_desktop("approve", "closed"), ("deny".to_string(), "window_closed"));
        for odd in ["allow\ndeny", "Allow", "yes", "allow deny", "deny allow"] {
            assert_eq!(map_desktop("approve", odd).0, "deny", "{odd}");
            assert_eq!(map_desktop("takeover", odd).0, "deny", "{odd}");
        }
        assert_eq!(map_desktop("answer", "use Ctrl+S"), ("use Ctrl+S".to_string(), "accepted"));
    }

    #[test]
    fn an_answer_about_an_older_task_is_not_reused() {
        let h = Human::new("elicit", 30.0, Some(Arc::new(|_, _| None)), Arc::new(|| true));
        let e = json!({"tool": "click", "hazard": "delete"});
        let m1 = json!({"effect": e, "task_binding": "v1.r0"});
        let m2 = json!({"effect": e, "task_binding": "v2.r0"});
        set_call(CallCtx { mrtr: true, binding: "k".into(), answer: Some(("approve".into(), "allow".into(), String::new(), answer_binding(&m1), "accepted".into())), ..Default::default() });
        assert_eq!(h.ask_kind("approve", "q", "t", m1.clone())["answer"], "allow", "same task version: the answer counts");
        set_call(CallCtx { mrtr: true, binding: "k".into(), answer: Some(("approve".into(), "allow".into(), String::new(), answer_binding(&m1), "accepted".into())), ..Default::default() });
        let r = h.ask_kind("approve", "q", "t", m2);
        assert_eq!(r["answer"], "input_required", "a new task version: asked again");
        assert_eq!(r["stale_answer"], true);
        let _ = take_call();
    }

    #[test]
    fn nobody_but_the_person_or_the_timeout_ends_a_request() {
        let h = Human::new("elicit", 30.0, Some(Arc::new(|_, _| None)), Arc::new(|| true));
        let park = |h: &Human| {
            set_call(CallCtx { mrtr: true, binding: "click|{}".into(), ..Default::default() });
            h.ask_kind("approve", "q", "t", json!({}));
            take_call().request.unwrap()["requestState"].as_str().unwrap().to_string()
        };
        let allow = json!({"human": {"action": "accept", "content": {"decision": "allow"}}});
        // a reset voids the request but the person is still answering: input stays frozen
        let st = park(&h);
        assert_eq!(h.pending.void_all(), 1);
        assert!(h.person_answering(), "a reset must not withdraw the question");
        assert!(matches!(h.pending.resolve(&st, "click|{}", Some(&allow)), Resolved::Invalid(_)), "voided: approves nothing");
        assert!(!h.person_answering(), "the person's answer ends it");
        // a call with another binding, or a retry without an answer, burns it but does not end it
        let st = park(&h);
        assert!(matches!(h.pending.resolve(&st, "click|{\"x\":1}", Some(&allow)), Resolved::Invalid(_)));
        assert!(h.person_answering());
        assert!(matches!(h.pending.resolve(&st, "click|{}", Some(&allow)), Resolved::Invalid(_)), "burnt");
        assert!(!h.person_answering());
        let st = park(&h);
        assert!(matches!(h.pending.resolve(&st, "click|{}", None), Resolved::Invalid(_)));
        assert!(matches!(h.pending.resolve(&st, "click|{}", None), Resolved::Invalid(_)));
        assert!(h.person_answering(), "answer-less retries never end it");
        assert!(matches!(h.pending.resolve(&st, "click|{}", Some(&allow)), Resolved::Invalid(_)));
        assert!(!h.person_answering());
        // a timed-out request is over by itself
        let st = h.pending.park(Pending { binding: "b".into(), kind: "approve".into(), deadline: now() - MRTR_GRACE_S - 1.0, effect: String::new(), asked: now(), voided: false, reported: false, ask_key: String::new(), task_version: 0 });
        assert!(!h.person_answering());
        assert!(matches!(h.pending.resolve(&st, "b", Some(&allow)), Resolved::Late(_, _)));
    }

    #[test]
    fn typed_answers_never_collide_with_button_words() {
        // a person who types "timeout" / "allow" / "closed" gave that text as their answer, nothing else
        for w in ["timeout", "allow", "closed", "deny"] {
            assert_eq!(dialog_result("zenity", Some((0, format!("{w}\n"))), true), (w.to_string(), "accepted".to_string()));
            assert_eq!(dialog_result("osascript", Some((0, format!("text:{w}"))), true), (w.to_string(), "accepted".to_string()));
            assert_eq!(dialog_result("powershell", Some((0, format!("text:{w}"))), true), (w.to_string(), "accepted".to_string()));
            assert_eq!(dialog_result("gtk", Some((0, json!({"answer": w, "typed": true}).to_string())), true), (w.to_string(), "accepted".to_string()));
        }
        assert_eq!(dialog_result("osascript", Some((0, "text:".into())), true).1, "denied");
        // the button channel
        assert_eq!(dialog_result("osascript", Some((0, "timeout".into())), true), ("timeout".into(), "timeout".into()));
        assert_eq!(dialog_result("osascript", Some((0, "allow".into())), false), ("allow".into(), "accepted".into()));
        assert_eq!(dialog_result("powershell", Some((0, "closed".into())), false), ("deny".into(), "window_closed".into()));
        assert_eq!(dialog_result("gtk", Some((0, "{\"answer\": \"closed\"}".into())), false), ("deny".into(), "window_closed".into()));
        assert_eq!(dialog_result("zenity", Some((5, String::new())), false), ("timeout".into(), "timeout".into()));
        assert_eq!(dialog_result("zenity", None, false), ("timeout".into(), "timeout".into()));
        // a typed text on a yes/no dialog, or "allow" on a text dialog, is never an approval
        assert_eq!(dialog_result("osascript", Some((0, "text:allow".into())), false).0, "deny");
        assert_eq!(dialog_result("osascript", Some((0, "allow".into())), true).0, "deny");
        // a helper that failed is nobody answering, not the person saying no (no denial record)
        assert_eq!(dialog_result("osascript", Some((1, String::new())), false), ("deny".into(), "channel_unavailable".into()));
        assert_eq!(dialog_result("powershell", Some((2, String::new())), false), ("deny".into(), "channel_unavailable".into()));
        assert_eq!(dialog_result("gtk", Some((1, "Traceback ...".into())), false), ("deny".into(), "channel_unavailable".into()));
    }

    #[test]
    fn dialogs_default_to_the_refusing_button() {
        for k in ["approve", "takeover", "ask_help"] {
            let z = zenity_args("Delete x?", "t", k, 60.0);
            assert!(z.contains(&"--default-cancel".to_string()), "{k}: {z:?}");
            let (_, no) = labels(k);
            assert!(applescript("Delete x?", "t", k, 60.0).contains(&format!("default button \"{no}\"")), "{k}");
        }
        assert!(!zenity_args("Which?", "t", "answer", 60.0).contains(&"--default-cancel".to_string()));
        assert!(applescript("Which?", "t", "answer", 60.0).contains("default button \"Send\""));
        assert!(applescript("Which?", "t", "answer", 60.0).contains("return \"text:\" & (text returned of r)"));
        assert!(PS_DIALOG.contains("$f.AcceptButton = $no; $f.ActiveControl = $no"));
        assert!(PS_DIALOG.contains("'text:' + $t.Text"));
        assert!(GTK_DIALOG.contains("set_default_response(1 if entry is not None else 2)"));
    }

    #[test]
    fn expired_requests_are_reported_once_and_still_resolve_late() {
        let h = Human::new("elicit", 30.0, Some(Arc::new(|_, _| None)), Arc::new(|| true));
        let dead = h.pending.park(Pending { binding: "b".into(), kind: "approve".into(), deadline: now() - MRTR_GRACE_S - 1.0, effect: String::new(), asked: now() - 40.0, voided: false, reported: false, ask_key: String::new(), task_version: 0 });
        let live = h.pending.park(Pending { binding: "c".into(), kind: "takeover".into(), deadline: now() + 60.0, effect: String::new(), asked: now(), voided: false, reported: false, ask_key: String::new(), task_version: 0 });
        let ex = h.pending.sweep_expired();
        assert_eq!(ex.len(), 1);
        assert_eq!(ex[0].0, req_id(&dead));
        assert_eq!(ex[0].1.kind, "approve");
        assert!(h.pending.sweep_expired().is_empty(), "reported once");
        assert_eq!(h.pending.live(), 1);
        // a retry after the report is still a timeout, never an approval
        let resp = json!({"human": {"action": "accept", "content": {"decision": "allow"}}});
        assert!(matches!(h.pending.resolve(&dead, "b", Some(&resp)), Resolved::Late(_, _)));
        let d = h.pending.drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].0, req_id(&live));
        assert_eq!(h.pending.len(), 0);
    }

    #[test]
    fn ask_help_another_way_is_an_answer_not_a_refusal() {
        let o = |c: Value| map_elicit_o("ask_help", Some(&json!({"action": "accept", "content": c})));
        assert_eq!(o(json!({"choice": "try_other"})), ("deny".to_string(), String::new(), "accepted".to_string()));
        assert_eq!(o(json!({"choice": "hint", "note": "Ctrl+S"})), ("deny".to_string(), "Ctrl+S".to_string(), "accepted".to_string()));
        assert_eq!(o(json!({"choice": "did_it"})).0, "allow");
        assert_eq!(o(json!({"choice": "?"})).2, "denied");
        assert_eq!(map_elicit_o("ask_help", Some(&json!({"action": "decline"}))).2, "declined");
        // other kinds unchanged
        assert_eq!(map_elicit_o("approve", Some(&json!({"action": "accept", "content": {"decision": "deny"}}))).2, "denied");
    }

    #[test]
    fn a_voided_request_keeps_the_freeze_until_its_timeout_is_swept() {
        let h = Human::new("elicit", 30.0, Some(Arc::new(|_, _| None)), Arc::new(|| true));
        let live = h.pending.park(Pending { binding: "c".into(), kind: "approve".into(), deadline: now() + 60.0, effect: String::new(), asked: now(), voided: false, reported: false, ask_key: String::new(), task_version: 0 });
        let (n, v) = h.pending.void_live();
        assert_eq!((n, v.len()), (1, 1));
        assert_eq!(v[0].0, req_id(&live));
        assert!(h.person_answering(), "a reset does not end the freeze");
        assert!(h.pending.void_live().1.is_empty(), "a second reset does not void (or log) it again");
        assert!(h.pending.sweep_expired().is_empty(), "not expired yet");
        // expired: swept once (logged as its timeout) and the freeze is over
        let dead = h.pending.park(Pending { binding: "d".into(), kind: "approve".into(), deadline: now() - MRTR_GRACE_S - 1.0, effect: String::new(), asked: now() - 40.0, voided: true, reported: false, ask_key: String::new(), task_version: 0 });
        let ex = h.pending.sweep_expired();
        assert_eq!(ex.len(), 1);
        assert!(ex[0].1.voided);
        assert_eq!(ex[0].0, req_id(&dead));
        let allow = json!({"human": {"action": "accept", "content": {"decision": "allow"}}});
        assert!(matches!(h.pending.resolve(&dead, "d", Some(&allow)), Resolved::Invalid(_)), "voided: never an approval");
        assert!(matches!(h.pending.resolve(&live, "c", Some(&allow)), Resolved::Invalid(_)), "voided: never an approval");
        assert!(!h.person_answering());
    }

    #[test]
    fn conflicting_ask_help_forms_are_not_another_way() {
        let o = |c: Value| map_elicit_o("ask_help", Some(&json!({"action": "accept", "content": c})));
        assert_eq!(o(json!({"choice": "hint", "done": true})).2, "denied");
        assert_eq!(o(json!({"choice": "try_other", "allow": true})).2, "denied");
        assert_eq!(o(json!({"choice": "try_other"})).2, "accepted");
    }

    #[test]
    fn mrtr_answer_use_is_tracked() {
        let h = Human::new("elicit", 30.0, Some(Arc::new(|_, _| None)), Arc::new(|| true));
        let e = json!({"hazard": "delete"});
        set_call(CallCtx { mrtr: true, binding: "k".into(), req: "abc".into(),
                           answer: Some(("approve".into(), "allow".into(), String::new(), e.to_string(), "accepted".into())), ..Default::default() });
        let r = h.ask_kind("approve", "q", "t", json!({"effect": e}));
        assert_eq!(r["req"], "abc");
        let c = take_call();
        assert!(c.used && c.dropped.is_none());
        set_call(CallCtx { mrtr: true, binding: "k".into(), req: "abc".into(),
                           answer: Some(("approve".into(), "allow".into(), String::new(), e.to_string(), "accepted".into())), ..Default::default() });
        let r = h.ask_kind("approve", "q", "t", json!({"effect": {"hazard": "send"}}));
        assert_eq!(r["answer"], "input_required");
        assert!(r["req"].as_str().unwrap().len() >= 16, "the new request has its own id");
        assert!(take_request().is_some());
        let c = take_call();
        assert!(!c.used);
        assert_eq!(c.dropped, Some("effect_changed"));
        set_call(CallCtx { mrtr: true, answer: Some(("approve".into(), "allow".into(), String::new(), String::new(), "accepted".into())), ..Default::default() });
        mark_dropped("frozen");
        assert_eq!(take_call().dropped, Some("frozen"));
    }

    #[test]
    fn native_dialog_programs() {
        let a = applescript("Delete \"x\"?", "gui-gate", "approve", 120.0);
        assert!(a.contains("giving up after 120"));
        assert!(a.contains("Delete \\\"x\\\"?"));
        assert!(a.contains("return \"timeout\""));
        let q = applescript("Which file?", "t", "answer", 30.0);
        assert!(q.contains("default answer \"\""));
        let p = ps_input("Take over", "t", "takeover", 60.0);
        assert_eq!(p["ok"], "I did it, continue");
        assert_eq!(p["want_text"], false);
        assert!(PS_DIALOG.contains("TopMost") && PS_DIALOG.contains("'timeout'"));
    }
}
