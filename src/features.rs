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
    let mut best: (Option<String>, Option<String>) = (None, None);
    let bump = |best: &mut (Option<String>, Option<String>), level: &str, reason: String| {
        if level == "high" || best.1.is_none() {
            if best.1.as_deref() != Some("high") {
                *best = (Some(reason), Some(level.to_string()));
            }
        }
    };
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
            if lv.is_some() && opener_role().is_match(&rl) {
                lv = None; // opening a menu, tab or list shows choices; it changes nothing
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
                    shell_check(&typed, pid, &mut |l, r| bump(&mut best, l, r));
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
                shell_check(&line, pid, &mut |l, r| bump(&mut best, l, r));
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
                            bump(&mut best, "high", format!("saving here would replace the existing file {hit}"));
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
    if tool == "kill_app" {
        bump(&mut best, "medium", "kill_app".into());
    }
    if tool == "browser_download" || tool == "browser_set_input_files" {
        bump(&mut best, "medium", tool.to_string());
    }
    if tool == "run_command" {
        if let Some((l, r)) = crate::command::command_risk(&pystr(args.get("command")), &command_cwd(args)) {
            bump(&mut best, &l, r);
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
    (best.0, best.1, handoff)
}

/// The directory a run_command runs in: its cwd argument (~ and $VARS expanded), else the user's home.
pub fn command_cwd(args: &Value) -> String {
    let c = pystr(args.get("cwd"));
    if c.trim().is_empty() { crate::util::home() } else { sidefx::p(c.trim(), &crate::util::home()) }
}

fn shell_check(cmd: &str, pid: i64, bump: &mut dyn FnMut(&str, String)) {
    if cmd.is_empty() || !sidefx::is_terminal(pid) {
        return;
    }
    if let Some((l, r)) = sidefx::shell_effects(cmd, &sidefx::shell_cwd(pid)) {
        bump(&l, r);
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
    if let Some(la) = st.last_action() {
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
    let prev: Vec<(String, bool, bool)> = st.actions.iter().map(|a| (a.key.clone(), a.known_fail(), a.no_effect)).collect();
    for (k, v) in history_feats(&prev, &ctx.signature) {
        f[k] = v;
    }
    let role = t.element.as_ref().map(|e| e.role.clone()).unwrap_or_else(|| t.role.clone());
    for (k, v) in role_feats(&role) {
        f[k] = v;
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn risk(role: &str, label: &str) -> Option<String> {
        let t = Target { kind: "token".into(), pid: Some(999999), element: Some(Element::new(1, role, label)), ..Default::default() };
        assess_risk("click", &json!({}), &t, &[], &GateState::default()).1
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
