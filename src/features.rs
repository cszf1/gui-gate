//! Turn one pending action into runtime-only features, and judge its risk from what it touches.
//! Everything here is derivable from what the agent and the driver already exchanged (plus cheap probes).
use crate::sidefx;
use crate::state::*;
use crate::util::*;
use indexmap::IndexMap;
use regex::Regex;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::OnceLock;

pub const GATED_TOOLS: &[&str] = &["click", "double_click", "right_click", "type_text", "set_value", "press_key", "hotkey",
    "scroll", "drag", "run_steps", "mouse_button_down", "invoke_menu", "browser_click", "browser_type", "browser_pointer",
    "browser_select", "browser_navigate", "browser_download", "browser_set_input_files", "kill_app", "run_script", "run_command"];
pub const OBS_TOOLS: &[&str] = &["get_window_state", "get_desktop_state", "zoom", "list_windows", "get_browser_state"];
const KEY_TOOLS: &[&str] = &["type_text", "press_key", "hotkey", "set_value"];

pub fn is_gated(t: &str) -> bool {
    GATED_TOOLS.contains(&t)
}

/// Driver tools that only read (Cua Driver 0.38.0 and the names other drivers use for the same reads). Every other
/// driver tool - launch_app, bring_to_front, a tool a newer driver adds - is an ACTION: it passes the single final
/// dispatch check in Gate::tool (and the freeze while a person is answering). Deny by classification: a tool is only
/// treated as read-only when it is on this list (or on the trusted launcher's GUI_GATE_READONLY_TOOLS list).
pub const READ_ONLY_TOOLS: &[&str] = &["get_window_state", "get_desktop_state", "get_browser_state", "list_windows", "list_apps",
    "screenshot", "zoom", "verify_state", "get_cursor_position", "get_screen_size", "list_displays", "list_screens",
    "check_permissions", "get_permissions", "get_config", "get_version", "get_accessibility_tree", "find_elements",
    "get_element", "get_focused_element", "get_clipboard_text_length"];

fn extra_read_only() -> &'static Vec<String> {
    static X: OnceLock<Vec<String>> = OnceLock::new();
    // a launcher setting (tests mark the fake driver's own harness tools); the agent cannot change the environment
    X.get_or_init(|| env("GUI_GATE_READONLY_TOOLS").map(|v| v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default())
}

pub fn is_read_only(t: &str) -> bool {
    READ_ONLY_TOOLS.contains(&t) || extra_read_only().iter().any(|x| x == t)
}

