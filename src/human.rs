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
//! A timeout, a declined / cancelled form or a closed dialog is never an approval.
use crate::util::*;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::process::{Command, Stdio};
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
    /// the person's answer this retry carries (kind, mapped answer, note)
    pub answer: Option<(String, String, String)>,
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
}

/// The gate's table of MRTR requests: one-time, unguessable ids, bound to the call and its deadline.
#[derive(Default)]
pub struct PendingTable {
    map: Mutex<HashMap<String, Pending>>,
}

pub enum Resolved {
    /// (kind, mapped answer, note)
    Answer(String, String, String),
    /// the retry came back after the deadline: a timeout, never an approval
    Late(String),
    Invalid(&'static str),
}

impl PendingTable {
    pub fn park(&self, p: Pending) -> String {
        let id = format!("gg-{}", rand_hex(16));
        let mut m = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let t = now();
        m.retain(|_, v| v.deadline + 600.0 > t); // forget long-dead requests
        m.insert(id.clone(), p);
        id
    }

    /// Consume a requestState from a retry and map the person's ElicitResult.
    pub fn resolve(&self, state: &str, binding: &str, responses: Option<&Value>) -> Resolved {
        let p = { self.map.lock().unwrap_or_else(|e| e.into_inner()).remove(state) };
        let Some(p) = p else { return Resolved::Invalid("unknown or already used requestState") };
        if p.binding != binding {
            return Resolved::Invalid("requestState belongs to a different tool call");
        }
        if now() > p.deadline + MRTR_GRACE_S {
            return Resolved::Late(p.kind);
        }
        let r = responses.and_then(|r| r.get("human"));
        let (ans, note) = map_elicit(&p.kind, r);
        Resolved::Answer(p.kind, ans, note)
    }

    pub fn len(&self) -> usize {
        self.map.lock().map(|m| m.len()).unwrap_or(0)
    }
}

/// The form for each kind (MCP restricted JSON Schema: flat object of primitives).
pub fn schema_for(kind: &str) -> Value {
    match kind {
        "answer" => json!({"type": "object", "properties": {"answer": {"type": "string", "title": "Your answer"}}, "required": ["answer"]}),
        "takeover" => json!({"type": "object", "properties": {"done": {"type": "boolean", "title": "I did it, continue",
            "description": "Do the step yourself in the app, then confirm here."}}, "required": ["done"]}),
        "ask_help" => json!({"type": "object", "properties": {
            "choice": {"type": "string", "title": "What now?", "enum": ["did_it", "try_other"],
                       "enumNames": ["I did this step myself, continue", "Let the agent try another way"]},
            "note": {"type": "string", "title": "A hint for the agent (optional)"}}, "required": ["choice"]}),
        _ => json!({"type": "object", "properties": {"decision": {"type": "string", "title": "Allow this action?",
            "enum": ["allow", "deny"], "enumNames": ["Allow", "Deny"]}}, "required": ["decision"]}),
    }
}

/// ElicitResult -> (answer, note). Anything but an accepted, positive form is a refusal.
pub fn map_elicit(kind: &str, r: Option<&Value>) -> (String, String) {
    let Some(r) = r else { return ("deny".into(), String::new()) };
    if s(r, "action") != "accept" {
        return ("deny".into(), String::new());
    }
    let c = r.get("content").cloned().unwrap_or(json!({}));
    match kind {
        "answer" => {
            let a = pystr(c.get("answer")).trim().to_string();
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
out = {1: "allow", 2: "deny", 3: "timeout"}.get(r, "deny")
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
$script:ans = 'deny'
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
        Human { channel: channel.into(), timeout, elicit, can_elicit, pending: PendingTable::default() }
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
                    Some(_) => "deny".into(),
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
                    Some(_) => "deny".into(),
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
            None => "timeout".into(),
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
        let ans: String = match ch.as_str() {
            "elicit" if call_mrtr() => {
                // a retry that carries the person's answer to this very request
                if let Some((k, a, _)) = CALL.with(|x| x.borrow().answer.clone()) {
                    if k == kind {
                        return json!({"answer": a, "channel": "elicit", "mrtr": true, "ms": 0.0});
                    }
                }
                if CALL.with(|x| x.borrow().in_batch) {
                    // a batch would be re-run from its first step on retry: the step must be sent on its own
                    return json!({"answer": "batch", "channel": "elicit", "mrtr": true, "ms": 0.0});
                }
                let binding = CALL.with(|x| x.borrow().binding.clone());
                let deadline = now() + self.timeout;
                let state = self.pending.park(Pending { binding, kind: kind.into(), deadline });
                let mut m = if meta.is_object() { meta } else { json!({}) };
                m["kind"] = json!(kind);
                m["title"] = json!(title);
                m["timeout_s"] = json!(self.timeout);
                m["deadline_ms"] = json!((deadline * 1000.0) as i64);
                let req = json!({"resultType": "input_required",
                    "inputRequests": {"human": {"method": "elicitation/create", "params": {
                        "mode": "form", "message": text, "requestedSchema": schema_for(kind), "_meta": {META_KEY: m}}}},
                    "requestState": state});
                CALL.with(|x| x.borrow_mut().request = Some(req));
                return json!({"answer": "input_required", "channel": "elicit", "mrtr": true, "ms": 0.0});
            }
            "elicit" if self.elicit.is_some() => {
                let mut params = json!({"message": text, "requestedSchema": schema_for(kind)});
                if kind == "approve" {
                    // legacy clients: the pre-2026 boolean form older clients render
                    params["requestedSchema"] = json!({"type": "object", "properties": {"allow": {"type": "boolean", "title": "Allow this action?"}}, "required": ["allow"]});
                }
                params["_meta"] = json!({META_KEY: {"kind": kind, "title": title, "timeout_s": self.timeout}});
                match (self.elicit.as_ref().unwrap())(params, self.timeout) {
                    None => "timeout".into(),
                    Some(r) => {
                        let (a, note) = map_elicit(kind, Some(&r));
                        if !note.is_empty() {
                            CALL.with(|x| { let mut c = x.borrow_mut(); c.answer = Some((kind.into(), a.clone(), note.clone())); });
                        }
                        a
                    }
                }
            }
            "deny" => "deny".into(),
            "desktop" => self.desktop(text, kind, if title.is_empty() { "gui-gate: a person is needed" } else { title }),
            _ => return json!({"answer": "chat", "channel": "chat", "ms": 0.0}),
        };
        json!({"answer": ans, "channel": ch, "ms": round_to(t0.elapsed().as_secs_f64() * 1000.0, 1)})
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
            Resolved::Answer(k, a, _) => { assert_eq!(k, "takeover"); assert_eq!(a, "allow"); }
            _ => panic!("expected an answer"),
        }
        assert!(matches!(h.pending.resolve(&st, "click|{}", Some(&resp)), Resolved::Invalid(_)), "one-time");
    }

    #[test]
    fn late_answer_is_a_timeout() {
        let h = Human::new("elicit", 0.0, Some(Arc::new(|_, _| None)), Arc::new(|| true));
        let st = h.pending.park(Pending { binding: "b".into(), kind: "approve".into(), deadline: now() - MRTR_GRACE_S - 1.0 });
        let resp = json!({"human": {"action": "accept", "content": {"decision": "allow"}}});
        assert!(matches!(h.pending.resolve(&st, "b", Some(&resp)), Resolved::Late(_)));
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
