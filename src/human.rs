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
}

thread_local! {
    static WAITED: RefCell<f64> = const { RefCell::new(0.0) };
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
        let p = { self.map.lock().unwrap_or_else(|e| e.into_inner()).remove(state) };
        let Some(p) = p else { return Resolved::Invalid("unknown or already used requestState") };
        if sign_state(parts[1], parts[2], &p.binding) != parts[3] {
            return Resolved::Invalid("requestState signature does not match");
        }
        if p.binding != binding {
            return Resolved::Invalid("requestState belongs to a different tool call");
        }
        WAITED.with(|w| *w.borrow_mut() = (now() - p.asked).max(0.0));
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

    /// A reset from the client: every parked request is void.
    pub fn clear(&self) -> usize {
        let mut m = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let n = m.len();
        m.clear();
        n
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
            sc["properties"]["choice"] = json!({"type": "string", "title": "Pick one", "enum": o});
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
        "timeout" => format!("did not answer within {} s", timeout as i64),
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
        "decline" => return ("deny".into(), String::new(), "declined".into()),
        "cancel" => return ("deny".into(), String::new(), "cancelled".into()),
        _ => return ("deny".into(), String::new(), "no_response".into()),
    }
    let (a, n) = map_accepted(kind, r);
    let o = if a == "deny" { "denied" } else { "accepted" };
    (a, n, o.into())
}