/// Lower-case words of an argument name: `screenshot_out_file`, `outputPath`, `save-to` -> [screenshot, out, file] ...
fn key_words(k: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut prev_lower = false;
    for ch in k.chars() {
        if !ch.is_alphanumeric() {
            if !cur.is_empty() { out.push(std::mem::take(&mut cur)); }
            prev_lower = false;
            continue;
        }
        if ch.is_uppercase() && prev_lower && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        prev_lower = ch.is_lowercase() || ch.is_ascii_digit();
        cur.extend(ch.to_lowercase());
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

/// Words that make an argument a place to WRITE to (a file, a path, a directory, "save to" ...).
const WRITE_WORDS: &[&str] = &["file", "filename", "filepath", "files", "path", "paths", "dir", "dirs", "directory", "folder",
    "dest", "destination", "outfile", "outpath", "outdir", "save", "saveto", "savepath", "write", "export", "dump"];
/// Words that make an argument about a capture (screenshot, image, recording ...).
const CAPTURE_WORDS: &[&str] = &["screenshot", "shot", "capture", "image", "img", "png", "jpeg", "jpg", "snapshot", "recording", "record", "video"];

fn looks_like_fs_path(v: &str) -> bool {
    let v = v.trim();
    let b = v.as_bytes();
    v.starts_with('/') || v.starts_with("~/") || v.starts_with("./") || v.starts_with("../") || v.starts_with("\\\\")
        || v.starts_with("file:") || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/'))
}

fn collect_write_args(prefix: &str, v: &Value, read_only: bool, out: &mut Vec<String>) {
    let Some(o) = v.as_object() else {
        if let Some(a) = v.as_array() {
            for (i, x) in a.iter().enumerate() {
                collect_write_args(&format!("{prefix}[{i}]"), x, read_only, out);
            }
        }
        return;
    };
    for (k, x) in o {
        let name = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
        let w = key_words(k);
        let writes = w.iter().any(|t| WRITE_WORDS.contains(&t.as_str()));
        let capture = w.iter().any(|t| CAPTURE_WORDS.contains(&t.as_str()));
        let present = !(x.is_null() || x == &Value::Bool(false) || x.as_str() == Some(""));
        let hit = if read_only {
            // a read takes no destination at all: any write-shaped argument, or any value that is a filesystem path
            // (top level only: a nested selector may legitimately look for a label that is a path)
            (writes && present) || (prefix.is_empty() && x.as_str().map(looks_like_fs_path).unwrap_or(false))
        } else {
            // an action's own targets may be paths (launch_app path, a file manager's folder): only a capture that is
            // written somewhere (screenshot_out_file ...) is a side effect the gate does not model
            capture && writes && present
        };
        if hit {
            out.push(name.clone());
        }
        if x.is_object() || x.is_array() {
            collect_write_args(&name, x, read_only, out);
        }
    }
}

/// Arguments that would make a driver call WRITE a file (save a screenshot to a path, dump a tree to a file ...).
/// A read-only tool may carry none of them (any write-shaped argument name, or a value that is a filesystem path);
/// any tool may not carry a capture-to-file argument (`screenshot_out_file` ...). Such a call is not a read: the gate
/// refuses it and sends nothing.
pub fn write_args(tool: &str, args: &Value) -> Vec<String> {
    let mut out = vec![];
    collect_write_args("", args, is_read_only(tool) && !is_gated(tool), &mut out);
    out
}

/// A driver tool that changes state: the gated GUI tools plus every driver tool not known to be read-only.
pub fn is_driver_action(t: &str) -> bool {
    is_gated(t) || !is_read_only(t)
}

/// A state-changing driver tool the gate has no GUI-target model for (launch_app, bring_to_front, a new driver tool).
pub fn is_passthrough_action(t: &str) -> bool {
    !is_gated(t) && !is_read_only(t)
}

/// launch_app: what it starts. Terminals that run a command, interpreters with code, file managers on a path and URLs
/// get the same risk classification as the equivalent action. Returns (level, reason, paths).
pub fn launch_risk(args: &Value) -> Option<(String, String, Vec<String>)> {
    let app = ["app_name", "name", "bundle_id", "app", "application", "path", "executable"].iter()
        .map(|k| pystr(args.get(*k))).find(|v| !v.is_empty()).unwrap_or_default();
    let mut list: Vec<String> = vec![];
    for k in ["args", "arguments", "argv", "urls", "url", "files", "file", "open", "paths", "documents", "command", "cmd"] {
        match args.get(k) {
            Some(Value::Array(a)) => list.extend(a.iter().map(|x| pystr(Some(x)))),
            Some(v) if !v.is_null() => list.push(pystr(Some(v))),
            _ => {}
        }
    }
    let base = app.rsplit(['/', '\\']).next().unwrap_or("").to_lowercase();
    let base = base.trim_end_matches(".exe").trim_end_matches(".app").to_string();
    let lapp = app.to_lowercase();
    let is_term = Regex::new(r"(?i)(terminal|konsole|xterm|alacritty|kitty|iterm|wezterm|tilix|terminator|foot|urxvt|xfce4-terminal|^cmd$|powershell|pwsh|^wt$|windowsterminal|^bash$|^sh$|^zsh$|^fish$|^dash$)").unwrap();
    let is_interp = Regex::new(r"^(python[0-9.]*|perl|ruby|node|osascript|wscript|cscript|php|lua)$").unwrap();
    let is_fm = Regex::new(r"(?i)(nautilus|^files$|dolphin|thunar|nemo|caja|pcmanfm|finder|explorer)").unwrap();
    let url = Regex::new(r"(?i)^[a-z][a-z0-9+.-]{1,20}:(//)?\S+").unwrap();
    // a URL handed to any app (a browser, xdg-open, mail client): the page or mailto/handler acts with the user's accounts
    if let Some(u) = list.iter().chain(std::iter::once(&app)).find(|x| url.is_match(x.trim()) && !x.trim().starts_with('/') && !x.contains(":\\")) {
        return Some(("medium".into(), format!("launch_app opens a URL ({})", trunc(u.trim(), 80)), vec![]));
    }
    if (is_term.is_match(&base) || is_interp.is_match(&base) || lapp.contains("terminal")) && !list.is_empty() {
        // the command part: after -e / -x / -- / -c / --command / /c, else everything
        let flags = ["-e", "-x", "--", "-c", "--command", "--command=", "/c", "/k", "-command", "--exec"];
        let mut cmd = list.clone();
        if let Some(i) = list.iter().position(|a| flags.contains(&a.as_str())) {
            cmd = list[i + 1..].to_vec();
        }
        let line = cmd.join(" ");
        if line.trim().is_empty() {
            return None;
        }
        if let Some((l, r, tg)) = crate::command::command_risk(&line, &crate::util::home()) {
            return Some((l, format!("launch_app {base}: {r}"), tg));
        }
        return Some(("medium".into(), format!("launch_app {base} would run: {}", trunc(&line, 80)), vec![]));
    }
    if is_fm.is_match(&base) {
        if let Some(pth) = list.iter().find(|x| !x.starts_with('-') && !x.trim().is_empty()) {
            let c = sidefx::canon(&sidefx::p(pth.trim(), &crate::util::home()));
            return Some(("medium".into(), format!("launch_app opens a file manager on {c}"), vec![c]));
        }
    }
    None
}

const RISK_HIGH: &[&str] = &["delete", "erase", "wipe", "pay", "pay now", "purchase", "buy now", "place order", "checkout",
    "confirm payment", "transfer", "uninstall", "format disk", "format drive", "format partition", "format volume",
    "factory reset", "empty trash", "permanently", "delete account", "close account", "remove account", "reset password",
    "删除", "清空", "支付", "付款", "购买", "立即购买", "下单", "转账", "卸载", "格式化", "永久", "注销账号", "恢复出厂"];
const RISK_MED: &[&str] = &["remove", "send", "submit", "publish", "post", "sign out", "log out", "logout", "overwrite",
    "replace all", "discard", "don't save", "revoke", "authorize", "allow", "grant", "accept", "shut down", "shutdown",
    "restart", "reboot", "install", "share", "invite", "move to trash", "order now",
    "移除", "发送", "提交", "发布", "退出登录", "覆盖", "不保存", "撤销", "授权", "允许", "关机", "重启", "安装", "分享", "邀请"];
const RISK_HOTKEYS_HIGH: &[&[&str]] = &[&["shift", "delete"], &["ctrl", "alt", "delete"]];
const RISK_HOTKEYS_MED: &[&[&str]] = &[&["ctrl", "enter"], &["ctrl", "return"], &["alt", "f4"], &["ctrl", "w"],
    &["ctrl", "q"], &["ctrl", "shift", "w"], &["super", "l"]];
const ENTER_KEYS: &[&str] = &["return", "enter", "kp_enter"];

macro_rules! re {
    ($name:ident, $pat:expr) => {
        fn $name() -> &'static Regex {
            static R: OnceLock<Regex> = OnceLock::new();
            R.get_or_init(|| Regex::new($pat).unwrap())
        }
    };
}
re!(shell_high, r"(?i)(\brm\s+-[a-z]*[rf]|\bmkfs\b|\bdd\s+if=|\bshutdown\b|\breboot\b|:\(\)\s*\{|\bchmod\s+-R\s+777|\bdrop\s+(table|database)\b|\btruncate\s+table\b|\bdelete\s+from\b|git\s+push\s+.*--force|\bcurl\b[^|]*\|\s*(ba)?sh|\bsudo\b|>\s*/dev/sd|\bformat\s+[a-z]:)");
re!(secret_field, r"(?i)(password|passcode|passwd|\bpin\b|密码|口令|cvv|cvc|security code|card number|卡号|验证码|one-time code|otp|2fa|verification code)");
re!(captcha, r"(?i)(captcha|recaptcha|hcaptcha|i'?m not a robot|verify you are human|人机验证|滑块验证|安全验证)");
re!(opener_role, r"^(menu|menu ?bar|menubar|menu bar item|tab|page tab|tab list|combo ?box|heading|label|static text|text)$");
re!(token_re, r"^(s[0-9a-f]{8}):(\d+)$");

struct WordRx(Vec<(&'static str, Option<Regex>)>);

fn word_rx(words: &'static [&'static str]) -> WordRx {
    WordRx(words.iter().map(|w| (*w, if w.is_ascii() { Regex::new(&format!(r"\b{}\b", regex::escape(w))).ok() } else { None })).collect())
}
fn high_res() -> &'static WordRx {
    static R: OnceLock<WordRx> = OnceLock::new();
    R.get_or_init(|| word_rx(RISK_HIGH))
}
fn med_res() -> &'static WordRx {
    static R: OnceLock<WordRx> = OnceLock::new();
    R.get_or_init(|| word_rx(RISK_MED))
}

