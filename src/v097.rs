//! 0.9.7: what the top OSWorld-Verified frameworks do, absorbed into the one MCP server (docs/sota-absorb-2026-10-22.md).
//! Every feature here is OFF unless switched on: `GUI_GATE_097=1` turns on all model-free ones, each can also be set
//! alone (`GUI_GATE_<NAME>=1`) or kept off inside the combined flag (`GUI_GATE_<NAME>=0`). Model roles additionally
//! need their own endpoint (`GUI_GATE_ROLE_<ROLE>_URL ...`, see roles.rs).
use regex::Regex;
use serde_json::{json, Value};
use std::sync::OnceLock;

/// Model-free features switched on together by GUI_GATE_097=1.
pub const FEATURES: &[&str] = &["INSPECT", "JOBS", "PHANTOM", "TWO_STRIKES", "FEASIBILITY_FIRST", "STATE_PLAN",
    "VERIFY_PLUS", "SKILLS", "SNAP", "PICK", "DIGEST", "MICRO_VERIFY"];

/// Is feature NAME on? An explicit GUI_GATE_<NAME> wins ("0"/"" = off, anything else = on); else GUI_GATE_097.
pub fn on(name: &str) -> bool {
    if let Ok(v) = std::env::var(format!("GUI_GATE_{name}")) {
        let v = v.trim().to_lowercase();
        return !(v.is_empty() || v == "0" || v == "false" || v == "off" || v == "no");
    }
    all_on()
}

pub fn all_on() -> bool {
    matches!(std::env::var("GUI_GATE_097").unwrap_or_default().trim().to_lowercase().as_str(), "1" | "true" | "on" | "yes")
}

/// Which 0.9.7 features are on (for gate_report and the log).
pub fn active() -> Vec<&'static str> {
    FEATURES.iter().copied().filter(|f| on(f)).collect()
}

// ------------------------------------------------------------------ phantom tools (Pointer: 57 / 11,603 calls)

fn suffix_rx() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)(?:_(?:safe|replacement|workaround|substitute|alt|alternative|fallback|new|v\d+|tool|action|does_not_exist|impl|fixed|retry|2))+$").unwrap())
}

/// Normalise a model-invented tool name: lowercase, camelCase -> snake, strip "computer_/gui_/mcp__x__" prefixes and
/// the "_safe / _replacement / _workaround ..." permutations Pointer saw.
pub fn normalize_name(name: &str) -> String {
    let mut s = String::new();
    for (i, c) in name.trim().chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            s.push('_');
        }
        s.extend(c.to_lowercase());
    }
    let mut s = s.replace(['-', ' ', '.'], "_");
    if let Some(i) = s.rfind("__") {
        s = s[i + 2..].to_string(); // mcp__server__tool
    }
    for p in ["computer_", "gui_", "desktop_", "mcp_", "os_", "ui_"] {
        if let Some(r) = s.strip_prefix(p) {
            s = r.to_string();
        }
    }
    let s = suffix_rx().replace(&s, "").to_string();
    s.trim_matches('_').to_string()
}

