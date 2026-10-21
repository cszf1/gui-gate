//! Hand a decision to a person.
//!
//! Channels, tried in this order when channel="auto":
//!   elicit   MCP elicitation/create to the client (clients that declare the `elicitation` capability)
//!   desktop  a small always-on-top dialog on the machine's own display (zenity / kdialog / GTK via python3)
//!   deny     unattended runs (benchmarks): every request is answered "deny" at once
//!   chat     nothing the gate can show itself: the refusal tells the agent to ask its user
//! ask() returns {"answer": "allow"|"deny"|"timeout"|<text>, "channel": ..., "ms": ...}.
use crate::util::*;
use serde_json::{json, Value};
use std::process::{Command, Stdio};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

pub type Elicit = Arc<dyn Fn(Value, f64) -> Option<Value> + Send + Sync>;
pub type CanElicit = Arc<dyn Fn() -> bool + Send + Sync>;

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
    d.add_button("Send", 1); d.add_button("Cancel", 2)
else:
    d.add_button("Allow", 1); d.add_button("Deny", 2)
GLib.timeout_add_seconds(int(q.get("timeout", 120)), lambda: (d.response(3), False)[1])
d.show_all(); d.present()
r = d.run()
out = {1: "allow", 2: "deny", 3: "timeout"}.get(r, "deny")
if entry is not None and r == 1:
    out = entry.get_text() or "allow"
print(json.dumps({"answer": out}))
"#;

pub struct Human {
    pub channel: String,
    pub timeout: f64,
    elicit: Option<Elicit>,
    can_elicit: CanElicit,
}

fn which(bin: &str) -> bool {
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file())).unwrap_or(false)
}

fn desktop_tool() -> Option<&'static str> {
    static T: OnceLock<Option<&'static str>> = OnceLock::new();
    *T.get_or_init(|| {
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

fn run_with_timeout(mut cmd: Command, timeout: f64) -> Option<(i32, String)> {
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
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

impl Human {
    pub fn new(channel: &str, timeout: f64, elicit: Option<Elicit>, can_elicit: CanElicit) -> Self {
        Human { channel: channel.into(), timeout, elicit, can_elicit }
    }

    pub fn resolve_channel(&self) -> String {
        if self.channel != "auto" {
            return self.channel.clone();
        }
        if self.elicit.is_some() && (self.can_elicit)() {
            return "elicit".into();
        }
        if (env("DISPLAY").is_some() || cfg!(target_os = "macos")) && desktop_tool().is_some() {
            return "desktop".into();
        }
        "chat".into()
    }

    fn desktop(&self, text: &str, want_text: bool, title: &str) -> String {
        let t = self.timeout;
        match desktop_tool() {
            Some("zenity") => {
                let mut c = Command::new("zenity");
                c.arg(format!("--title={title}")).arg(format!("--timeout={}", t as i64));
                if want_text {
                    c.args(["--entry", &format!("--text={text}")]);
                } else {
                    c.args(["--question", &format!("--text={text}"), "--ok-label=Allow", "--cancel-label=Deny"]);
                }
                match run_with_timeout(c, t + 10.0) {
                    Some((0, out)) if want_text => { let o = out.trim().to_string(); if o.is_empty() { "allow".into() } else { o } }
                    Some((0, _)) => "allow".into(),
                    Some((5, _)) | None => "timeout".into(),
                    Some(_) => "deny".into(),
                }
            }
            Some("kdialog") => {
                let mut c = Command::new("kdialog");
                c.arg("--title").arg(title);
                if want_text { c.arg("--inputbox").arg(text); } else { c.arg("--yesno").arg(text); }
                match run_with_timeout(c, t) {
                    Some((0, out)) if want_text => { let o = out.trim().to_string(); if o.is_empty() { "allow".into() } else { o } }
                    Some((0, _)) => "allow".into(),
                    None => "timeout".into(),
                    Some(_) => "deny".into(),
                }
            }
            Some(_) => {
                let q = json!({"text": text, "want_text": want_text, "timeout": t, "title": title});
                let mut c = Command::new("python3");
                c.args(["-c", GTK_DIALOG, &q.to_string()]);
                match run_with_timeout(c, t + 10.0) {
                    Some((_, out)) => out.lines().last().and_then(|l| serde_json::from_str::<Value>(l).ok())
                        .map(|v| pystr(v.get("answer"))).filter(|a| !a.is_empty()).unwrap_or_else(|| "deny".into()),
                    None => "timeout".into(),
                }
            }
            None => "timeout".into(),
        }
    }

    pub fn ask(&self, text: &str, want_text: bool, title: &str) -> Value {
        let ch = self.resolve_channel();
        let t0 = Instant::now();
        let ans: String = match ch.as_str() {
            "elicit" if self.elicit.is_some() => {
                let schema = if want_text {
                    json!({"type": "object", "properties": {"answer": {"type": "string", "title": "Your answer"}}, "required": ["answer"]})
                } else {
                    json!({"type": "object", "properties": {"allow": {"type": "boolean", "title": "Allow this action?"}}, "required": ["allow"]})
                };
                match (self.elicit.as_ref().unwrap())(json!({"message": text, "requestedSchema": schema}), self.timeout) {
                    None => "timeout".into(),
                    Some(r) if s(&r, "action") != "accept" => "deny".into(),
                    Some(r) => {
                        let c = r.get("content").cloned().unwrap_or(json!({}));
                        if want_text {
                            let a = pystr(c.get("answer"));
                            if a.is_empty() { "allow".into() } else { a }
                        } else if truthy(c.get("allow")) { "allow".into() } else { "deny".into() }
                    }
                }
            }
            "deny" => "deny".into(),
            "desktop" => self.desktop(text, want_text, if title.is_empty() { "gui-gate: a person is needed" } else { title }),
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
}