fn match_words(s: &str, res: &WordRx) -> Option<&'static str> {
    res.0.iter().find(|(w, rx)| match rx { Some(r) => r.is_match(s), None => s.contains(w) }).map(|(w, _)| *w)
}

pub fn risky_text(s: &str) -> Option<&'static str> {
    let s = s.to_lowercase();
    if s.is_empty() {
        return None;
    }
    match_words(&s, high_res()).or_else(|| match_words(&s, med_res()))
}

pub fn risk_of_label(s: &str) -> (Option<&'static str>, Option<&'static str>) {
    let s = s.to_lowercase();
    let s = s.trim();
    if s.is_empty() {
        return (None, None);
    }
    if let Some(w) = match_words(s, high_res()) {
        return (Some("high"), Some(w));
    }
    if let Some(w) = match_words(s, med_res()) {
        return (Some("medium"), Some(w));
    }
    (None, None)
}

#[derive(Clone, Debug, Default)]
pub struct Target {
    pub kind: String, // token | index | pixel | name | keyboard | none
    pub pid: Option<i64>,
    pub window_id: Option<i64>,
    pub snapshot: Option<String>,
    pub index: Option<i64>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub role: String,
    pub label: String,
    pub element: Option<Element>,
    pub obs: Option<ObsId>,
}

impl Target {
    pub fn kind(k: &str) -> Self {
        Target { kind: k.into(), ..Default::default() }
    }
    pub fn the_label(&self) -> &str {
        self.element.as_ref().map(|e| e.label.as_str()).unwrap_or(&self.label)
    }
    pub fn the_role(&self) -> &str {
        self.element.as_ref().map(|e| e.role.as_str()).unwrap_or(&self.role)
    }
    pub fn is_elem(&self) -> bool {
        matches!(self.kind.as_str(), "token" | "index" | "name")
    }
}

#[derive(Clone, Debug, Default)]
pub struct ActionCtx {
    pub tool: String,
    pub args: Value,
    pub target: Target,
    pub signature: String,
    pub risk: Option<String>,
    pub steps: Vec<(String, Value, Target)>,
    pub risk_level: Option<String>,
    pub handoff: Option<String>,
}

fn keys_str(args: &Value) -> String {
    for k in ["keys", "key", "text", "value"] {
        if truthy(args.get(k)) {
            let v = args.get(k).unwrap();
            return match v {
                Value::Array(a) => format!("[{}]", a.iter().map(|x| format!("'{}'", pystr(Some(x)))).collect::<Vec<_>>().join(", ")),
                other => pystr(Some(other)),
            };
        }
    }
    String::new()
}

fn opt_i(v: Option<i64>) -> String {
    v.map(|x| x.to_string()).unwrap_or_else(|| "None".into())
}

fn signature(tool: &str, t: &Target, args: &Value) -> String {
    match t.kind.as_str() {
        "token" | "index" => {
            let (lab, role) = t.element.as_ref().map(|e| (e.label.clone(), e.role.clone())).unwrap_or_default();
            format!("{tool}|{}|{role}|{lab}", opt_i(t.pid))
        }
        "pixel" => format!("{tool}|{}|{}|{}", opt_i(t.window_id), round_tens(t.x.unwrap_or(0.0)), round_tens(t.y.unwrap_or(0.0))),
        "name" => format!("{tool}|{}|{}", t.role, t.label),
        "command" => format!("{tool}|{}|{}", pystr(args.get("cwd")), pystr(args.get("command"))),
        _ => format!("{tool}|{}|{}", opt_i(t.pid), keys_str(args)),
    }
}

pub fn resolve_target(tool: &str, args: &Value, st: &GateState) -> Target {
    if tool == "run_command" {
        return Target { kind: "command".into(), role: "command".into(), label: trunc(&pystr(args.get("command")), 80), ..Default::default() };
    }
    let tok = args.get("element_token").and_then(|v| v.as_str());
    let mut pid = as_i64(args.get("pid"));
    let mut wid = as_i64(args.get("window_id"));
    if let Some(tgt) = args.get("target").filter(|t| t.is_object()) {
        if s(tgt, "kind") == "window" {
            pid = pid.or(as_i64(tgt.get("pid")));
            wid = wid.or(as_i64(tgt.get("window_id")));
        }
    }
    if let Some(m) = tok.and_then(|t| token_re().captures(t)) {
        let snap = m[1].to_string();
        let idx: i64 = m[2].parse().unwrap_or(0);
        let obs = st.obs_by_snapshot.get(&snap).copied();
        let el = obs.and_then(|o| st.get(o).elements.get(&idx).cloned());
        let o = obs.map(|o| st.get(o));
        return Target {
            kind: "token".into(), pid: pid.or(o.and_then(|o| o.pid)), window_id: wid.or(o.and_then(|o| o.window_id)),
            snapshot: Some(snap), index: Some(idx), role: el.as_ref().map(|e| e.role.clone()).unwrap_or_default(),
            label: el.as_ref().map(|e| e.label.clone()).unwrap_or_default(), element: el, obs, ..Default::default()
        };
    }
    if present(args.get("element_index")) {
        let obs = st.latest_for(pid, wid);
        let idx = as_i64(args.get("element_index")).unwrap_or(0);
        let o = obs.map(|o| st.get(o));
        let el = o.and_then(|o| o.elements.get(&idx).cloned());
        return Target {
            kind: "index".into(), pid, window_id: wid.or(o.and_then(|o| o.window_id)), snapshot: o.map(|o| o.snapshot_id.clone()),
            index: Some(idx), role: el.as_ref().map(|e| e.role.clone()).unwrap_or_default(),
            label: el.as_ref().map(|e| e.label.clone()).unwrap_or_default(), element: el, obs, ..Default::default()
        };
    }
    if present(args.get("x")) || present(args.get("from_x")) {
        let obs = st.latest_for(pid, wid);
        let x = if present(args.get("x")) { as_f64(args.get("x")) } else { as_f64(args.get("from_x")) };
        let y = if present(args.get("x")) { as_f64(args.get("y")) } else { as_f64(args.get("from_y")) };
        return Target { kind: "pixel".into(), pid: pid.or(obs.and_then(|o| st.get(o).pid)), window_id: wid, x, y, obs, ..Default::default() };
    }
    if truthy(args.get("role")) || truthy(args.get("name")) {
        let obs = st.latest_for(pid, wid);
        return Target { kind: "name".into(), pid, window_id: wid, role: pystr(args.get("role")), label: pystr(args.get("name")), obs, ..Default::default() };
    }
    if KEY_TOOLS.contains(&tool) {
        return Target { kind: "keyboard".into(), pid, window_id: wid, obs: st.latest_for(pid, wid), ..Default::default() };
    }
    Target { kind: "none".into(), pid, window_id: wid, obs: st.latest_for(pid, wid), ..Default::default() }
}