/// What an unknown tool name means: (real tool, adapted args, note). `real` is one of the advertised tool names or
/// one of the gate's pseudo-targets: "__wait", "__final", "__triple_click".
pub fn phantom_alias(name: &str, args: &Value, known: &[String]) -> Option<(String, Value, String)> {
    let n = normalize_name(name);
    let has = |t: &str| known.iter().any(|k| k == t);
    if has(&n) && n != name {
        return Some((n.clone(), args.clone(), format!("there is no tool \"{name}\"; ran {n}")));
    }
    let mut a = args.clone();
    if !a.is_object() {
        a = json!({});
    }
    // Anthropic computer-use style coordinate: [x, y]
    if let Some(c) = a.get("coordinate").and_then(|c| c.as_array()).cloned() {
        if c.len() == 2 {
            a["x"] = c[0].clone();
            a["y"] = c[1].clone();
            a.as_object_mut().unwrap().remove("coordinate");
        }
    }
    let rename = |a: &mut Value, from: &[&str], to: &str| {
        if a.get(to).is_none() {
            for f in from {
                if let Some(v) = a.as_object_mut().and_then(|o| o.remove(*f)) {
                    a[to] = v;
                    break;
                }
            }
        }
    };
    let target = match n.as_str() {
        "triple_click" | "tripleclick" | "click3" | "select_line" | "select_paragraph" => "__triple_click",
        "left_click" | "single_click" | "mouse_click" | "tap" | "click_element" | "click_on" | "press_button" | "click_button" => "click",
        "double" | "dbl_click" | "doubleclick" | "left_double_click" => "double_click",
        "context_click" | "rightclick" | "secondary_click" | "context_menu" => "right_click",
        "type" | "type_keys" | "typewrite" | "write" | "input_text" | "enter_text" | "send_text" | "fill" | "type_string" | "insert_text" => {
            rename(&mut a, &["content", "value", "input", "string", "chars"], "text");
            "type_text"
        }
        "key" | "keypress" | "press" | "send_keys" | "keys" | "key_press" | "keyboard" | "press_keys" => {
            let k = a.get("keys").or(a.get("key")).or(a.get("text")).cloned().unwrap_or(Value::Null);
            let combo = match &k {
                Value::Array(v) if v.len() > 1 => Some(v.iter().map(|x| x.as_str().unwrap_or("").to_lowercase()).collect::<Vec<_>>()),
                Value::String(s) if s.contains('+') && s.len() > 1 => Some(s.split('+').map(|x| x.trim().to_lowercase()).collect()),
                _ => None,
            };
            if let Some(parts) = combo.filter(|_| has("hotkey")) {
                return Some(("hotkey".into(), json!({"pid": a.get("pid"), "window_id": a.get("window_id"), "keys": parts}),
                             format!("there is no tool \"{name}\"; sent the combination with hotkey")));
            }
            let single = match &k { Value::Array(v) => v.first().cloned().unwrap_or(Value::Null), _ => k.clone() };
            if let Some(o) = a.as_object_mut() {
                o.remove("keys");
                o.remove("text");
            }
            a["key"] = single;
            "press_key"
        }
        "hotkeys" | "shortcut" | "key_combo" | "keyboard_shortcut" | "hold_and_press" | "key_combination" | "chord" => {
            rename(&mut a, &["combo", "shortcut", "key"], "keys");
            "hotkey"
        }
        "scroll_down" | "scroll_up" | "scroll_left" | "scroll_right" | "wheel" | "mouse_scroll" => {
            if a.get("direction").is_none() {
                if let Some(d) = n.strip_prefix("scroll_") {
                    a["direction"] = json!(d);
                }
            }
            "scroll"
        }
        "drag_and_drop" | "drag_drop" | "left_click_drag" | "mouse_drag" => "drag",
        "screenshot" | "take_screenshot" | "capture_screen" | "get_screenshot" | "observe" | "get_screen" | "screen" | "look"
        | "read_screen" | "get_state" | "window_state" | "get_window" | "snapshot" => {
            if a.get("pid").is_some() { "get_window_state" } else { "list_windows" }
        }
        "get_windows" | "windows" | "wmctrl" | "list_window" => "list_windows",
        "get_apps" | "list_applications" | "apps" | "running_apps" => "list_apps",
        "open" | "open_app" | "launch" | "start_app" | "open_application" | "run_app" | "start_application" => {
            rename(&mut a, &["name", "app", "application"], "app_name");
            "launch_app"
        }
        "switch_applications" | "switch_app" | "focus" | "focus_window" | "activate" | "activate_window" | "switch_window" => "bring_to_front",
        "select_all" => {
            return has("hotkey").then(|| ("hotkey".into(), json!({"pid": a.get("pid"), "window_id": a.get("window_id"), "keys": ["ctrl", "a"]}),
                                          format!("there is no tool \"{name}\"; sent ctrl+a with hotkey")));
        }
        "bash" | "shell" | "run_bash" | "run_shell" | "execute" | "exec" | "terminal" | "sh" | "command" | "run" | "execute_command"
        | "run_terminal" | "shell_command" | "cmd" | "run_script" | "execute_bash" => {
            rename(&mut a, &["cmd", "script", "code", "input"], "command");
            "run_command"
        }
        "run_python" | "python" | "execute_python" | "run_code" | "python_exec" | "exec_python" | "code" => {
            let code = a.get("code").or(a.get("script")).or(a.get("source")).or(a.get("command")).and_then(|c| c.as_str()).unwrap_or("").to_string();
            if code.is_empty() || !has("run_command") {
                return None;
            }
            return Some(("run_command".into(), json!({"command": format!("python3 - <<'GG_PY_EOF'\n{code}\nGG_PY_EOF")}),
                         format!("there is no tool \"{name}\"; ran the code with python3 via run_command")));
        }
        "read_file" | "open_file" | "view_file" | "cat" | "file_read" | "read" | "view" | "read_document" | "read_pdf" => {
            rename(&mut a, &["file", "filename", "file_path", "filepath"], "path");
            a["kind"] = json!("file");
            "inspect"
        }
        "wait" | "sleep" | "pause" | "delay" => "__wait",
        "done" | "finish" | "task_complete" | "complete" | "fail" | "terminate" | "stop" | "end" | "give_up" | "infeasible"
        | "submit" | "answer" | "final_answer" | "report_infeasible" => "__final",
        _ => return None,
    };
    if target.starts_with("__") {
        return Some((target.into(), a, String::new()));
    }
    if !has(target) {
        return None;
    }
    Some((target.into(), a, format!("there is no tool \"{name}\"; ran {target}")))
}

/// The closest advertised tool names to an unknown one (shared word parts, then edit distance).
pub fn nearest_tools(name: &str, known: &[String], k: usize) -> Vec<String> {
    let n = normalize_name(name);
    let parts: Vec<&str> = n.split('_').filter(|p| p.len() > 2).collect();
    let mut scored: Vec<(i64, String)> = known.iter().map(|t| {
        let shared = parts.iter().filter(|p| t.contains(*p)).count() as i64;
        (shared * 100 - edit_distance(&n, t) as i64, t.clone())
    }).collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().take(k).map(|x| x.1).collect()
}

pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + if a[i - 1] == b[j - 1] { 0 } else { 1 });
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Extra lines for the server instructions when 0.9.7 features are on (Pointer executor rules, VLAA, II cautions).
pub fn instructions_note() -> String {
    let mut s = String::new();
    if !all_on() && active().is_empty() {
        return s;
    }
    s.push_str("\n[gui-gate 0.9.7] Work like this: (1) decide first whether the task can be done at all on this computer (set_plan with feasible true/false, using the gate's briefing and inspect kind=env); a task that needs a feature the app does not have, missing files or hardware, or a workaround is infeasible: reply FAIL with the reason. (2) Plan milestones as END STATES (\"the sheet is sorted by date and saved\"), not clicks; revise them when you learn more. (3) Prefer the app's own GUI for settings, formatting and document objects; use run_command / inspect for reading files and extracting data. A file that is open in an app must be closed (or reloaded) before you change it in code. Operate on the exact file / app the task names. (4) Two failed attempts with the same mechanism = switch to a genuinely different one (keyboard vs menu vs code). (5) Before DONE, call check_done and fix real gaps.");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> Vec<String> {
        ["click", "double_click", "right_click", "type_text", "press_key", "hotkey", "scroll", "drag", "get_window_state",
         "list_windows", "list_apps", "launch_app", "bring_to_front", "run_command", "inspect"].iter().map(|s| s.to_string()).collect()
    }

    /// Offline: GG_PHANTOM_IN=<file: one "name<TAB>known1,known2,..." per line> GG_PHANTOM_OUT=<tsv>
    /// cargo test -- --ignored phantom_replay -> name, mapped-to (or "-"), nearest tools.
    #[test]
    #[ignore]
    fn phantom_replay() {
        let (Ok(inp), Ok(outp)) = (std::env::var("GG_PHANTOM_IN"), std::env::var("GG_PHANTOM_OUT")) else { return };
        let mut out = String::new();
        for l in std::fs::read_to_string(inp).unwrap().lines() {
            let (name, ks) = l.split_once('\t').unwrap_or((l, ""));
            let known: Vec<String> = ks.split(',').map(String::from).collect();
            let m = phantom_alias(name, &json!({}), &known).map(|x| x.0).unwrap_or_else(|| "-".into());
            out.push_str(&format!("{name}\t{m}\t{}\n", nearest_tools(name, &known, 3).join(",")));
        }
        std::fs::write(outp, out).unwrap();
    }

    #[test]
    fn phantom_names_are_normalised_and_mapped() {
        assert_eq!(normalize_name("triple_click_safe"), "triple_click");
        assert_eq!(normalize_name("triple_click_does_not_exist"), "triple_click");
        assert_eq!(normalize_name("tripleClick"), "triple_click");
        assert_eq!(normalize_name("mcp__gui__click_v2"), "click");
        let k = known();
        assert_eq!(phantom_alias("triple_click_workaround", &json!({"x": 1, "y": 2}), &k).unwrap().0, "__triple_click");
        assert_eq!(phantom_alias("left_click", &json!({"coordinate": [5, 6]}), &k).unwrap().1, json!({"x": 5, "y": 6}));
        let (t, a, _) = phantom_alias("type", &json!({"content": "hi"}), &k).unwrap();
        assert_eq!((t.as_str(), a), ("type_text", json!({"text": "hi"})));
        let (t, a, _) = phantom_alias("key", &json!({"keys": "ctrl+s", "pid": 3}), &k).unwrap();
        assert_eq!(t, "hotkey");
        assert_eq!(a["keys"], json!(["ctrl", "s"]));
        let (t, a, _) = phantom_alias("key", &json!({"key": "Return"}), &k).unwrap();
        assert_eq!((t.as_str(), a["key"].clone()), ("press_key", json!("Return")));
        let (t, a, _) = phantom_alias("run_python", &json!({"code": "print(1)"}), &k).unwrap();
        assert!(t == "run_command" && a["command"].as_str().unwrap().contains("python3 - <<'GG_PY_EOF'\nprint(1)"));
        assert_eq!(phantom_alias("bash", &json!({"cmd": "ls"}), &k).unwrap().1, json!({"command": "ls"}));
        assert_eq!(phantom_alias("read_file", &json!({"file_path": "/a"}), &k).unwrap().1, json!({"path": "/a", "kind": "file"}));
        assert_eq!(phantom_alias("screenshot", &json!({}), &k).unwrap().0, "list_windows");
        assert_eq!(phantom_alias("wait", &json!({"seconds": 2}), &k).unwrap().0, "__wait");
        assert_eq!(phantom_alias("done", &json!({}), &k).unwrap().0, "__final");
        assert!(phantom_alias("frobnicate", &json!({}), &k).is_none());
        // a mapped tool that is not advertised (inspect off) is not used
        let no_inspect: Vec<String> = k.iter().filter(|t| *t != "inspect").cloned().collect();
        assert!(phantom_alias("read_file", &json!({"path": "/a"}), &no_inspect).is_none());
        assert_eq!(nearest_tools("press_the_key", &k, 2)[0], "press_key");
    }
}