fn map_accepted(kind: &str, r: &Value) -> (String, String) {
    let c = r.get("content").cloned().unwrap_or(json!({}));
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
        // approve: the enum form, or the pre-2026 boolean form some clients still send
        _ => (if s(&c, "decision") == "allow" || c.get("allow").and_then(|v| v.as_bool()) == Some(true) { "allow".into() } else { "deny".into() },
              String::new()),
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
GLib.timeout_add_seconds(int(q.get("timeout", 120)), lambda: (d.response(3), False)[1])
d.show_all(); d.present()
r = d.run()
out = {1: "allow", 2: "deny", 3: "timeout"}.get(r, "closed")
if entry is not None and r == 1:
    out = entry.get_text() or "deny"
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
$ok.Add_Click({ if ($t -ne $null) { $script:ans = $t.Text; if ($script:ans -eq '') { $script:ans = 'deny' } } else { $script:ans = 'allow' }; $f.Close() })
$no = New-Object System.Windows.Forms.Button; $no.Text = $q.no; $no.Left = 385; $no.Top = 160; $no.Width = 110
$no.Add_Click({ $script:ans = 'deny'; $f.Close() })
$f.Controls.Add($ok); $f.Controls.Add($no)
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
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let dpid = child.id() as i64;
    dialog_pids().lock().unwrap_or_else(|e| e.into_inner()).insert(dpid);
    struct Unreg(i64);
    impl Drop for Unreg {
        fn drop(&mut self) {
            dialog_pids().lock().unwrap_or_else(|e| e.into_inner()).remove(&self.0);
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
    format!("try\nset r to display dialog \"{}\"{ans} with title \"{}\" buttons {{\"{no}\", \"{ok}\"}} default button \"{ok}\" giving up after {}\n\
             if gave up of r then return \"timeout\"\nif button returned of r is \"{ok}\" then\n{}\nend if\nreturn \"deny\"\non error\nreturn \"deny\"\nend try",
            esc_applescript(text), esc_applescript(title), timeout.max(1.0) as i64,
            if kind == "answer" { "if text returned of r is \"\" then return \"deny\"\nreturn text returned of r" } else { "return \"allow\"" })
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

    fn desktop(&self, text: &str, kind: &str, title: &str) -> String {
        let t = self.timeout;
        let want_text = kind == "answer";
        let (ok, no) = labels(kind);
        let text_or = |out: String| { let o = out.trim().to_string(); if o.is_empty() { "deny".to_string() } else { o } };
        match desktop_tool() {
            Some("zenity") => {
                let mut c = Command::new("zenity");
                c.arg(format!("--title={title}")).arg(format!("--timeout={}", t as i64));
                if want_text {
                    c.args(["--entry", &format!("--text={text}"), &format!("--ok-label={ok}"), &format!("--cancel-label={no}")]);
                } else {
                    c.args(["--question", &format!("--text={text}"), &format!("--ok-label={ok}"), &format!("--cancel-label={no}")]);
                }
                match run_with_timeout(c, t + 10.0, None) {
                    Some((0, out)) if want_text => text_or(out),
                    Some((0, _)) => "allow".into(),
                    Some((5, _)) | None => "timeout".into(),
                    Some((1, _)) => "deny".into(),
                    Some(_) => "closed".into(),
                }
            }
            Some("kdialog") => {
                let mut c = Command::new("kdialog");
                c.arg("--title").arg(title);
                if want_text { c.arg("--inputbox").arg(text); } else { c.arg("--yesno").arg(text).arg("--yes-label").arg(ok).arg("--no-label").arg(no); }
                match run_with_timeout(c, t, None) {
                    Some((0, out)) if want_text => text_or(out),
                    Some((0, _)) => "allow".into(),
                    None => "timeout".into(),
                    Some((1, _)) => "deny".into(),
                    Some(_) => "closed".into(),
                }
            }
            Some("osascript") => {
                let mut c = Command::new("osascript");
                c.arg("-e").arg(applescript(text, title, kind, t));
                match run_with_timeout(c, t + 10.0, None) {
                    Some((0, out)) => { let o = out.trim().to_string(); if o.is_empty() { "deny".into() } else { o } }
                    None => "timeout".into(),
                    Some(_) => "deny".into(),
                }
            }
            Some("powershell") => {
                let mut c = Command::new("powershell");
                c.args(["-NoProfile", "-NonInteractive", "-STA", "-ExecutionPolicy", "Bypass", "-Command", PS_DIALOG]);
                match run_with_timeout(c, t + 15.0, Some(ps_input(text, title, kind, t).to_string())) {
                    Some((0, out)) => { let o = out.trim().to_string(); if o.is_empty() { "deny".into() } else { o } }
                    None => "timeout".into(),
                    Some(_) => "deny".into(),
                }
            }
            Some(_) => {
                let q = json!({"text": text, "want_text": want_text, "timeout": t, "title": title, "ok": ok, "no": no});
                let mut c = Command::new("python3");
                c.args(["-c", GTK_DIALOG, &q.to_string()]);
                match run_with_timeout(c, t + 10.0, None) {
                    Some((_, out)) => out.lines().last().and_then(|l| serde_json::from_str::<Value>(l).ok())
                        .map(|v| pystr(v.get("answer"))).filter(|a| !a.is_empty()).unwrap_or_else(|| "deny".into()),
                    None => "timeout".into(),
                }
            }
            None => "unavailable".into(),
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
                let effect_now = meta.get("effect").map(|e| e.to_string()).unwrap_or_default();
                let mut stale = false;
                if let Some((k, a, _, eff, outcome)) = CALL.with(|x| x.borrow().answer.clone()) {
                    if k == kind && eff == effect_now {
                        return json!({"answer": a, "channel": "elicit", "mrtr": true, "ms": round_to(waited_s() * 1000.0, 1), "outcome": outcome, "kind": kind});
                    }
                    if k == kind {
                        // the target / window / payload changed since the person answered: that answer is not reused
                        stale = true;
                        CALL.with(|x| x.borrow_mut().answer = None);
                    }
                }
                if CALL.with(|x| x.borrow().in_batch) {
                    // a batch would be re-run from its first step on retry: the step must be sent on its own
                    return json!({"answer": "batch", "channel": "elicit", "mrtr": true, "ms": 0.0});
                }
                let binding = CALL.with(|x| x.borrow().binding.clone());
                let deadline = now() + self.timeout;
                let state = self.pending.park(Pending { binding, kind: kind.into(), deadline, effect: effect_now, asked: now() });
                let mut m = if meta.is_object() { meta } else { json!({}) };
                if stale {
                    m["asked_again"] = json!("the action's target changed since the earlier answer");
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
                return json!({"answer": "input_required", "channel": "elicit", "mrtr": true, "ms": 0.0, "stale_answer": stale});
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
                let a = self.desktop(text, kind, if title.is_empty() { "gui-gate: a person is needed" } else { title });
                let (a, o) = match a.as_str() {
                    "allow" => (a.clone(), "accepted"),
                    "deny" => (a.clone(), "denied"),
                    "timeout" => (a.clone(), "timeout"),
                    "closed" => ("deny".to_string(), "window_closed"),
                    "unavailable" => ("timeout".to_string(), "channel_unavailable"),
                    _ => (a.clone(), "accepted"), // a typed answer
                };
                outcome = o.into();
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
        let st = h.pending.park(Pending { binding: "b".into(), kind: "approve".into(), deadline: now() - MRTR_GRACE_S - 1.0, effect: String::new(), asked: now() });
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