pub fn build_ctx(tool: &str, args: &Value, st: &GateState) -> ActionCtx {
    let mut steps = vec![];
    let t = if tool == "run_steps" {
        for s0 in args.get("steps").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
            let Some(o) = s0.as_object() else { continue };
            let (stool, mut sargs) = if let Some(tl) = o.get("tool") {
                (pystr(Some(tl)), o.get("args").cloned().filter(|v| v.is_object()).unwrap_or(Value::Object(Obj::new())))
            } else {
                let Some(k) = o.keys().find(|k| !["wait_for", "expect", "timeout_ms"].contains(&k.as_str())) else { continue };
                (k.clone(), o.get(k).cloned().filter(|v| v.is_object()).unwrap_or(Value::Object(Obj::new())))
            };
            for k in ["pid", "window_id"] {
                if sargs.get(k).is_none() {
                    if let Some(v) = args.get(k) {
                        sargs[k] = v.clone();
                    }
                }
            }
            let tg = resolve_target(&stool, &sargs, st);
            steps.push((stool, sargs, tg));
        }
        steps.first().map(|s| s.2.clone()).unwrap_or_else(|| Target::kind("none"))
    } else {
        resolve_target(tool, args, st)
    };
    let (risk, level, handoff) = assess_risk(tool, args, &t, &steps, st);
    ActionCtx { tool: tool.into(), args: args.clone(), signature: signature(tool, &t, args), target: t, risk, steps, risk_level: level, handoff }
}

/// The canonical absolute paths a risky action would touch (the disk checks run again; only called for risky steps).
pub fn risk_targets(ctx: &ActionCtx, st: &GateState) -> Vec<String> {
    if ctx.risk.is_none() {
        return vec![];
    }
    assess_risk_t(&ctx.tool, &ctx.args, &ctx.target, &ctx.steps, st).3
}

fn obs_blob(o: &Observation) -> String {
    let labels: Vec<&str> = o.elements.values().map(|e| e.label.as_str()).collect();
    format!("{} {}", trunc(&o.text, 8000), trunc(&labels.join(" "), 4000))
}

pub fn newest_obs_blob(st: &GateState, pid: Option<i64>) -> String {
    let Some(pid) = pid else { return String::new() };
    st.newest_for_pid(pid, now() - 30.0).map(|i| obs_blob(st.get(i))).unwrap_or_default()
}

pub fn newest_obs_text(st: &GateState, pid: i64) -> String {
    st.newest_for_pid(pid, now() - 30.0).map(|i| st.get(i).text.clone()).unwrap_or_default()
}

fn key_set(a: &Value, split_key: bool) -> HashSet<String> {
    let mut ks: HashSet<String> = a.get("keys").and_then(|k| k.as_array()).map(|arr| arr.iter().map(|k| pystr(Some(k)).to_lowercase()).collect())
        .unwrap_or_default();
    if truthy(a.get("key")) {
        let k = pystr(a.get("key"));
        if split_key {
            ks.extend(k.split('+').map(|x| x.trim().to_lowercase()));
        } else {
            ks.insert(k.to_lowercase());
        }
    }
    ks
}

/// (reason, level, handoff) from what the action touches. Rules only, plus file-system checks.
pub fn assess_risk(tool: &str, args: &Value, t: &Target, steps: &[(String, Value, Target)], st: &GateState)
    -> (Option<String>, Option<String>, Option<String>) {
    let (r, l, h, _) = assess_risk_t(tool, args, t, steps, st);
    (r, l, h)
}

type Best = (Option<String>, Option<String>, Vec<String>);

/// assess_risk plus the canonical paths every risky part would touch (the union: a key built from them covers all).
pub fn assess_risk_t(tool: &str, args: &Value, t: &Target, steps: &[(String, Value, Target)], st: &GateState)
    -> (Option<String>, Option<String>, Option<String>, Vec<String>) {
    let mut best: Best = (None, None, vec![]);
    let bump_t = |best: &mut Best, level: &str, reason: String, tg: Vec<String>| {
        if level == "high" || best.1.is_none() {
            if best.1.as_deref() != Some("high") {
                best.0 = Some(reason);
                best.1 = Some(level.to_string());
            }
        }
        best.2.extend(tg);
        best.2.sort();
        best.2.dedup();
    };
    let bump = |best: &mut Best, level: &str, reason: String| bump_t(best, level, reason, vec![]);
    let mut items: Vec<(String, Value, Target)> = vec![(tool.to_string(), args.clone(), t.clone())];
    items.extend(steps.iter().cloned());
    let mut handoff: Option<String> = None;
    for (tl, a, tg) in &items {
        let tl = tl.as_str();
        let label = tg.the_label().to_string();
        let role = tg.the_role().to_string();
        if ["click", "double_click", "browser_click", "invoke_menu", "set_value"].contains(&tl) || tg.is_elem() {
            let (mut lv, w) = risk_of_label(&label);
            let rl = role.to_lowercase();
            if lv.is_some() && (opener_role().is_match(&rl) || tg.element.as_ref().map(|e| e.opens_submenu()).unwrap_or(false)) {
                lv = None; // opening a menu, tab, list or submenu shows choices; it changes nothing
            } else if lv == Some("high") && rl.contains("menu") {
                lv = Some("medium"); // an app command from a menu is usually undoable
            }
            if let (Some(lv), Some(w)) = (lv, w) {
                if tl != "type_text" {
                    bump(&mut best, lv, format!("\"{}\" ({w})", trunc(&label, 40)));
                }
            }
        }
        let text = if truthy(a.get("text")) { pystr(a.get("text")) } else { pystr(a.get("value")) };
        if ["type_text", "set_value", "browser_type"].contains(&tl) && !text.is_empty() {
            if secret_field().is_match(&format!("{role} {label}")) || role.to_lowercase().contains("password") {
                if handoff.is_none() {
                    handoff = Some(format!("typing into a secret field \"{}\"", if label.is_empty() { role.clone() } else { trunc(&label, 40) }));
                }
            }
            // in a terminal, typing does not run anything (the Enter is checked against the disk); elsewhere
            // (an editor, a SQL console, a chat box) dangerous text is still flagged
            let term_line = tg.pid.map(sidefx::is_terminal).unwrap_or(false) && !text.contains('\n');
            if let (Some(m), false) = (shell_high().find(&text), term_line) {
                bump(&mut best, "medium", format!("typed a dangerous command ({})", trunc(m.as_str(), 30)));
            }
        }
        if tl == "press_key" || tl == "hotkey" {
            let ks = key_set(a, true);
            let mut sorted: Vec<String> = ks.iter().cloned().collect();
            sorted.sort();
            if RISK_HOTKEYS_HIGH.iter().any(|h| h.iter().all(|k| ks.contains(*k))) {
                bump(&mut best, "high", sorted.join("+"));
            } else if RISK_HOTKEYS_MED.iter().any(|h| h.iter().all(|k| ks.contains(*k))) {
                bump(&mut best, "medium", sorted.join("+"));
            }
            if ENTER_KEYS.iter().any(|k| ks.contains(*k)) {
                if let Some(pid) = tg.pid {
                    let typed = st.last_typed.get(&pid).cloned().unwrap_or_default();
                    // `rm -rf` in a terminal is judged by what it would delete (shell_check below: existing paths
                    // that predate the session); other dangerous patterns (sudo, mkfs, curl|sh, ...) stay high
                    let term = sidefx::is_terminal(pid);
                    let hit = shell_high().find_iter(&typed).any(|m| !(term && m.as_str().trim_start().starts_with("rm")));
                    if !typed.is_empty() && hit {
                        bump(&mut best, "high", format!("Enter would run: {}", trunc(&typed, 60)));
                    }
                    shell_check(&typed, pid, &mut |l, r, tg| bump_t(&mut best, l, r, tg));
                }
            }
            if tg.pid.is_some() && sidefx::overwrite_dialog(&newest_obs_blob(st, tg.pid), "", &ks) {
                bump(&mut best, "high", "confirms replacing an existing file".into());
            }
        }
        if (tl == "type_text" || tl == "set_value") && text.contains('\n') {
            if let Some(pid) = tg.pid {
                let prev = st.last_typed.get(&pid).cloned().unwrap_or_default();
                let line = format!("{prev}{}", text.split('\n').next().unwrap_or(""));
                shell_check(&line, pid, &mut |l, r, tg| bump_t(&mut best, l, r, tg));
            }
        }
        if (tl == "click" || tl == "double_click") && !label.is_empty() && tg.pid.is_some() {
            let blob = match tg.obs { Some(o) => obs_blob(st.get(o)), None => newest_obs_blob(st, tg.pid) };
            if sidefx::overwrite_dialog(&blob, &label, &HashSet::new()) {
                bump(&mut best, "high", format!("\"{}\" confirms replacing an existing file", trunc(&label, 30)));
            }
        }
        if ["click", "double_click", "press_key"].contains(&tl) {
            if let Some(pid) = tg.pid {
                let lab = label.trim().to_lowercase().replace(['_', '&'], "");
                let lab = lab.trim_end_matches('.');
                let enter = tl == "press_key" && ENTER_KEYS.iter().any(|k| key_set(a, false).contains(*k));
                if sidefx::SAVE_LABELS.contains(&lab) || enter {
                    let own = if tl != "press_key" { tg.obs.map(|o| st.get(o).text.clone()).unwrap_or_default() } else { String::new() };
                    let src = if own.is_empty() { newest_obs_text(st, pid) } else { own };
                    let head = trunc(&src, 400);
                    if head.contains("dialog") || src.contains("Save") || src.contains("Export") {
                        if let Some(hit) = sidefx::save_dialog_target(&src) {
                            let c = sidefx::canon(&hit);
                            bump_t(&mut best, "high", format!("saving here would replace the existing file {hit}"), vec![c]);
                        }
                    }
                }
            }
        }
        if (tl == "click" || tl == "double_click") && label.is_empty() && tg.pid.is_some() && tg.kind == "pixel"
            && sidefx::overwrite_ctx().is_match(&newest_obs_blob(st, tg.pid)) {
            bump(&mut best, "medium", "pixel click inside a dialog that asks to replace an existing file".into());
        }
    }
    if tool == "launch_app" {
        if let Some((l, r, tg)) = launch_risk(args) {
            bump_t(&mut best, &l, r, tg);
        }
    }
    if tool == "kill_app" {
        bump(&mut best, "medium", "kill_app".into());
    }
    if tool == "browser_download" || tool == "browser_set_input_files" {
        bump(&mut best, "medium", tool.to_string());
    }
    if tool == "run_command" {
        if let Some((l, r, tg)) = crate::command::command_risk(&pystr(args.get("command")), &command_cwd(args)) {
            bump_t(&mut best, &l, r, tg);
        }
    }
    if tool == "run_script" {
        let code = if truthy(args.get("script")) { pystr(args.get("script")) } else { pystr(args.get("code")) };
        if shell_high().is_match(&code) || risky_text(&code).is_some() {
            bump(&mut best, "high", "run_script with destructive content".into());
        }
    }
    if let Some(o) = t.obs {
        if handoff.is_none() && tool != "press_key" {
            let labels: Vec<&str> = st.get(o).elements.values().map(|e| e.label.as_str()).collect();
            if captcha().is_match(&trunc(&labels.join(" "), 4000)) {
                handoff = Some("the window shows a human-verification (captcha) step".into());
            }
        }
    }
    let tg = if best.1.is_some() { best.2 } else { vec![] };
    (best.0, best.1, handoff, tg)
}

/// The directory a run_command runs in: its cwd argument (~ and $VARS expanded), else the user's home.
pub fn command_cwd(args: &Value) -> String {
    let c = pystr(args.get("cwd"));
    if c.trim().is_empty() { crate::util::home() } else { sidefx::p(c.trim(), &crate::util::home()) }
}

fn shell_check(cmd: &str, pid: i64, bump: &mut dyn FnMut(&str, String, Vec<String>)) {
    if cmd.is_empty() || !sidefx::is_terminal(pid) {
        return;
    }
    if let Some((l, r, tg)) = sidefx::shell_effects(cmd, &sidefx::shell_cwd(pid)) {
        bump(&l, r, tg);
    }
}

pub const FEATURE_NAMES: &[&str] = &["bias", "is_pixel", "is_token", "is_name", "is_keyboard", "no_obs", "token_stale",
    "log_obs_age", "acts_since_obs", "obs_had_shot", "last_err", "last_unverif", "last_popup", "repeat", "edge_close",
    "small_target", "label_empty", "label_dup", "loading_cue", "window_changed", "window_added", "elem_unknown",
    "check_pass", "check_fail",
    // v2: what this session has already shown (known failures so far) and the kind of element
    "prev_none", "sess_fail", "consec_fail", "sig_failed", "role_menu", "role_cell", "role_toggle"];

/// Session-history features from the actions before this one: (sig, known failure, no visible effect).
/// Shared by the live gate and the offline trainer so both compute exactly the same numbers.
pub fn history_feats(prev: &[(String, bool, bool)], sig: &str) -> [(&'static str, f64); 4] {
    let n = prev.len() as f64;
    let fails = prev.iter().filter(|p| p.1).count() as f64;
    let consec = prev.iter().rev().take_while(|p| p.1).count().min(3) as f64;
    let sig_failed = prev.iter().any(|p| p.1 && p.0 == sig);
    let prev_none = prev.last().map(|p| p.2).unwrap_or(false);
    [("prev_none", if prev_none { 1.0 } else { 0.0 }),
     ("sess_fail", ((fails + 1.0) / (n - fails + 4.0)).ln()),   // shrunk log-odds of failing in this session
     ("consec_fail", consec),
     ("sig_failed", if sig_failed { 1.0 } else { 0.0 })]
}

pub fn role_feats(role: &str) -> [(&'static str, f64); 3] {
    let r = role.to_lowercase();
    let b = |x: bool| if x { 1.0 } else { 0.0 };
    [("role_menu", b(r.contains("menu"))), ("role_cell", b(r.contains("cell"))),
     ("role_toggle", b(r.contains("toggle") || r.contains("check") || r.contains("radio")))]
}

/// v2 features (GUI_GATE_VALUE_MODEL=v2 only; never computed or logged on the default path): the kind of GUI
/// action and of its target, both known at decision time and present in every gate log (tool, target.role).
pub const V2_FEATURES: &[&str] = &["tool_click", "tool_scroll", "tool_type", "tool_key", "tool_set_value", "scroll_pixel",
    "role_button", "role_text", "role_link_tab", "role_list_item", "role_combo"];

/// FEATURE_NAMES followed by V2_FEATURES (the offline trainer's row layout; v1 rows keep the v2 slots at 0).
pub const ALL_FEATURES: &[&str] = &["bias", "is_pixel", "is_token", "is_name", "is_keyboard", "no_obs", "token_stale",
    "log_obs_age", "acts_since_obs", "obs_had_shot", "last_err", "last_unverif", "last_popup", "repeat", "edge_close",
    "small_target", "label_empty", "label_dup", "loading_cue", "window_changed", "window_added", "elem_unknown",
    "check_pass", "check_fail", "prev_none", "sess_fail", "consec_fail", "sig_failed", "role_menu", "role_cell", "role_toggle",
    "tool_click", "tool_scroll", "tool_type", "tool_key", "tool_set_value", "scroll_pixel",
    "role_button", "role_text", "role_link_tab", "role_list_item", "role_combo"];

/// A command (run_command) is not a GUI step: v2 keeps it out of the GUI predictor and out of the GUI history.
pub fn is_command_tool(tool: &str) -> bool {
    tool == "run_command"
}

/// The v2 features of one step from its tool, target kind and target role (shared by the gate and the trainer).
pub fn v2_feats(tool: &str, kind: &str, role: &str) -> [(&'static str, f64); 11] {
    let r = role.to_lowercase();
    let b = |x: bool| if x { 1.0 } else { 0.0 };
    let scroll = tool == "scroll";
    [("tool_click", b(tool.contains("click"))), ("tool_scroll", b(scroll)), ("tool_type", b(tool == "type_text")),
     ("tool_key", b(tool == "press_key" || tool == "hotkey")), ("tool_set_value", b(tool == "set_value")),
     ("scroll_pixel", b(scroll && kind == "pixel")),
     ("role_button", b(r.contains("button") && !r.contains("radio") && !r.contains("toggle") && !r.contains("menu"))),
     ("role_text", b(r.contains("text") || r.contains("entry") || r.contains("edit") || r.contains("paragraph") || r.contains("document"))),
     ("role_link_tab", b(r.contains("link") || r.contains("tab"))),
     ("role_list_item", b(r.contains("list item") || r.contains("tree item") || r.contains("row") || r == "item")),
     ("role_combo", b(r.contains("combo")))]
}

pub type Feats = IndexMap<&'static str, f64>;

pub fn fget(f: &Feats, k: &str) -> f64 {
    f.get(k).copied().unwrap_or(0.0)
}

fn b(x: bool) -> f64 {
    if x { 1.0 } else { 0.0 }
}

pub fn features(ctx: &ActionCtx, st: &GateState, window_changed: Option<bool>, window_added: Option<bool>,
                check: Option<bool>, now_t: Option<f64>) -> Feats {
    let nowv = now_t.unwrap_or_else(now);
    let t = &ctx.target;
    let wk = if t.pid.is_some() { wkey(t.pid, t.window_id) } else { None };
    let latest = if t.pid.is_some() { st.latest_for(t.pid, t.window_id) } else { None };
    let mut f: Feats = FEATURE_NAMES.iter().map(|k| (*k, 0.0)).collect();
    f["bias"] = 1.0;
    f["is_pixel"] = b(t.kind == "pixel");
    f["is_token"] = b(t.kind == "token" || t.kind == "index");
    f["is_name"] = b(t.kind == "name");
    f["is_keyboard"] = b(t.kind == "keyboard");
    let refo = latest.or(t.obs);
    f["no_obs"] = b(refo.is_none());
    if (t.kind == "token" || t.kind == "index") && t.snapshot.is_some() {
        if let Some(l) = latest {
            f["token_stale"] = b(Some(&st.get(l).snapshot_id) != t.snapshot.as_ref());
        }
    }
    if let Some(r) = refo {
        let o = st.get(r);
        let age = (nowv - o.t).max(0.0);
        f["log_obs_age"] = age.min(300.0).ln_1p();
        f["acts_since_obs"] = st.actions_since(o.t, wk.as_deref()).min(5) as f64;
        // text-only client: the tree is the fullest observation it can use (keeps p_direct comparable to runs with shots)
        f["obs_had_shot"] = b(o.with_screenshot || st.text_only);
        f["loading_cue"] = b(o.loading_cue);
    }
    // v2 (st.gui_history): command exit codes are not GUI outcomes; the GUI history skips commands
    let gui = |a: &&crate::state::ActionRecord| !(st.gui_history && is_command_tool(&a.tool));
    if let Some(la) = st.actions.iter().rev().find(gui) {
        f["last_err"] = b(la.is_error);
        f["last_unverif"] = b(la.unverifiable);
        f["last_popup"] = b(la.popup);
    }
    f["repeat"] = st.repeat_count(&ctx.signature, refo.map(|r| st.get(r).t).unwrap_or(0.0)).min(3) as f64;
    if let (true, Some(r), Some(x)) = (t.kind == "pixel", refo, t.x) {
        let o = st.get(r);
        if let (Some(w), Some(h)) = (o.shot_w, o.shot_h) {
            if w > 0.0 && h > 0.0 {
                let y = t.y.unwrap_or(0.0);
                let m = x.min(y).min(w - x).min(h - y);
                f["edge_close"] = b(m < 0.03 * w.min(h));
            }
        }
    }
    if let Some(fr) = t.element.as_ref().and_then(|e| e.frame.as_ref()) {
        f["small_target"] = b(fr.w * fr.h < 18.0 * 18.0);
    }
    if t.is_elem() {
        let el = t.element.as_ref();
        let lab = el.map(|e| e.label.clone()).unwrap_or_else(|| t.label.clone());
        f["label_empty"] = b(lab.is_empty());
        f["elem_unknown"] = b(el.is_none() && t.kind != "name");
        if let (Some(r), false) = (refo, lab.is_empty()) {
            let dup = st.get(r).elements.values().filter(|e| e.label == lab && el.map(|x| x.role == e.role).unwrap_or(true)).count();
            f["label_dup"] = b(dup > 1);
        }
    }
    if let Some(w) = window_changed {
        f["window_changed"] = b(w);
    }
    if let Some(w) = window_added {
        f["window_added"] = b(w);
    }
    match check {
        Some(true) => f["check_pass"] = 1.0,
        Some(false) => f["check_fail"] = 1.0,
        None => {}
    }
    let prev: Vec<(String, bool, bool)> = st.actions.iter().filter(gui).map(|a| (a.key.clone(), a.known_fail(), a.no_effect)).collect();
    for (k, v) in history_feats(&prev, &ctx.signature) {
        f[k] = v;
    }
    let role = t.element.as_ref().map(|e| e.role.clone()).unwrap_or_else(|| t.role.clone());
    for (k, v) in role_feats(&role) {
        f[k] = v;
    }
    if st.gui_history {
        for (k, v) in v2_feats(&ctx.tool, &t.kind, &role) {
            f.insert(k, v);
        }
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_read_takes_no_destination() {
        use serde_json::json;
        let w = |t: &str, a: Value| write_args(t, &a);
        assert_eq!(w("get_window_state", json!({"pid": 1, "screenshot_out_file": "/tmp/x.png"})), vec!["screenshot_out_file"]);
        assert_eq!(w("screenshot", json!({"outputPath": "x.png"})), vec!["outputPath"]);
        assert_eq!(w("zoom", json!({"save_to": "x.png"})), vec!["save_to"]);
        assert_eq!(w("list_windows", json!({"to": "/home/u/.bashrc"})), vec!["to"]);
        assert_eq!(w("screenshot", json!({"to": "C:\\Users\\u\\a.png"})), vec!["to"]);
        assert_eq!(w("get_window_state", json!({"opts": {"dump_dir": "d"}})), vec!["opts.dump_dir"]);
        // ordinary read arguments stay reads
        assert!(w("get_window_state", json!({"pid": 1, "window_id": 2, "include_screenshot": true, "tree_format": "elements",
            "full_output": true, "capture_mode": "window", "max_image_dimension": 800, "timeout_ms": 500, "screenshot_out_file": null})).is_empty());
        assert!(w("verify_state", json!({"expect": [{"element": {"selector": {"label_contains": "/tmp/report.txt"}}}]})).is_empty());
        // an action's own path targets are its business (risk rules); a capture written to a file is not
        assert!(w("launch_app", json!({"app_name": "nautilus", "path": "/home/u"})).is_empty());
        assert!(w("click", json!({"element_token": "s1:1"})).is_empty());
        assert_eq!(w("click", json!({"element_token": "s1:1", "screenshot_out_file": "/tmp/a.png"})), vec!["screenshot_out_file"]);
    }
    use serde_json::json;

    fn risk(role: &str, label: &str) -> Option<String> {
        let t = Target { kind: "token".into(), pid: Some(999999), element: Some(Element::new(1, role, label)), ..Default::default() };
        assess_risk("click", &json!({}), &t, &[], &GateState::default()).1
    }

    #[test]
    fn v2_gui_history_skips_commands_and_the_default_keeps_its_features() {
        let rec = |tool: &str, key: &str, err: bool| crate::state::ActionRecord { t: 1.0, tool: tool.into(), key: key.into(), window_key: None,
            is_error: err, unverifiable: false, popup: false, stale_error: false, aid: 1, no_effect: false, retro: false };
        let ctx = ActionCtx { tool: "click".into(), args: json!({}), target: Target { kind: "token".into(), role: "push button".into(), label: "OK".into(), ..Default::default() },
                              signature: "click|1|push button|OK".into(), ..Default::default() };
        for v2 in [false, true] {
            let mut st = GateState::for_config(false, v2);
            st.record_action(rec("click", "click|1|push button|OK", false));
            for i in 0..3 {
                st.record_action(rec("run_command", &format!("run_command||grep {i}"), true));
            }
            let f = features(&ctx, &st, None, None, None, Some(2.0));
            if v2 {
                assert_eq!(f["consec_fail"], 0.0, "a grep exit code is not a GUI failure");
                assert_eq!(f["last_err"], 0.0);
                assert_eq!(f["tool_click"], 1.0);
                assert_eq!(f["role_button"], 1.0);
            } else {
                assert_eq!(f["consec_fail"], 3.0, "default path: unchanged (commands count)");
                assert_eq!(f["last_err"], 1.0);
                assert_eq!(f.keys().cloned().collect::<Vec<_>>(), FEATURE_NAMES.to_vec(), "no v2 keys on the default path");
            }
        }
        assert_eq!(&ALL_FEATURES[..FEATURE_NAMES.len()], FEATURE_NAMES);
        assert_eq!(ALL_FEATURES.len(), FEATURE_NAMES.len() + V2_FEATURES.len());
        assert_eq!(&ALL_FEATURES[FEATURE_NAMES.len()..], V2_FEATURES);
    }

    #[test]
    fn menu_openers_are_not_risky() {
        assert_eq!(risk("menu", "Format"), None);
        assert_eq!(risk("menu", "Edit"), None);
        assert_eq!(risk("button", "Format"), None);
        assert_eq!(risk("menu item", "Delete Rows").as_deref(), Some("medium"));
        assert_eq!(risk("button", "Delete").as_deref(), Some("high"));
        assert_eq!(risk("button", "Format Disk").as_deref(), Some("high"));
    }

    /// v0.10.2 regression (docs/failure-analysis-v099.md, chrome-35253b65): Chromium's "Cast, save, and share" row
    /// offers only `open` (a submenu parent), so clicking it shares nothing; leaf items (select / click) keep their risk.
    #[test]
    fn submenu_parent_is_not_risky_but_leaf_items_are() {
        let el = |role: &str, label: &str, acts: &[&str]| {
            let mut e = Element::new(1, role, label);
            e.actions = acts.iter().map(|x| x.to_string()).collect();
            let t = Target { kind: "token".into(), pid: Some(999999), element: Some(e), ..Default::default() };
            assess_risk("click", &json!({}), &t, &[], &GateState::default()).1
        };
        assert_eq!(el("menu item", "Cast, save, and share", &["open", "showcontextmenu"]), None);
        assert_eq!(el("menu item", "Share", &["expand"]), None);
        assert_eq!(el("menu item", "Share", &["select", "showcontextmenu"]).as_deref(), Some("medium"));
        assert_eq!(el("menu item", "Delete", &["open", "click"]).as_deref(), Some("medium"));
        assert_eq!(el("menu item", "Share", &[]).as_deref(), Some("medium"), "no action list: unchanged");
        assert_eq!(el("push button", "Share", &["open"]).as_deref(), Some("medium"), "only menu items");
        // the row's action list is read from the tree markdown
        let md = "- [183] menu item \"More tools\" [actions=[open,showContextMenu]]\n- [184] menu item \"Share page\" [actions=[select,showContextMenu]]";
        let els = crate::state::parse_elements(&json!({"tree_markdown": md}));
        assert!(els[&183].opens_submenu());
        assert!(!els[&184].opens_submenu());
        assert_eq!(els[&183].actions, vec!["open", "showcontextmenu"]);
    }

    #[test]
    fn yes_in_replace_dialog_is_high() {
        let mut st = GateState::default();
        let md = "- alert = \"Question\"\n  - label = \"A file named \"pre.pptx\" already exists.\n\nDo you want to replace it?\"\n  - [0] button \"No\" [actions=[click]]\n  - [1] button \"Yes\" [actions=[click]]";
        let res = json!({"structuredContent": {"snapshot_id": "s00000009", "pid": 4242, "window_id": 7, "tree_markdown": md},
                         "content": [{"type": "text", "text": md}]});
        let o = st.record_observation(&json!({"pid": 4242, "window_id": 7}), &res, None).unwrap();
        let yes = st.get(o).elements.get(&1).cloned();
        assert!(yes.is_some());
        let t = Target { kind: "token".into(), pid: Some(4242), window_id: Some(7), snapshot: Some("s00000009".into()), index: Some(1), element: yes, obs: Some(o), ..Default::default() };
        assert_eq!(assess_risk("click", &json!({}), &t, &[], &st).1.as_deref(), Some("high"));
        let no = Target { element: st.get(o).elements.get(&0).cloned(), index: Some(0), ..t.clone() };
        assert_eq!(assess_risk("click", &json!({}), &no, &[], &st).1, None);
        let key = Target { kind: "keyboard".into(), pid: Some(4242), ..Default::default() };
        assert_eq!(assess_risk("press_key", &json!({"key": "Return"}), &key, &[], &st).1.as_deref(), Some("high"));
    }

    #[test]
    fn save_button_over_existing_export() {
        let d = std::env::temp_dir().join(format!("gg-ft-{}", rand_hex(5)));
        std::fs::create_dir_all(&d).unwrap();
        let d = d.to_string_lossy().to_string();
        std::fs::write(format!("{d}/Report.pdf"), "signed").unwrap();
        let txt = format!("- dialog = \"Export\"\n  - [0] text \"{d}\" value=\"{d}\" [actions=[activate]]\n  - [2] combo box \"PDF - Portable Document Format (.pdf)\"\n  - [44] text \"Report\" value=\"Report\" [actions=[activate]]\n  - [47] button \"Export\"");
        let mut st = GateState::default();
        let res = json!({"structuredContent": {"snapshot_id": "s00000005", "pid": 4243, "window_id": 9, "tree_markdown": txt},
                         "content": [{"type": "text", "text": txt}]});
        let o = st.record_observation(&json!({"pid": 4243, "window_id": 9}), &res, None).unwrap();
        let btn = st.get(o).elements.get(&47).cloned();
        let t = Target { kind: "token".into(), pid: Some(4243), window_id: Some(9), snapshot: Some("s00000005".into()), index: Some(47), element: btn, obs: Some(o), ..Default::default() };
        assert_eq!(assess_risk("click", &json!({}), &t, &[], &st).1.as_deref(), Some("high"));
    }
}
