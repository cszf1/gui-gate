//! gui-gate: Cua Driver with a pre-action value gate, served as ONE MCP server.
//!
//!     agent (Claude Code, Codex, Cursor, Pi, ...)  --stdio MCP-->  gui-gate  --stdio-->  cua-driver mcp
//!
//! Every driver tool is exposed (core set by default). Action tools pass through the gate, which executes them,
//! probes cheaply first, waits, repairs a stale token, or answers with a short "[gate] ..." instruction.
//! Observation tools pass straight through and update what the gate knows.
use crate::authz;
use crate::batch;
use crate::decide::*;
use crate::features::*;
use crate::human::Human;
use crate::log::EventLog;
use crate::reviewer::Reviewer;
use crate::rpc::{ChildMCP, Handler, Out};
use crate::state::*;
use crate::updates;
use crate::util::*;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::BufRead;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const CORE_TOOLS: &[&str] = &["list_apps", "list_windows", "get_window_state", "launch_app", "bring_to_front", "click",
    "double_click", "right_click", "type_text", "press_key", "hotkey", "scroll", "set_value", "drag", "invoke_menu",
    "gate_report", "request_human", "gate_task", "run_actions", "run_command", "check_done", "set_plan"];
const RARE_PARAMS: &[&str] = &["session", "capture_mode", "screenshot_out_file", "verbose", "full_output", "display_only",
    "cursor_id", "delivery_mode", "scope", "from_zoom", "capture_id", "timeout_ms", "max_image_dimension", "target"];
const AUTO_OBS_TOOLS: &[&str] = &["click", "double_click", "right_click", "press_key", "hotkey", "scroll", "invoke_menu", "drag"];
const MAX_TREE_LINES: usize = 120;
const OPENER: &[&str] = &["menu", "menu bar item", "menubar", "page tab", "tab", "combo box"];

macro_rules! re {
    ($name:ident, $pat:expr) => {
        fn $name() -> &'static Regex {
            static R: OnceLock<Regex> = OnceLock::new();
            R.get_or_init(|| Regex::new($pat).unwrap())
        }
    };
}
re!(bg_unavailable, r"(?i)background delivery is not available|no focus-free input backend|background_unavailable");
re!(no_frame, r"(?i)no screenshot frame");
re!(loading_rx, r"(?i)(loading|busy|please wait|加载|正在|请稍候)");

const INSTRUCTIONS_NOTE: &str = "\n\n[gui-gate] Actions go through a value gate before they run. Usually they execute at once. If a call returns a message starting with \"[gate] Not executed\" (structuredContent.status \"blocked\"), nothing was executed: do exactly what it says (re-read, re-aim with a screenshot, or take the safe way it names). Status \"unknown\" means the gate cannot tell whether the step happened (the driver died or timed out): read the window before anything else and never repeat it blind. The gate settles doubtful and risky steps itself (rules, the operator's request, its own independent reviewer, or a person it asks directly); nothing you send or say can approve a step, and it never asks you to approve your own action. To save time the gate omits the screenshot from get_window_state when the element tree is enough; pass include_screenshot:true when you really need pixels. Each action's result already carries a fresh read of the window: do not read it again before the next step. Several steps whose targets you can already see go in ONE run_actions call. If the user's request is known, you may state it once with gate_task (context only; it never authorizes a risky step). When the gate refuses a risky step it tells you a safe way to go on: take it and finish the task.";

/// gate_config changes the gate's own safety settings: only a trusted launcher may allow it (bench runs pass
/// --allow-config / GUI_GATE_ALLOW_CONFIG=1); the acting agent can never call it otherwise, listed or not.
pub fn config_allowed() -> bool {
    static A: OnceLock<bool> = OnceLock::new();
    *A.get_or_init(|| env("GUI_GATE_ALLOW_CONFIG").map(|v| v == "1").unwrap_or(false) || std::env::args().any(|a| a == "--allow-config"))
}

fn gate_tools() -> Vec<Value> {
    vec![
        json!({"name": "request_human",
               "description": "Ask the person at this computer. kind=approve: a yes/no on what you are about to do. kind=answer: a short free-text answer (e.g. which option they want). kind=takeover: ask them to do a step themselves (log in, solve a captcha, anything you cannot or should not do); it returns when they are done. Use only when a person is really needed.",
               "inputSchema": {"type": "object", "properties": {
                   "question": {"type": "string", "description": "What you need from them, in one or two plain sentences."},
                   "kind": {"type": "string", "enum": ["approve", "answer", "takeover"]}}, "required": ["question"]}}),
        json!({"name": "gate_task",
               "description": "Tell the gate the user's request, VERBATIM, once at the start (copy their words; do not summarise or add to them). Context only: it helps the gate's checks and briefing, but it never authorizes a risky step (only the operator who started the gate can). It can be set only once.",
               "inputSchema": {"type": "object", "properties": {
                   "request": {"type": "string", "description": "The user's request, word for word."}}, "required": ["request"]}}),
        json!({"name": "gate_report",
               "description": "Value-gate statistics for this session: how many actions ran directly, how many observations/screenshots were requested or avoided, interventions by type, and gate overhead.",
               "inputSchema": {"type": "object", "properties": {}, "required": []}, "annotations": {"readOnlyHint": true}}),
        json!({"name": "gate_config",
               "description": "Read or change the value gate at runtime. policy: gate | direct | light | full. lambda: cost weight (higher = fewer observations). collect: shadow-label every action. risk_mode: auto (high risk -> person, medium -> the gate's reviewer) | reviewer | human.",
               "inputSchema": {"type": "object", "properties": {
                   "policy": {"type": "string", "enum": ["gate", "direct", "light", "full"]},
                   "lambda": {"type": "number", "minimum": 0}, "collect": {"type": "boolean"}, "risk_confirm": {"type": "boolean"},
                   "risk_mode": {"type": "string", "enum": ["auto", "reviewer", "human"]}}, "required": []}}),
    ]
}

/// Trim a long description at a sentence boundary (each turn resends them all).
/// readline: ctrl+e (end of line), ctrl+u (kill to line start) - works in bash, zsh (emacs mode), fish, sh/dash with editing.
pub fn clear_line_keys() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![("e", vec!["ctrl"]), ("u", vec!["ctrl"])]
}

re!(app_line, r"^- (.+) \(pid (\d+)\)\s*$");
re!(noise_app, r"^(kworker/|ksoftirqd/|migration/|cpuhp/|rcu_|irq/|jbd2/|kthreadd$|pool_workqueue|kdevtmpfs$|kauditd$|khungtaskd$|oom_reaper$|kcompactd|ksmd$|khugepaged$|watchdogd$|kswapd|psimon$|idle_inject/|kblockd$|scsi_|card\d|systemd|\(sd-pam\)$|dbus-daemon$|dbus-broker|agetty$|sshd$|sshd-session$|\(udev-worker\)$|containerd|dockerd$|dconf-service$|at-spi|ssh-agent$|gpg-agent$|polkitd$|udisksd$|rtkit-daemon$|cron$|rsyslogd$|gvfs|xdg-permission-|xdg-document-|gui-gate$|cua-driver$)");

/// A process without a command line is a kernel thread (live check on the controlled machine).
pub fn proc_is_kernel_thread(pid: i64) -> bool {
    match std::fs::read(format!("/proc/{pid}/cmdline")) {
        Ok(b) => b.is_empty(),
        Err(_) => false,
    }
}

/// Drop kernel threads and system daemons from a list_apps result (text lines and structured list).
/// Returns how many entries were hidden. Apps the agent can work with (anything with a desktop name, shells,
/// interpreters, terminals) stay.
pub fn filter_apps(res: &mut Value, kthread: &dyn Fn(i64) -> bool) -> usize {
    let noise = |name: &str, pid: i64| noise_app().is_match(name.trim()) || (pid > 2 && kthread(pid));
    let mut hidden = 0;
    if let Some(items) = res.get_mut("content").and_then(|c| c.as_array_mut()) {
        for it in items.iter_mut() {
            let Some(tx) = it.get("text").and_then(|x| x.as_str()) else { continue };
            let mut kept = vec![];
            for line in tx.lines() {
                if let Some(c) = app_line().captures(line) {
                    if noise(&c[1], c[2].parse().unwrap_or(0)) {
                        hidden += 1;
                        continue;
                    }
                }
                kept.push(line);
            }
            it["text"] = json!(kept.join("\n"));
        }
    }
    if let Some(sc) = res.get_mut("structuredContent").and_then(|v| v.as_object_mut()) {
        for (_, v) in sc.iter_mut() {
            if let Some(arr) = v.as_array_mut() {
                if arr.iter().all(|x| x.get("name").is_some() && x.get("pid").is_some()) {
                    arr.retain(|x| !noise(x.get("name").and_then(|n| n.as_str()).unwrap_or(""), as_i64(x.get("pid")).unwrap_or(0)));
                }
            }
        }
    }
    hidden
}

re!(occluded_rx, r#"element \[(\d+)\] \(pid \d+\) has a snapshot frame at \((-?\d+), (-?\d+)\), but that screen point is owned by ([\w ]+?) "([^"]*)" rather than the observed element"#);

/// GUI_GATE_CHECK_DONE=0 removes the check_done tool.
pub fn check_done_enabled() -> bool {
    std::env::var("GUI_GATE_CHECK_DONE").map(|v| v != "0").unwrap_or(true)
}

/// GUI_GATE_PLAN=0 removes set_plan (and its progress line).
pub fn plan_enabled() -> bool {
    std::env::var("GUI_GATE_PLAN").map(|v| v != "0").unwrap_or(true)
}

fn plan_tool() -> Value {
    let mut t = plan_tool_096();
    if crate::v097::on("STATE_PLAN") {
        t["description"] = json!("Your own plan and working memory, kept by the gate. Call it once at the start with your feasibility verdict and 2-6 milestones that describe END STATES, not clicks (\"report.xlsx is sorted by date and saved\", not \"click Data > Sort\"): feasible:false when the task cannot be done on this computer (the app has no such feature, the thing asked for does not exist, files or hardware are missing, it would need a workaround) - then reply FAIL with the reason. Attach checks the gate can verify without you: checks:[{n:1, check:{file_changed:\"~/Desktop/report.xlsx\"}}] (file_exists, file_changed, file_contains:[path,text], title_contains, not_title_contains, window_open, app_running, terminal_contains, cell:[\"B3\",\"value\"]). Revise the plan when you learn something (send milestones again; done marks of unchanged milestones are kept). facts:{key: value} stores values you will need later (null deletes). Every action result ends with a one-line progress note; check_done evaluates the checks.");
        let p = &mut t["inputSchema"]["properties"];
        p["milestones"]["description"] = json!("2-6 end states (replaces the current list)");
        p["checks"] = json!({"type": "array", "items": {"type": "object"}, "description": "[{n: milestone number, check: {predicate: value}}]"});
        p["facts"] = json!({"type": "object", "description": "working memory: {key: value} (value null removes the key)"});
    }
    t
}

fn plan_tool_096() -> Value {
    json!({"name": "set_plan",
        "description": "Your own plan, kept by the gate (no model involved). Call it once at the start with 2-6 short milestones and your feasibility verdict: feasible:false when the task cannot be done on this computer (the app has no such feature or setting, the thing asked for does not exist, the file or data is not there) - then reply FAIL with the reason instead of trying. Later calls mark milestones done (done: [numbers]) or replace the list. Every action result then ends with a one-line progress note, and check_done lists milestones still open.",
        "inputSchema": {"type": "object", "properties": {
            "milestones": {"type": "array", "items": {"type": "string"}, "description": "2-6 short steps (replaces the current list)"},
            "done": {"type": "array", "items": {"type": "integer"}, "description": "numbers (1-based) of milestones now done"},
            "feasible": {"type": "boolean", "description": "false = this task cannot be done here"},
            "reason": {"type": "string", "description": "why (required with feasible:false)"}},
            "required": []}})
}

/// "[plan] 2/5 done; next: 3. save as pre.pptx"
pub fn plan_line(plan: &[(String, bool)]) -> Option<String> {
    if plan.is_empty() {
        return None;
    }
    let done = plan.iter().filter(|m| m.1).count();
    let next = plan.iter().enumerate().find(|(_, m)| !m.1).map(|(i, m)| format!("; next: {}. {}", i + 1, trunc(&m.0, 80))).unwrap_or_else(|| "; all milestones done - call check_done, then reply".into());
    Some(format!("[plan] {done}/{} done{next}", plan.len()))
}

/// 0.9.7 two strikes: the "mechanism" of a step = its mode + what it acts on (label, else keys, else the window),
/// so varying coordinates / arguments on the same control counts as the same mechanism.
pub fn mech_target(ctx: &ActionCtx) -> String {
    let l = ctx.target.label.trim();
    if !l.is_empty() {
        return trunc(l, 40);
    }
    if ctx.tool == "run_command" {
        return trunc(pystr(ctx.args.get("command")).split_whitespace().next().unwrap_or(""), 30);
    }
    if ctx.target.kind == "pixel" {
        return "pixels".into();
    }
    "window".into()
}

/// 0.9.7: fingerprint of a window state (title + the set of element roles/labels of its newest read).
pub fn state_fp(st: &GateState, pid: i64) -> Option<String> {
    use std::hash::{Hash, Hasher};
    let o = st.newest_for_pid(pid, 0.0)?;
    let ob = st.get(o);
    let mut rows: Vec<String> = ob.elements.values().map(|e| format!("{}|{}|{}", e.role, e.label, e.value.clone().unwrap_or_default())).collect();
    rows.sort();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    rows.hash(&mut h);
    Some(format!("{}|{:x}", trunc(&window_title_of(&ob.text), 60), h.finish()))
}

const DESKTOP_WINDOWS: &[&str] = &["Desktop", "xfce4-panel", "xfdesktop", "Xfce4-panel", ""];

/// GUI_GATE_STEP_INSIST=0 turns off "a re-sent doubted step runs".
fn step_insist_enabled() -> bool {
    std::env::var("GUI_GATE_STEP_INSIST").map(|v| v != "0").unwrap_or(true)
}

const FOCUS_HEAL_TOOLS: &[&str] = &["click", "double_click", "right_click", "press_key", "hotkey", "type_text", "scroll", "set_value"];

/// The driver could not focus the target window and says it sent nothing, so one retry cannot double an effect.
pub fn focus_lost(txt: &str) -> bool {
    let head: String = txt.chars().take(600).collect();
    head.contains("foreground_unavailable") && head.contains("no input was sent")
}

// (a shell prompt at the end of terminal text)
re!(prompt_rx, r"[$#%>]\s*$");

/// the terminal text ends at a shell prompt (user@host:dir$, #, %, >)
pub fn at_prompt(t: &str) -> bool {
    t.lines().rev().find(|l| !l.trim().is_empty()).map(|l| prompt_rx().is_match(l)).unwrap_or(false)
}

/// typed text that commits a line / cell (Enter or Tab inside or at the end)
pub fn commits_input(text: &str) -> bool {
    text.contains('\n') || text.contains('\r') || text.contains('\t')
}

/// GUI_GATE_TYPED_LOOK=0: no terminal text / cell changes after committing type_text
pub fn typed_look_enabled() -> bool {
    std::env::var("GUI_GATE_TYPED_LOOK").map(|v| v != "0").unwrap_or(true)
}

pub fn no_window_list_enabled() -> bool {
    std::env::var("GUI_GATE_NO_WINDOW_LIST").map(|v| v != "0").unwrap_or(true)
}

/// list_windows rows minus the desktop and panels, at most 12
pub fn window_rows_for_agent(lw: &str) -> String {
    lw.lines()
        .filter(|l| l.contains("window_id=") && !l.contains("app=Xfdesktop") && !l.contains("app=Xfce4-panel") && !l.contains("\"xfce4-panel\""))
        .map(|l| l.trim().to_string())
        .take(12)
        .collect::<Vec<_>>()
        .join("\n")
}

/// any "owned by another control" refusal, named owner or not
pub fn occluded_any(txt: &str) -> bool {
    txt.contains("has a snapshot frame at") && txt.contains("but that screen point is owned by")
}

/// GUI_GATE_OCCLUSION_BG=0 turns off the focus-free retry of a token click refused for occlusion.
pub fn occlusion_bg_enabled() -> bool {
    std::env::var("GUI_GATE_OCCLUSION_BG").map(|v| v != "0").unwrap_or(true)
}

pub fn occlusion(txt: &str) -> Option<(i64, i64, String, String)> {
    let c = occluded_rx().captures(txt)?;
    Some((c[2].parse().ok()?, c[3].parse().ok()?, c[4].trim().to_string(), c[5].to_string()))
}

/// The menu a row of a tree markdown sits under (nearest less-indented row whose role is "menu").
pub fn parent_menu(tree: &str, idx: i64) -> Option<String> {
    let rowrx = Regex::new(r#"^(\s*)- \[(\d+)\] ([\w ]+?) "([^"]*)""#).ok()?;
    let lines: Vec<&str> = tree.lines().collect();
    let pos = lines.iter().position(|l| rowrx.captures(l).map(|c| c[2].parse::<i64>().ok() == Some(idx)).unwrap_or(false))?;
    let mut ind = rowrx.captures(lines[pos])?[1].len();
    for l in lines[..pos].iter().rev() {
        let Some(c) = rowrx.captures(l) else { continue };
        if c[1].len() < ind {
            if c[3].trim() == "menu" && !c[4].is_empty() {
                return Some(c[4].to_string());
            }
            ind = c[1].len();
        }
    }
    None
}

/// Which rows of a fresh tree to send when the agent already saw `seen` for this window: the rows it has not seen
/// (new or changed, compared verbatim incl. their [N] index, so tokens of unchanged rows stay valid with the new
/// snapshot id) plus their ancestors for context. None = send the whole tree (nothing seen yet, or >= thr changed).
pub fn tree_delta(seen: &HashSet<String>, cur: &[&str], thr: f64) -> Option<Vec<usize>> {
    if seen.is_empty() || cur.is_empty() {
        return None;
    }
    let changed: Vec<usize> = (0..cur.len()).filter(|&i| !cur[i].trim().is_empty() && !seen.contains(cur[i])).collect();
    if changed.len() as f64 >= thr * cur.len() as f64 {
        return None;
    }
    let indent = |l: &str| l.len() - l.trim_start().len();
    let mut keep: BTreeSet<usize> = changed.iter().copied().collect();
    for &i in &changed {
        let mut ind = indent(cur[i]);
        for j in (0..i).rev() {
            if ind == 0 {
                break;
            }
            let d = indent(cur[j]);
            if d < ind && !cur[j].trim().is_empty() {
                keep.insert(j);
                ind = d;
            }
        }
    }
    Some(keep.into_iter().collect())
}

/// GUI_GATE_STALL_HINTS=0 turns off the read-stall hints.
/// Driver advice that assumes eyes ("confirm with a screenshot") rewritten for a text-only client.
pub fn text_only_rewrite(res: &mut Value) {
    if let Some(arr) = res.get_mut("content").and_then(|c| c.as_array_mut()) {
        for c in arr.iter_mut() {
            if let Some(t) = c.get("text").and_then(|t| t.as_str()) {
                if t.contains("confirm with a screenshot") {
                    let n = t.replace("confirm with a screenshot", "confirm from the tree");
                    c["text"] = json!(n);
                }
            }
        }
    }
}

re!(partial_advice_rx, r"If the element you need is absent, retry with a larger timeout_ms[^\n]*?max_depth\.");
pub const GRID_LISTED: &str = "The sheet's cell grid is the container that was not expanded; its visible cells are listed by the gate at the end of this result (a larger timeout_ms, max_elements or a query does not list more of the grid; move the cursor or scroll to see other cells).";

/// The driver's PARTIAL TREE advice sends the model re-reading with timeout_ms / query for cells (m3: 125 such
/// re-reads in 23 gate8 runs). Once the gate attached the visible cells, say so instead.
pub fn rewrite_partial_advice(res: &mut Value) -> bool {
    let mut done = false;
    if let Some(arr) = res.get_mut("content").and_then(|c| c.as_array_mut()) {
        for c in arr.iter_mut() {
            if let Some(t) = c.get("text").and_then(|t| t.as_str()) {
                if partial_advice_rx().is_match(t) {
                    let n = partial_advice_rx().replace(t, GRID_LISTED).to_string();
                    c["text"] = json!(n);
                    done = true;
                }
            }
        }
    }
    done
}

/// GUI_GATE_FOCUS_BG=0 turns off the focus-free retry of a token click.
pub fn focus_bg_enabled() -> bool {
    std::env::var("GUI_GATE_FOCUS_BG").map(|v| v != "0").unwrap_or(true)
}

pub fn stall_hints_enabled() -> bool {
    std::env::var("GUI_GATE_STALL_HINTS").map(|v| v != "0").unwrap_or(true)
}

pub const HINT_GRID: &str = "[gate] Spreadsheet cells are not in the accessibility tree (the grid is too big to list), so no read or query will show cell contents. Work by keyboard instead: ctrl+Home goes to A1, arrows / Tab / Enter move, the action result names the focused cell (\"focus: cell B3\"); type values or formulas straight into the focused cell. Use column headers you already know from the task.";
pub const HINT_TERMINAL: &str = "[gate] This terminal does not expose its text: no read shows the command output, so re-reading will not help. Enter runs the typed line. Check a command's effect in another window whose tree shows it (for example a file manager listing a folder), or, when the command you ran is clearly right, finish.";

/// One-time, cause-specific notes for a read whose content is not in the tree (per window and cause).
/// (Not when the gate already attached the cells / the terminal text from AT-SPI.)
pub fn read_hints(txt: &str, terminal: bool, key: &str, hinted: &mut HashSet<String>) -> Vec<&'static str> {
    let mut out = vec![];
    let cells_shown = txt.contains("[gate] Visible cells") || txt.contains("[gate] Cells changed");
    if txt.contains("cell grid") && !cells_shown && hinted.insert(format!("grid:{key}")) {
        out.push(HINT_GRID);
    }
    if terminal && !txt.contains("[gate] Terminal text") && hinted.insert(format!("term:{key}")) {
        out.push(HINT_TERMINAL);
    }
    out
}

re!(win_title_rx, r#"(?m)^\s*- (?:frame|window|dialog)[^"\n]*"([^"\n]+)""#);

/// the window title from a tree's first frame row
pub fn window_title_of(md: &str) -> String {
    win_title_rx().captures(md).map(|c| c[1].to_string()).unwrap_or_default()
}

/// The tree says a spreadsheet grid was not expanded.
pub fn grid_not_listed(txt: &str) -> bool {
    txt.contains("cell grid") || (txt.contains("PARTIAL TREE") && txt.contains("LibreOffice Calc"))
}

/// LibreOffice (the driver's PARTIAL TREE note names "a spreadsheet's cell grid" for any big container, so the app
/// decides whether a sheet grid is worth looking for)
pub fn is_spreadsheet_app(pid: i64) -> bool {
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default().to_lowercase();
    let cmd = std::fs::read_to_string(format!("/proc/{pid}/cmdline")).unwrap_or_default().to_lowercase();
    comm.starts_with("soffice") || cmd.contains("libreoffice") || comm.contains("gnumeric")
}

/// Every 3rd read in a row without an action in between.
pub fn streak_hint(streak: u32) -> Option<String> {
    if streak >= 3 && streak % 3 == 0 {
        Some(format!("[gate] {streak} reads in a row without an action: the tree only changes after an action. What is not in it now is not exposed to the tree; act on what is there (or reply FAIL if the task cannot be done this way)."))
    } else {
        None
    }
}

pub fn short(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let cut: String = text.chars().take(limit).collect();
    let k = [cut.rfind(". "), cut.rfind(".\n"), cut.rfind('。')].into_iter().flatten().max();
    match k {
        Some(k) if (cut[..k].chars().count() as f64) > limit as f64 * 0.4 => {
            let end = k + cut[k..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            cut[..end].to_string()
        }
        _ => format!("{} ...", cut.trim_end()),
    }
}

/// structuredContent.status, so a client never has to guess from the text whether something ran:
///   executed (the driver ran it; isError tells whether it reported a failure), blocked (the gate ran nothing),
///   partial (run_actions stopped after some steps; batch.executed says how many ran), done (a non-GUI tool answered).
const PERMIT_TTL_S: f64 = 30.0;

/// The result of an action whose outcome the gate cannot know (status "unknown").
pub fn unknown_result(name: &str, why: &str) -> Value {
    json!({"isError": true,
           "content": [{"type": "text", "text": format!("[gate] Outcome unknown: {why} while `{name}` was being delivered, so the gate cannot tell whether it happened. It was NOT sent again. Read the window to see whether it took effect; send it again only if it did not (it will be decided afresh).")}],
           "structuredContent": {"status": "unknown", "outcome": "unknown", "unknown_reason": why}})
}

pub fn outcome_unknown(r: &Value) -> bool {
    r.get("structuredContent").and_then(|x| x.get("status")).and_then(|x| x.as_str()) == Some("unknown")
}

pub fn set_status(name: &str, r: &mut Value) {
    let sc = r.get("structuredContent").cloned().unwrap_or(json!({}));
    let preset = s(&sc, "status");
    let st = if preset == "unknown" || preset == "input_required" {
        if preset == "unknown" { "unknown" } else { "input_required" }
    } else if sc.get("batch").and_then(|b| b.get("unknown_step")).map(|x| !x.is_null()).unwrap_or(false) {
        "unknown"
    } else if sc.get("gate").and_then(|g| g.get("executed")) == Some(&json!(false)) {
        "blocked"
    } else if let Some(b) = sc.get("batch") {
        if b.get("stopped").map(|x| !x.is_null()).unwrap_or(false) { if as_i64(b.get("executed")).unwrap_or(0) > 0 { "partial" } else { "blocked" } } else { "executed" }
    } else if is_gated(name) {
        if text_of(r).starts_with("[gate] Not executed") { "blocked" } else { "executed" }
    } else {
        "done"
    };
    if !r.get("structuredContent").map(|v| v.is_object()).unwrap_or(false) {
        r["structuredContent"] = json!({});
    }
    r["structuredContent"]["status"] = json!(st);
}

pub fn refusal(dec: &Decision, text: String, extra: Option<Obj>) -> Value {
    let mut g = json!({"decision": dec.option, "tier": dec.tier, "p_direct": round_to(dec.p_direct, 3),
                       "reason": dec.reason, "executed": false});
    if let Some(e) = extra {
        for (k, v) in e {
            g[k] = v;
        }
    }
    json!({"isError": true, "content": [{"type": "text", "text": text}], "structuredContent": {"gate": g}})
}

/// Cases where any extra look costs more than a miss: opening a menu or tab by its tree row, or a pixel click
/// planned on a screenshot the model got right before it, with nothing done since.
pub fn fast_path(ctx: &ActionCtx, f: &Feats) -> Option<String> {
    let t = &ctx.target;
    let role = t.the_role().to_lowercase();
    if matches!(t.kind.as_str(), "token" | "index" | "name") && OPENER.contains(&role.as_str()) && ctx.tool == "click" {
        return Some(format!("opens a {role}: harmless if wrong (Escape), the result is shown right after"));
    }
    let g = |k: &str| f.get(k).copied();
    if t.kind == "pixel" && g("obs_had_shot").unwrap_or(0.0) >= 1.0 && g("acts_since_obs").unwrap_or(1.0) == 0.0
        && g("window_changed").unwrap_or(0.0) < 0.5 && g("last_err").unwrap_or(0.0) < 0.5 && g("repeat").unwrap_or(0.0) < 0.5 {
        return Some("pixel target planned on a fresh screenshot, nothing changed since".into());
    }
    None
}

/// Several rows share the role+label: keep the one where the old row was (same place, else nearest index).
pub fn nearest(old: &Element, cands: Vec<Element>) -> Vec<Element> {
    let ctr = |e: &Element| e.frame.as_ref().map(|f| (f.x + f.w / 2.0, f.y + f.h / 2.0));
    if let Some(oc) = ctr(old) {
        if cands.iter().all(|c| ctr(c).is_some()) {
            let mut d: Vec<(f64, usize)> = cands.iter().enumerate()
                .map(|(i, c)| { let cc = ctr(c).unwrap(); ((cc.0 - oc.0).abs() + (cc.1 - oc.1).abs(), i) }).collect();
            d.sort_by(|a, b| a.partial_cmp(b).unwrap());
            if d[0].0 <= 8.0 && (d.len() == 1 || d[1].0 > 30.0) {
                return vec![cands[d[0].1].clone()];
            }
            return cands;
        }
    }
    let mut d: Vec<(i64, usize)> = cands.iter().enumerate().map(|(i, c)| ((c.index - old.index).abs(), i)).collect();
    d.sort();
    if d.len() == 1 || d[1].0 > d[0].0 + 5 {
        return vec![cands[d[0].1].clone()];
    }
    cands
}

pub fn ids_of(sig: Option<&str>) -> HashSet<String> {
    sig.unwrap_or("").split(';').filter(|x| !x.is_empty()).map(|x| x.split('@').next().unwrap_or("").to_string()).collect()
}

fn ids_list(sig: &str) -> Vec<String> {
    sig.split(';').filter(|x| !x.is_empty()).map(|x| x.split('@').next().unwrap_or("").to_string()).collect()
}

/// What the child reader thread needs (never the big lock: it must not wait on a gated call).
pub struct Shared {
    /// request id -> (method, client used the MCP 2026-07-28 per-request `_meta` era)
    pub pending_methods: HashMap<String, (String, bool)>,
    /// the driver's answer to the gate's own legacy `initialize` (raw, not rewritten)
    pub driver_init: Option<Value>,
    pub update_note: String,
    pub update_told: bool,
    pub driver_version: Option<String>,
    pub delivery_tools: HashSet<String>,
    pub compat_warned: bool,
    /// 0.9.7: the tool names advertised to the agent (phantom-tool aliasing) and click's argument names
    pub tool_names: Vec<String>,
    pub click_props: Vec<String>,
}

/// A one-time permission for exactly one dispatch of an approved risky action: bound to the final action's signature
/// and its full structured effect (tool, hazard, resources, app, window, payload), short-lived, removed on use.
#[derive(Clone, Debug)]
pub struct Permit {
    pub sig: String,
    pub key: crate::effect::EffectKey,
    pub expires: f64,
}

/// Everything a gated call reads and writes, under one lock.
pub struct Inner {
    pub cfg: GateConfig,
    pub state: GateState,
    pub stats: Obj,
    pub aid: u64,
    pub want_shot: HashSet<i64>,
    pub seen_pids: HashSet<i64>,
    pub fg_sticky: bool,
    pub pixel_pids: HashSet<i64>,
    pub probe_disabled: bool,
    /// refused effects (a record, never a credential): (structured key, who refused); covers the same effect by any route
    pub denials: Vec<(crate::effect::EffectKey, String)>,
    /// one-shot dispatch permits gated() issued for an approved risky effect (see Gate::tool's final check)
    pub permits: Vec<Permit>,
    /// the risky effect the current gated() call has approval for, and from whom (operator task, reviewer, person)
    pub approval: Option<(crate::effect::EffectKey, String)>,
    /// what the agent itself said the user's request is (gate_task): context only, it never authorizes anything
    pub agent_task: String,
    /// signatures of actions whose outcome was unknown (driver died / timed out / unparseable): never re-sent blind
    pub unknown_sigs: HashMap<String, String>,
    pub step_doubted: HashSet<String>,
    pub acted: bool,
    pub read_streak: u32,             // reads since the last GUI action (stall hint)
    pub hinted: HashSet<String>,      // one-time read hints already given (cause:window)
    pub cells_seen: HashMap<String, (String, Vec<(String, String)>)>, // window -> (range, cells) last shown
    pub term_seen: HashMap<String, String>, // window -> terminal text last shown
 // signatures the step reviewer doubted (look_light) and the agent has not re-sent yet
    pub denials_consec: u32,
    pub denials_total: u32,
    pub task_locked: bool,
    pub task_source: Option<String>,
    pub in_batch: bool,
    pub batch_ctx: Value,
    pub pix_age: HashMap<i64, i64>,
    pub last_auto: HashMap<i64, Value>,
    /// tree rows the agent has already been shown, per "pid:window_id" (for auto-observe diffs): (snapshot, rows)
    pub seen_rows: HashMap<String, (String, HashSet<String>)>,
    /// the last few executed actions, summarised for the reviewer
    pub recent: Vec<Value>,
    pub lb: crate::loopbreak::LoopTrack,
    pub plan: Vec<(String, bool)>,   // set_plan milestones (text, done)
    pub plan_verdict: String,
    /// 0.9.7: milestone predicates (parallel to plan), working-memory facts, briefing state
    pub plan_checks: Vec<Option<Value>>,
    pub facts: Vec<(String, String)>,
    pub briefed: bool,
    pub history: Vec<String>,
    pub verdict_nagged: bool,
    pub risk_info: Value,
    pub cur_ack: bool,
}

pub struct Gate {
    pub inner: Mutex<Inner>,
    pub shared: Arc<Mutex<Shared>>,
    pub out: Arc<Out>,
    pub log: Arc<EventLog>,
    child: Mutex<Arc<ChildMCP>>,
    child_cmd: Vec<String>,
    client_elicit: Arc<AtomicBool>,
    client_pending: Arc<Mutex<HashMap<String, Sender<Value>>>>,
    pub restarts: AtomicU64,
    pub human: Human,
    pub reviewer: Reviewer,
    started: f64,
    toolset: String,
    batch_mode: String,
    pub shot: crate::shot::ShotCfg,
    /// 0.9.7: optional model roles (feasibility, planner, verifier, judge, search, compact, grounder) and jobs
    pub roles: crate::roles::Roles,
    pub jobs: crate::jobs::Jobs,
}

fn new_stats() -> Obj {
    let mut m = Obj::new();
    for k in ["actions", "executed", "obs_calls", "screenshots", "screenshot_px", "tree_reads", "model_turns_requested",
              "exec_errors", "heals", "checks", "screenshots_skipped", "reviews", "review_allowed", "auto_obs",
              "human_asked", "human_allowed", "img_bytes_in", "img_bytes_out"] {
        m.insert(k.into(), json!(0));
    }
    m.insert("by_option".into(), json!({}));
    m.insert("gate_ms".into(), json!(0.0));
    m.insert("driver_ms".into(), json!(0.0));
    m.insert("img_ms".into(), json!(0.0));
    m
}

impl Inner {
    pub fn inc(&mut self, k: &str) {
        self.add(k, 1.0);
    }
    pub fn add(&mut self, k: &str, by: f64) {
        let cur = self.stats.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let isint = self.stats.get(k).map(|v| v.is_i64() || v.is_u64()).unwrap_or(true) && by.fract() == 0.0;
        self.stats.insert(k.into(), if isint { json!((cur + by) as i64) } else { json!(cur + by) });
    }
    pub fn stat(&self, k: &str) -> f64 {
        self.stats.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0)
    }
}

fn spawn_child(cmd: &[String], shared: Arc<Mutex<Shared>>, out: Arc<Out>, toolset: String, batch_mode: String) -> Arc<ChildMCP> {
    let h: Handler = Arc::new(move |msg: Value| from_child(msg, &shared, &out, &toolset, &batch_mode));
    match ChildMCP::spawn(cmd, h, env("GUI_GATE_DEBUG").is_some()) {
        Ok(c) => c,
        Err(e) => {
            log_err(&format!("cannot start the driver ({}): {e}", cmd.join(" ")));
            std::process::exit(2);
        }
    }
}

/// Responses from the driver to the agent's own requests: rewrite initialize and tools/list, and give modern-era
/// requests the 2026-07-28 result envelope (the driver itself only ever sees legacy requests from the gate).
fn from_child(mut msg: Value, shared: &Arc<Mutex<Shared>>, out: &Arc<Out>, toolset: &str, batch_mode: &str) {
    let mid = msg.get("id").map(|v| v.to_string());
    if let (Some(mid), true) = (mid, msg.get("method").is_none()) {
        let pend = shared.lock().unwrap().pending_methods.remove(&mid);
        if let Some((meth, modern)) = pend {
            if msg.get("result").map(|r| r.is_object()).unwrap_or(false) {
                match meth.as_str() {
                    "tools/list" => augment_tools(&mut msg["result"], shared, toolset, batch_mode),
                    "initialize" => {
                        let mut sh = shared.lock().unwrap();
                        if sh.driver_init.is_none() {
                            sh.driver_init = Some(msg["result"].clone());
                        }
                        rewrite_init(&mut msg["result"], &mut sh);
                    }
                    _ => {}
                }
                if modern {
                    let sh = shared.lock().unwrap();
                    modern_envelope(&mut msg["result"], &meth, &sh);
                }
            }
        }
    }
    out.send(&msg);
}

/// The driver's initialize result as the agent sees it: gate instructions appended, gate serverInfo.
fn rewrite_init(r: &mut Value, sh: &mut Shared) {
    r["instructions"] = json!(gate_instructions(r, sh));
    let mut si = r.get("serverInfo").cloned().filter(|v| v.is_object()).unwrap_or(json!({}));
    let dv = si.get("version").map(|v| pystr(Some(v)));
    sh.driver_version = dv.clone();
    si["name"] = json!(format!("gui-gate (cua-driver {})", dv.unwrap_or_else(|| "?".into())));
    si["version"] = json!(VERSION);
    r["serverInfo"] = si;
}

fn gate_instructions(driver_init: &Value, sh: &mut Shared) -> String {
    let mut ins = pystr(driver_init.get("instructions")) + INSTRUCTIONS_NOTE + &crate::v097::instructions_note();
    if !sh.update_note.is_empty() {
        ins.push('\n');
        ins.push_str(&sh.update_note);
        sh.update_told = true;
    }
    ins
}

fn gate_server_info(sh: &Shared) -> Value {
    json!({"name": format!("gui-gate (cua-driver {})", sh.driver_version.clone().unwrap_or_else(|| "?".into())),
           "version": VERSION})
}

// ------------------------------------------------------------------ MCP 2026-07-28 (per-request `_meta`)
/// Modern protocol versions the gate handles end to end. The gate terminates the modern era itself: it validates
/// the per-request metadata, talks legacy MCP (after its own `initialize`) to the driver, and adds the modern result
/// envelope, so this does not depend on the driver's own modern support.
pub const MODERN_VERSIONS: &[&str] = &["2026-07-28"];
pub const PV_KEY: &str = "io.modelcontextprotocol/protocolVersion";
pub const CC_KEY: &str = "io.modelcontextprotocol/clientCapabilities";

fn rpc_error(id: Value, code: i64, message: &str, data: Option<Value>) -> Value {
    let mut e = json!({"code": code, "message": message});
    if let Some(d) = data {
        e["data"] = d;
    }
    json!({"jsonrpc": "2.0", "id": id, "error": e})
}

/// Does this request carry modern per-request metadata?
pub fn is_modern(msg: &Value) -> bool {
    let meta = msg.get("params").and_then(|p| p.get("_meta"));
    meta.map(|m| m.get(PV_KEY).is_some() || m.get(CC_KEY).is_some()).unwrap_or(false)
        || msg.get("method").and_then(|m| m.as_str()) == Some("server/discover")
}

/// Same rules as the 2026-07-28 stdio servers (cua-driver's mcp_wire): Ok(true) for a valid modern request,
/// Ok(false) for a legacy one, Err(error response) otherwise.
pub fn classify(msg: &Value) -> Result<bool, Value> {
    let id = msg.get("id").cloned().unwrap_or(Value::Null);
    let meta = msg.get("params").and_then(|p| p.get("_meta"));
    if !is_modern(msg) {
        if meta.map(|m| !m.is_object()).unwrap_or(false) {
            return Err(rpc_error(id, -32602, "MCP _meta must be an object", None));
        }
        return Ok(false);
    }
    let id_ok = id.is_string() || id.is_i64() || id.is_u64();
    if msg.get("jsonrpc").and_then(|v| v.as_str()) != Some("2.0") || !id_ok {
        return Err(rpc_error(Value::Null, -32600, "Invalid JSON-RPC request", None));
    }
    if !msg.get("params").map(|p| p.is_object()).unwrap_or(false) {
        return Err(rpc_error(id, -32602, "MCP params must be an object", None));
    }
    let Some(ver) = meta.and_then(|m| m.get(PV_KEY)).and_then(|v| v.as_str()) else {
        return Err(rpc_error(id, -32602, "Missing or invalid MCP protocolVersion metadata", None));
    };
    if !MODERN_VERSIONS.contains(&ver) {
        return Err(rpc_error(id, -32022, "Unsupported protocol version",
                             Some(json!({"supported": MODERN_VERSIONS, "requested": ver}))));
    }
    if !meta.and_then(|m| m.get(CC_KEY)).map(|c| c.is_object()).unwrap_or(false) {
        return Err(rpc_error(id, -32602, "Missing or invalid MCP clientCapabilities metadata", None));
    }
    Ok(true)
}

/// Drop the modern keys from `params._meta` so the (gate-initialized) driver sees a plain legacy request.
fn strip_modern_meta(msg: &mut Value) {
    let Some(params) = msg.get_mut("params").and_then(|p| p.as_object_mut()) else { return };
    let empty = match params.get_mut("_meta").and_then(|m| m.as_object_mut()) {
        Some(m) => {
            m.remove(PV_KEY);
            m.remove(CC_KEY);
            m.is_empty()
        }
        None => false,
    };
    if empty {
        params.remove("_meta");
    }
}

/// The 2026-07-28 result envelope: resultType "complete" and the gate as serverInfo (tool content untouched).
fn modern_envelope(result: &mut Value, method: &str, sh: &Shared) {
    let Some(o) = result.as_object_mut() else { return };
    if o.get("resultType").and_then(|v| v.as_str()) != Some("input_required") {
        o.insert("resultType".into(), json!("complete"));
    }
    let meta = o.entry("_meta").or_insert_with(|| json!({}));
    if !meta.is_object() {
        *meta = json!({});
    }
    meta["io.modelcontextprotocol/serverInfo"] = gate_server_info(sh);
    if method == "server/discover" || method.ends_with("/list") || method.ends_with("/read") {
        o.entry("ttlMs").or_insert(json!(0));
        o.entry("cacheScope").or_insert(json!("private"));
    }
}

/// The gate's own legacy handshake with a freshly started driver, so every gate-built request is valid whatever
/// era the agent speaks (2026-07-28 drivers reject metadata-free requests before a legacy initialize).
fn handshake(c: &ChildMCP, shared: &Arc<Mutex<Shared>>) {
    let r = c.call("initialize", Some(json!({"protocolVersion": "2025-06-18", "capabilities": {},
        "clientInfo": {"name": "gui-gate", "version": VERSION}})), 30.0);
    let _ = c.forward(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    match r.get("result").filter(|v| v.is_object()) {
        Some(res) => {
            let mut sh = shared.lock().unwrap();
            sh.driver_version = res.get("serverInfo").and_then(|s| s.get("version")).map(|v| pystr(Some(v)));
            sh.driver_init = Some(res.clone());
        }
        None => log_err(&format!("driver initialize failed: {}",
                                 r.get("error").and_then(|e| e.get("message")).map(|m| pystr(Some(m))).unwrap_or_default())),
    }
}

fn augment_tools(result: &mut Value, shared: &Arc<Mutex<Shared>>, toolset: &str, batch_mode: &str) {
    let mut tools: Vec<Value> = result.get("tools").and_then(|t| t.as_array()).cloned().unwrap_or_default();
    let mut sh = shared.lock().unwrap();
    for t in tools.iter_mut() {
        let name = s(t, "name").to_string();
        if !t.get("inputSchema").map(|v| v.is_object()).unwrap_or(false) {
            t["inputSchema"] = json!({"type": "object", "properties": {}});
        }
        let sch = &mut t["inputSchema"];
        if !sch.get("required").map(|v| v.is_array()).unwrap_or(false) {
            sch["required"] = json!([]); // some OpenAI-compatible backends reject schemas without it
        }
        if !sch.get("properties").map(|v| v.is_object()).unwrap_or(false) {
            sch["properties"] = json!({});
        }
        if sch["properties"].get("delivery_mode").is_some() {
            sh.delivery_tools.insert(name.clone());
        }
        if name == "click" {
            sh.click_props = sch["properties"].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        }
        let required: Vec<String> = sch["required"].as_array().unwrap().iter().map(|v| pystr(Some(v))).collect();
        if toolset == "core" {
            let props = sch["properties"].as_object_mut().unwrap();
            props.retain(|k, _| !(RARE_PARAMS.contains(&k.as_str()) && !required.contains(k)));
            for pv in props.values_mut() {
                if let Some(d) = pv.get("description").and_then(|d| d.as_str()).map(String::from) {
                    pv["description"] = json!(short(&d, 140));
                }
            }
            let d = pystr(t.get("description"));
            t["description"] = json!(short(&d, 600));
        }
        let props = t["inputSchema"]["properties"].as_object_mut().unwrap();
        if name == "get_window_state" {
            props.insert("fresh".into(), json!({"type": "boolean", "description": "Force a new read even though nothing was done since the tree that came back with your last action."}));
        }
        if name == "list_apps" {
            props.insert("all".into(), json!({"type": "boolean", "description": "Also list kernel threads and system daemons (hidden by default)."}));
        }
    }
    let names: Vec<String> = tools.iter().map(|t| s(t, "name").to_string()).collect();
    let miss = updates::missing_tools(&names);
    if !miss.is_empty() && !sh.compat_warned {
        sh.compat_warned = true;
        log_err(&format!("this Cua Driver lacks tools the gate needs: {}; run `gui-gate update --to {}`", miss.join(", "), updates::TESTED));
    }
    let mut all = tools;
    all.extend(gate_tools().into_iter().filter(|t| s(t, "name") != "gate_config" || config_allowed()));
    if crate::command::enabled() {
        let mut rc = crate::command::tool_schema();
        if crate::v097::on("JOBS") {
            crate::command::add_job_modes(&mut rc);
        }
        all.push(rc);
    }
    if check_done_enabled() {
        all.push(crate::done::tool_schema());
    }
    if plan_enabled() {
        all.push(plan_tool());
    }
    if batch_mode != "off" {
        let mut ra = batch::run_actions_tool();
        if crate::v097::on("PICK") {
            batch::add_pick_one(&mut ra);
        }
        all.push(ra);
    }
    if crate::v097::on("INSPECT") {
        all.push(crate::inspect::tool_schema());
    }
    // every tool that exists (also driver tools the core toolset hides): only names outside it are phantoms
    sh.tool_names = all.iter().map(|t| s(t, "name").to_string()).collect();
    if toolset == "core" {
        all.retain(|t| CORE_TOOLS.contains(&s(t, "name")) || s(t, "name") == "inspect");
    }
    result["tools"] = Value::Array(all);
}

impl Gate {
    pub fn new(child_cmd: Vec<String>, mut cfg: GateConfig, log_path: Option<String>) -> Arc<Gate> {
        let out = Arc::new(Out::new());
        let shared = Arc::new(Mutex::new(Shared {
            pending_methods: HashMap::new(), driver_init: None, update_note: String::new(), update_told: false, driver_version: None,
            delivery_tools: ["click", "double_click", "right_click", "drag", "type_text", "press_key", "hotkey", "set_value", "scroll"]
                .iter().map(|s| s.to_string()).collect(),
            compat_warned: false, tool_names: vec![], click_props: vec![],
        }));
        let client_pending: Arc<Mutex<HashMap<String, Sender<Value>>>> = Arc::new(Mutex::new(HashMap::new()));
        let client_elicit = Arc::new(AtomicBool::new(false));
        let (o2, cp2) = (out.clone(), client_pending.clone());
        let cids = Arc::new(AtomicU64::new(0));
        // one request to the MCP client (elicitation for a person)
        let client_request: Arc<dyn Fn(&str, Value, f64) -> Option<Value> + Send + Sync> = Arc::new(move |method: &str, params: Value, timeout: f64| {
            let cid = format!("gate-c-{}", cids.fetch_add(1, Ordering::SeqCst) + 1);
            let (tx, rx) = channel();
            cp2.lock().unwrap().insert(cid.clone(), tx);
            o2.send(&json!({"jsonrpc": "2.0", "id": cid, "method": method, "params": params}));
            match rx.recv_timeout(Duration::from_secs_f64(timeout)) {
                Ok(m) => m.get("result").cloned(),
                Err(_) => {
                    cp2.lock().unwrap().remove(&cid);
                    None
                }
            }
        });
        let cr = client_request.clone();
        let elicit: crate::human::Elicit = Arc::new(move |params: Value, timeout: f64| cr("elicitation/create", params, timeout));
        let rv = Reviewer::from_env();
        cfg.reviewer = rv.available();
        let ce = client_elicit.clone();
        let human = Human::new(&cfg.human_channel, cfg.human_timeout, Some(elicit), Arc::new(move || ce.load(Ordering::SeqCst)));
        let toolset = cfg.toolset.clone();
        let batch_mode = cfg.batch.clone();
        let child = spawn_child(&child_cmd, shared.clone(), out.clone(), toolset.clone(), batch_mode.clone());
        handshake(&child, &shared);
        let task_locked = !cfg.task.is_empty();
        let task_source = if task_locked { Some("operator".to_string()) } else { None };
        Arc::new(Gate {
            inner: Mutex::new(Inner {
                state: GateState::with_text_only(cfg.text_only), cfg, stats: new_stats(), aid: 0,
                want_shot: HashSet::new(), seen_pids: HashSet::new(), fg_sticky: false, pixel_pids: HashSet::new(),
                probe_disabled: false, denials: vec![], permits: vec![], approval: None, agent_task: String::new(), unknown_sigs: HashMap::new(), step_doubted: HashSet::new(), acted: false, read_streak: 0, hinted: HashSet::new(), cells_seen: HashMap::new(), term_seen: HashMap::new(), denials_consec: 0, denials_total: 0,
                task_locked, task_source, in_batch: false, batch_ctx: Value::Null, pix_age: HashMap::new(),
                last_auto: HashMap::new(), seen_rows: HashMap::new(), recent: vec![], plan: vec![], plan_verdict: String::new(), plan_checks: vec![], facts: vec![], briefed: false, history: vec![], verdict_nagged: false, lb: Default::default(), risk_info: Value::Null, cur_ack: false,
            }),
            shared, out, log: Arc::new(EventLog::new(log_path)), child: Mutex::new(child), child_cmd,
            client_elicit, client_pending, restarts: AtomicU64::new(0), human,
            reviewer: rv, started: now(), toolset, batch_mode, shot: crate::shot::ShotCfg::from_env(),
            roles: crate::roles::Roles::from_env(), jobs: Default::default(),
        })
    }

    fn child(&self) -> Arc<ChildMCP> {
        self.child.lock().unwrap().clone()
    }

    /// The driver died: start a new one and redo the gate's own handshake (the agent never notices).
    fn respawn(&self) {
        let old = self.child();
        old.close();
        let c = spawn_child(&self.child_cmd, self.shared.clone(), self.out.clone(), self.toolset.clone(), self.batch_mode.clone());
        *self.child.lock().unwrap() = c.clone();
        let n = self.restarts.fetch_add(1, Ordering::SeqCst) + 1;
        handshake(&c, &self.shared);
        self.log.write(json!({"ev": "driver_restart", "n": n}));
        log_err("driver restarted");
    }

    /// A driver tool call with input delivery handled: background when the desktop supports it, else a sticky
    /// fallback to foreground for the rest of the session.
    ///
    /// The single final dispatch check: every GUI action the gate sends to the driver passes here (the gated direct
    /// path, its focus/occlusion/background re-sends, heal_occlusion's own clicks and key presses, batch steps, the
    /// terminal line clearing). For the FINAL arguments the ActionCtx is rebuilt; a risky or hand-off target is
    /// refused unless gated() issued a one-shot permit for exactly this signature and structured effect.
    /// After a send whose outcome is unknown (driver died, timed out, unparseable answer) nothing is re-sent.
    pub fn tool(&self, inn: &mut Inner, name: &str, args: &Value, timeout: f64) -> Value {
        let action = is_gated(name) && name != "run_command";
        if action {
            if let Some(refused) = self.dispatch_check(inn, name, args) {
                return refused;
            }
        }
        let delivery_tool = self.shared.lock().unwrap().delivery_tools.contains(name);
        let mut args = args.clone();
        if delivery_tool && args.get("delivery_mode").is_none() {
            let mode = inn.cfg.delivery.clone();
            if mode == "foreground" || (mode == "auto" && inn.fg_sticky) {
                args["delivery_mode"] = json!("foreground");
            }
        }
        let mut r = self.tool_raw(name, &args, timeout, action);
        // (the driver said nothing was sent: background input is unavailable on this desktop)
        if inn.cfg.delivery == "auto" && is_error(&r) && delivery_tool && s(&args, "delivery_mode") != "foreground"
            && bg_unavailable().is_match(&text_of(&r)) {
            if !inn.fg_sticky {
                inn.fg_sticky = true;
                self.log.write(json!({"ev": "delivery_fallback", "tool": name, "to": "foreground"}));
            }
            inn.inc("delivery_fallbacks");
            args["delivery_mode"] = json!("foreground");
            r = self.tool_raw(name, &args, timeout, action);
        }
        if action && outcome_unknown(&r) {
            let sig = build_ctx(name, &args, &inn.state).signature;
            let why = pystr(sc(&r).get("unknown_reason"));
            inn.unknown_sigs.insert(sig.clone(), why.clone());
            inn.inc("outcome_unknown");
            self.log.write(json!({"ev": "outcome_unknown", "aid": inn.aid, "tool": name, "sig": sig, "why": why}));
        }
        r
    }

    /// The structured effect of a risky action as the gate sees it now (disk checks, dialog text, terminal line).
    pub fn effect_key(&self, inn: &Inner, ctx: &ActionCtx) -> crate::effect::EffectKey {
        let dialog = self.dialog_text(inn, ctx);
        let targets = crate::features::risk_targets(ctx, &inn.state);
        let line = ctx.target.pid.filter(|p| crate::sidefx::is_terminal(*p)).map(|p| inn.state.last_typed.get(&p).cloned().unwrap_or_default());
        authz::effect_of(ctx, &targets, &dialog, line.as_deref())
    }

    /// gated() approved a risky effect (operator task, reviewer, person): permit ONE dispatch of these final
    /// arguments, but only when their effect is exactly the approved one (a heal / re-bind / retry that lands on another
    /// target, window or payload gets nothing and is refused at the dispatch check).
    pub fn grant(&self, inn: &mut Inner, name: &str, args: &Value) {
        let Some((ak, src)) = inn.approval.clone() else { return };
        let ctx = build_ctx(name, args, &inn.state);
        if ctx.risk.is_none() && ctx.handoff.is_none() {
            return;
        }
        let k = self.effect_key(inn, &ctx);
        if k == ak {
            inn.permits.push(Permit { sig: ctx.signature, key: k, expires: now() + PERMIT_TTL_S });
        } else {
            inn.inc("permits_withheld");
            self.log.write(json!({"ev": "permit_withheld", "aid": inn.aid, "tool": name, "approved": ak.to_json(), "final": k.to_json(), "source": src}));
        }
    }

    /// None = may be sent. Some(refusal) = a risky / hand-off target without a matching one-shot permit.
    fn dispatch_check(&self, inn: &mut Inner, name: &str, args: &Value) -> Option<Value> {
        if !inn.cfg.risk_confirm {
            return None; // the operator switched risk confirmation off for this session (a launcher setting)
        }
        let ctx = build_ctx(name, args, &inn.state);
        if ctx.risk.is_none() && ctx.handoff.is_none() {
            return None;
        }
        let k = self.effect_key(inn, &ctx);
        let t = now();
        inn.permits.retain(|p| p.expires > t);
        if let Some(i) = inn.permits.iter().position(|p| p.sig == ctx.signature && p.key == k) {
            inn.permits.remove(i); // one shot
            inn.inc("permits_used");
            return None;
        }
        let why = format!("{} has no approval for this exact effect ({})", ctx.risk.clone().or(ctx.handoff.clone()).unwrap_or_default(), k.short());
        inn.inc("dispatch_refused");
        self.log.write(json!({"ev": "dispatch_refused", "aid": inn.aid, "tool": name, "sig": ctx.signature, "effect": k.to_json(), "why": why}));
        Some(json!({"isError": true,
            "content": [{"type": "text", "text": format!("[gate] Not executed: the final dispatch check stopped `{name}` on \"{}\": {why}. Nothing was sent.", trunc(ctx.target.the_label(), 60))}],
            "structuredContent": {"gate": {"decision": "deny", "tier": "dispatch", "executed": false, "reason": why, "effect": k.to_json()}}}))
    }

    /// `no_resend`: an action tool. When the driver dies or times out during it, the gate cannot tell whether the input
    /// reached the app, so it restarts the driver but never sends the action again; the result says "unknown".
    fn tool_raw(&self, name: &str, args: &Value, timeout: f64, no_resend: bool) -> Value {
        if !self.child().alive() {
            self.respawn();
        }
        let r = self.child().tool(name, args, timeout);
        let died = is_error(&r) && (!self.child().alive() || text_of(&r) == "driver exited");
        if no_resend {
            let why = if died {
                Some("the driver exited during the call")
            } else if is_error(&r) && text_of(&r).starts_with("timeout calling") {
                Some("the driver did not answer in time")
            } else if !r.is_object() || (r.get("content").is_none() && r.get("structuredContent").is_none() && r.get("isError").is_none()) {
                Some("the driver's answer could not be parsed")
            } else {
                None
            };
            if let Some(why) = why {
                if died || !self.child().alive() {
                    self.respawn();
                }
                return unknown_result(name, why);
            }
            return r;
        }
        if died {
            self.respawn();
            let r2 = self.child().tool(name, args, timeout);
            if is_error(&r2) && text_of(&r2) == "driver exited" {
                return text_result("driver exited twice; giving up on this call", true);
            }
            return r2;
        }
        r
    }

    pub fn check_updates(self: &Arc<Self>) {
        let me = self.clone();
        std::thread::spawn(move || {
            for _ in 0..50 {
                if me.shared.lock().unwrap().driver_version.is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            let dv = me.shared.lock().unwrap().driver_version.clone();
            let n = updates::note(&updates::status(true, dv));
            if n.is_empty() {
                return;
            }
            me.shared.lock().unwrap().update_note = n.clone();
            log_err(&n);
            std::thread::sleep(Duration::from_secs(2));
            let told = std::mem::replace(&mut me.shared.lock().unwrap().update_told, true);
            if !told {
                me.out.send(&json!({"jsonrpc": "2.0", "method": "notifications/message",
                                    "params": {"level": "warning", "logger": "gui-gate", "data": n}}));
            }
        });
    }

    pub fn serve(self: &Arc<Self>) {
        crate::proc::install_signal_handlers();
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(msg) = serde_json::from_str::<Value>(line) else { continue };
            let method = msg.get("method").and_then(|m| m.as_str()).map(String::from);
            if method.is_none() {
                if let Some(mid) = msg.get("id").and_then(|i| i.as_str()) {
                    let tx = self.client_pending.lock().unwrap().remove(mid);
                    if let Some(tx) = tx {
                        let _ = tx.send(msg);
                        continue;
                    }
                }
            }
            let has_id = msg.get("id").is_some();
            let mut msg = msg;
            let modern = if has_id && method.is_some() {
                match classify(&msg) {
                    Ok(m) => m,
                    Err(e) => {
                        self.out.send(&e);
                        continue;
                    }
                }
            } else {
                false
            };
            if modern {
                // 2026-07-28: client capabilities come with every request instead of initialize
                let caps = msg["params"]["_meta"].get(CC_KEY).cloned().unwrap_or(json!({}));
                self.client_elicit.store(caps.get("elicitation").is_some(), Ordering::SeqCst);
            }
            if method.as_deref() == Some("initialize") {
                let p = msg.get("params").cloned().unwrap_or(json!({}));
                if !modern {
                    self.client_elicit.store(p.get("capabilities").and_then(|c| c.get("elicitation")).is_some(), Ordering::SeqCst);
                }
                self.log.write(json!({"ev": "client", "name": p.get("clientInfo").and_then(|c| c.get("name")),
                                      "reviewer": self.reviewer.backend()}));
            }
            if method.as_deref() == Some("tools/call") && has_id {
                let me = self.clone();
                std::thread::spawn(move || me.handle_call(msg));
                continue;
            }
            if has_id && matches!(method.as_deref(), Some("server/discover") | Some("initialize")) {
                // answered by the gate from its own handshake with the driver (no second initialize to the driver)
                if let Some(r) = self.local_answer(method.as_deref().unwrap(), modern) {
                    self.out.send(&json!({"jsonrpc": "2.0", "id": msg["id"], "result": r}));
                    continue;
                }
            }
            if has_id && (modern || matches!(method.as_deref(), Some("tools/list") | Some("initialize"))) {
                self.shared.lock().unwrap().pending_methods.insert(msg["id"].to_string(), (method.clone().unwrap(), modern));
            }
            if modern {
                strip_modern_meta(&mut msg);
            }
            if !self.child().alive() && method.as_deref() != Some("initialize") {
                self.respawn();
            }
            if self.child().forward(&msg).is_err() {
                self.respawn();
                let _ = self.child().forward(&msg);
            }
        }
        self.shutdown();
    }

    /// initialize / server/discover answered by the gate from the driver's cached handshake result.
    fn local_answer(&self, method: &str, modern: bool) -> Option<Value> {
        let mut sh = self.shared.lock().unwrap();
        let mut init = sh.driver_init.clone()?;
        let mut r = if method == "server/discover" {
            let ins = gate_instructions(&init, &mut sh);
            json!({"supportedVersions": MODERN_VERSIONS,
                   "capabilities": init.get("capabilities").cloned().filter(|c| c.is_object()).unwrap_or(json!({"tools": {}})),
                   "instructions": ins})
        } else {
            rewrite_init(&mut init, &mut sh);
            init
        };
        if modern {
            modern_envelope(&mut r, method, &sh);
        }
        Some(r)
    }

    /// Send a tools/call result; modern-era requests get the 2026-07-28 envelope.
    fn reply(&self, msg: &Value, mut result: Value) {
        if is_modern(msg) {
            let sh = self.shared.lock().unwrap();
            modern_envelope(&mut result, "tools/call", &sh);
        }
        self.out.send(&json!({"jsonrpc": "2.0", "id": msg["id"], "result": result}));
    }

    /// The client closed stdin: leave nothing behind - background jobs and still-running commands are killed (whole
    /// process groups), the throwaway LibreOffice profile run_command suggested is removed, the driver is closed.
    pub fn shutdown(&self) {
        let jobs = self.jobs.stop_all();
        crate::proc::kill_all();
        let lo = crate::command::lo_profile_dir();
        if lo.exists() {
            let _ = std::fs::remove_dir_all(&lo);
        }
        self.log.write(json!({"ev": "shutdown", "jobs_killed": jobs}));
        self.child().close();
        if let Some(d) = self.child_cmd.first() {
            crate::proc::stop_mac_daemon_if_ours(d);
        }
    }

    fn handle_call(self: &Arc<Self>, msg: Value) {
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        let name0 = s(&params, "name").to_string();
        let args0 = params.get("arguments").cloned().filter(|a| a.is_object()).unwrap_or(json!({}));
        // a person is asked through MRTR when this request is 2026-07-28 and its client declared elicitation
        let mut cc = crate::human::CallCtx::default();
        cc.mrtr = is_modern(&msg) && params.get("_meta").and_then(|m| m.get(CC_KEY)).and_then(|c| c.get("elicitation")).is_some();
        cc.binding = crate::human::binding_of(&name0, &args0);
        if let Some(st) = params.get("requestState") {
            let st = st.as_str().unwrap_or("");
            match self.human.pending.resolve(st, &cc.binding, params.get("inputResponses")) {
                crate::human::Resolved::Answer(k, a, n, eff, o) => {
                    self.log.write(json!({"ev": "human_answer", "kind": k, "answer": trunc(&a, 200), "note": trunc(&n, 200), "outcome": o}));
                    cc.answer = Some((k, a, n, eff, o));
                }
                crate::human::Resolved::Late(k, eff) => {
                    self.log.write(json!({"ev": "human_answer", "kind": k, "answer": "timeout", "late": true, "outcome": "timeout"}));
                    cc.answer = Some((k, "timeout".into(), String::new(), eff, "timeout".into()));
                }
                crate::human::Resolved::Invalid(why) => {
                    self.log.write(json!({"ev": "human_answer_rejected", "why": why}));
                    self.out.send(&rpc_error(msg["id"].clone(), -32602, &format!("Invalid requestState: {why}"), None));
                    return;
                }
            }
        }
        crate::human::set_call(cc);
        let msg_for_reply = msg.clone();
        let me0 = self.clone();
        let take_input = move || -> bool {
            let c = crate::human::take_call();
            if let Some(req) = c.request {
                me0.reply(&msg_for_reply, req);
                return true;
            }
            false
        };
        self.handle_call_inner(msg, params, name0, args0, take_input);
    }

    fn handle_call_inner(self: &Arc<Self>, msg: Value, params: Value, name0: String, args0: Value, take_input: impl FnOnce() -> bool) {
        let _ = &params;
        // 0.9.7 phantom tools: a name the agent invented (triple_click, run_bash, type ...) is mapped to a real tool
        let (name, args, alias_note) = match self.phantom(&name0, &args0) {
            Ok(x) => x,
            Err(res) => {
                self.reply(&msg, res);
                return;
            }
        };
        // 0.9.7 feasibility first: no GUI action before the agent stated its feasibility verdict (once)
        if crate::v097::on("FEASIBILITY_FIRST") && plan_enabled() && (is_gated(&name) || name == "run_actions") && name != "run_command" {
            let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if inn.plan_verdict.is_empty() && !inn.verdict_nagged {
                inn.verdict_nagged = true;
                inn.inc("verdict_required");
                self.log.write(json!({"ev": "verdict_required", "tool": name}));
                drop(inn);
                let pre = if name0 == "gate_task" { None } else { self.briefing() };
                let mut r = text_result("[gate] Nothing was executed. First decide whether this task can be done on this computer at all: call set_plan with feasible:true and your milestones (end states), or feasible:false with the reason and reply FAIL. Use the briefing / inspect kind=env for facts. Then send this action again.", false);
                if let Some(b) = pre { push_text(&mut r, b); }
                self.reply(&msg, r);
                return;
            }
        }
        let brief_first = if name != "gate_task" { self.briefing() } else { None };
        let me = self.clone();
        let nm = name.clone();
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            match nm.as_str() {
                "inspect" if crate::v097::on("INSPECT") => me.inspect(&args),
                "run_command" if crate::v097::on("JOBS") && matches!(s(&args, "mode"), "poll" | "stop") => me.job_poll(&args),
                "__triple_click" => me.triple_click(&args),
                "gate_report" => me.report(),
                "gate_config" if !config_allowed() => text_result("[gate] gate_config is not available in this session (the gate's settings are fixed by whoever started it).", true),
                "gate_config" => me.config(&args),
                "request_human" => me.request_human(&args),
                "gate_task" => me.set_task(&args),
                "check_done" => me.check_done(&args),
                "set_plan" => me.set_plan(&args),
                "run_actions" => {
                    let mut inn = me.inner.lock().unwrap_or_else(|e| e.into_inner());
                    batch::run_actions(&me, &mut inn, &args)
                }
                n if is_gated(n) => {
                    let mut inn = me.inner.lock().unwrap_or_else(|e| e.into_inner());
                    me.gated(&mut inn, n, args)
                }
                n => me.passthrough(n, args),
            }
        }));
        let result = match res {
            Ok(r) => r,
            Err(e) => {
                let what = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
                log_err(&format!("error handling {name}: {what}"));
                text_result(format!("[gate] internal error: {what}"), true)
            }
        };
        if take_input() {
            return; // a person is being asked (MRTR): the client retries this call with the answer
        }
        let mut result = result;
        set_status(&name, &mut result);
        if let Some(n) = alias_note {
            push_text(&mut result, format!("[gate] {n}."));
        }
        let brief = brief_first.or_else(|| if name == "gate_task" && !is_error(&result) { self.briefing() } else { None });
        if let Some(b) = brief {
            push_text(&mut result, b);
        }
        {
            let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if inn.cfg.policy == "gate" {
                if name == "get_window_state" && inn.acted {
                    // reads before the first action are orientation, not a stall
                    inn.read_streak += 1;
                    if let Some(h) = streak_hint(inn.read_streak).filter(|_| stall_hints_enabled()) {
                        inn.inc("streak_hints");
                        self.log.write(json!({"ev": "streak_hint", "n": inn.read_streak}));
                        push_text(&mut result, h);
                    }
                } else if is_gated(&name) || name == "run_actions" {
                    inn.read_streak = 0;
                    inn.acted = true;
                }
            }
        }
        if is_gated(&name) || name == "run_actions" {
            let inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(l) = plan_line(&inn.plan).filter(|_| plan_enabled()) {
                drop(inn);
                push_text(&mut result, l);
            }
        }
        if (is_gated(&name) || name == "run_actions") && crate::v097::on("DIGEST") {
            if let Some(d) = self.digest() {
                push_text(&mut result, d);
            }
        }
        if self.inner.lock().unwrap_or_else(|e| e.into_inner()).cfg.text_only {
            text_only_rewrite(&mut result);
        }
        // v0.9.4: screenshots go back to the agent as full-size JPEG (all policies, so bench conditions match)
        if let Some(mut ev) = crate::shot::reencode_result(&mut result, &self.shot) {
            {
                let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                inn.add("img_bytes_in", ev["bytes_in"].as_f64().unwrap_or(0.0));
                inn.add("img_bytes_out", ev["bytes_out"].as_f64().unwrap_or(0.0));
                inn.add("img_ms", ev["ms"].as_f64().unwrap_or(0.0));
                ev["aid"] = json!(inn.aid);
            }
            ev["tool"] = json!(name);
            self.log.write(ev);
        }
        self.reply(&msg, result);
    }

    // ------------------------------------------------------------------ pass-through
    fn passthrough(&self, name: &str, mut args: Value) -> Value {
        let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let t0 = Instant::now();
        let mut trimmed = false;
        let mut fresh = false;
        if name == "get_window_state" {
            fresh = truthy(args.get("fresh"));
            if let Some(o) = args.as_object_mut() {
                o.remove("fresh");
            }
        }
        if name == "get_window_state" && inn.cfg.policy == "gate" && !fresh {
            if let Some(dup) = self.duplicate_read(&mut inn, &args) {
                return dup;
            }
        }
        let pid = as_i64(args.get("pid"));
        let mut shot_refused = false;
        if name == "get_window_state" && inn.cfg.text_only {
            // the client cannot use images: never capture (saves the capture + re-encode on every read)
            shot_refused = truthy(args.get("include_screenshot"));
            args["include_screenshot"] = json!(false);
            inn.inc("screenshots_skipped");
        }
        if name == "get_window_state" && inn.cfg.policy == "gate" && args.get("include_screenshot").is_none() {
            if let Some(p) = pid {
                if inn.seen_pids.contains(&p) && !inn.want_shot.contains(&p) && !inn.pixel_pids.contains(&p) {
                    args["include_screenshot"] = json!(false);
                    trimmed = true;
                }
            }
        }
        let all_apps = name == "list_apps" && truthy(args.get("all"));
        if name == "list_apps" {
            if let Some(o) = args.as_object_mut() {
                o.remove("all");
            }
        }
        let mut res = self.tool(&mut inn, name, &args, 120.0);
        if name == "list_apps" && inn.cfg.policy == "gate" && !all_apps && !is_error(&res) {
            let hidden = filter_apps(&mut res, &|p| proc_is_kernel_thread(p));
            if hidden > 0 {
                inn.add("apps_hidden", hidden as f64);
                push_text(&mut res, format!("[gate] {hidden} kernel threads / system daemons hidden; call list_apps with all:true to see everything."));
            }
        }
        // m3 run: 31 reads of a pid without a window (a helper from list_apps: instabox-term, an app finder...);
        // each cost a turn plus a list_windows. Hand the window list back with the error.
        if name == "get_window_state" && is_error(&res) && text_of(&res).contains("No windows found for pid") && no_window_list_enabled() {
            let lw = self.tool(&mut inn, "list_windows", &json!({}), 20.0);
            if !is_error(&lw) {
                let rows = window_rows_for_agent(&text_of(&lw));
                if !rows.is_empty() {
                    inn.inc("no_window_lists");
                    push_text(&mut res, format!("[gate] pid {} owns no window (a helper or background process). Windows you can read now (pass their pid and window_id):\n{}",
                                                pid.map(|p| p.to_string()).unwrap_or_default(), rows));
                }
            }
        }
        if shot_refused {
            push_text(&mut res, "[gate] text-only session: no screenshot is taken (the model cannot see images); use the tree rows and their element_tokens.".into());
        }
        if name == "get_window_state" && text_of(&res).contains("PARTIAL TREE") && !inn.cfg.text_only {
            if let Some(p) = pid {
                inn.pixel_pids.insert(p);
            }
        }
        if name == "get_window_state" && !is_error(&res) && args.get("query").is_none() {
            let scv = sc(&res);
            if let Some(p) = as_i64(scv.get("pid")).or(pid) {
                let key = format!("{p}:{}", as_i64(scv.get("window_id")).or(as_i64(args.get("window_id"))).unwrap_or(0));
                let mut txt = text_of(&res);
                if let Some(md) = scv.get("tree_markdown").and_then(|v| v.as_str()) {
                    txt = format!("{md}\n{txt}");
                }
                if let Some(extra) = self.tree_extra(&mut inn, p, &key, &txt, false) {
                    if extra.starts_with("[gate] Visible cells") && rewrite_partial_advice(&mut res) {
                        inn.inc("partial_advice_rewritten");
                    }
                    push_text(&mut res, extra);
                }
            }
        }
        if trimmed {
            let scv = sc(&res);
            let md = scv.get("tree_markdown").and_then(|v| v.as_str()).map(String::from).unwrap_or_else(|| text_of(&res));
            if md.matches("- [").count() < 2 {
                args["include_screenshot"] = json!(true);
                res = self.tool(&mut inn, name, &args, 120.0);
                trimmed = false;
            } else {
                inn.inc("screenshots_skipped");
                push_text(&mut res, "[gate] screenshot omitted (the element tree is enough here); call again with include_screenshot:true if you need pixels.".into());
            }
        }
        if name == "get_window_state" && inn.cfg.tree_diff && !is_error(&res) {
            let scv = sc(&res);
            if let (Some(p), Some(md)) = (as_i64(scv.get("pid")).or(pid), scv.get("tree_markdown").and_then(|v| v.as_str())) {
                let k = format!("{p}:{}", as_i64(scv.get("window_id")).or(as_i64(args.get("window_id"))).unwrap_or(0));
                if args.get("query").is_none() && args.get("since").is_none() && !md.trim().is_empty() {
                    inn.seen_rows.insert(k, (pystr(scv.get("snapshot_id")), md.lines().map(String::from).collect()));
                }
            }
        }
        if name == "get_window_state" {
            if let Some(p) = pid {
                inn.seen_pids.insert(p);
                if args.get("include_screenshot").map(|v| truthy(Some(v))).unwrap_or(true) {
                    inn.want_shot.remove(&p);
                }
            }
        }
        if name == "get_window_state" && inn.cfg.policy == "gate" && inn.cfg.text_only && stall_hints_enabled() && !is_error(&res) {
            let scv = sc(&res);
            if let Some(p) = as_i64(scv.get("pid")).or(pid) {
                let key = format!("{p}:{}", as_i64(scv.get("window_id")).or(as_i64(args.get("window_id"))).unwrap_or(0));
                let txt = text_of(&res);
                let mut hinted = std::mem::take(&mut inn.hinted);
                let hs = read_hints(&txt, crate::sidefx::is_terminal(p), &key, &mut hinted);
                inn.hinted = hinted;
                for h in hs {
                    inn.inc("read_hints");
                    self.log.write(json!({"ev": "read_hint", "pid": p, "hint": if h == HINT_GRID { "grid" } else { "terminal" }}));
                    push_text(&mut res, h.to_string());
                }
            }
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        if OBS_TOOLS.contains(&name) {
            inn.inc("obs_calls");
            let shot = res.get("content").and_then(|c| c.as_array()).map(|a| a.iter().any(|c| s(c, "type") == "image")).unwrap_or(false);
            let scv = sc(&res);
            let mut px = 0i64;
            if shot {
                inn.inc("screenshots");
                px = as_i64(scv.get("screenshot_width")).unwrap_or(0) * as_i64(scv.get("screenshot_height")).unwrap_or(0);
                inn.add("screenshot_px", px as f64);
            }
            if name == "get_window_state" {
                inn.inc("tree_reads");
                if let Some(o) = inn.state.record_observation(&args, &res, None) {
                    if let Some(p) = inn.state.get(o).pid {
                        let sig = self.window_sig(&mut inn, p);
                        inn.state.get_mut(o).window_count = sig;
                    }
                }
            }
            let pidv = scv.get("pid").cloned().or_else(|| args.get("pid").cloned()).unwrap_or(Value::Null);
            self.log.write(json!({"ev": "obs", "tool": name, "ms": round_to(ms, 1), "screenshot": shot, "px": px,
                                  "snapshot": scv.get("snapshot_id"), "pid": pidv, "trimmed": trimmed}));
        }
        res
    }

    /// What the driver's tree leaves out, from AT-SPI: visible spreadsheet cells, a terminal's visible text.
    /// `diff`: the gate's own post-action look - only what changed since the window's last cells/text shown.
    /// `settle`: right after an action, re-read a terminal until its text holds still (a command may still print).
    pub fn tree_extra(&self, inn: &mut Inner, pid: i64, key: &str, txt: &str, diff: bool) -> Option<String> {
        self.tree_extra_s(inn, pid, key, txt, diff, false)
    }

    pub fn tree_extra_s(&self, inn: &mut Inner, pid: i64, key: &str, txt: &str, diff: bool, settle: bool) -> Option<String> {
        let budget = Duration::from_millis(1500);
        let t0 = Instant::now();
        if grid_not_listed(txt) && crate::atspi::cells_enabled() && is_spreadsheet_app(pid) {
            let r = crate::atspi::visible_grid(pid, &window_title_of(txt), budget);
            let ms = round_to(t0.elapsed().as_secs_f64() * 1000.0, 1);
            return match r {
                Ok(g) => {
                    let prev = inn.cells_seen.get(key).cloned();
                    let out = prev.as_ref().filter(|_| diff).and_then(|(rg, items)| crate::atspi::render_grid_diff(rg, items, &g, 3000))
                        .unwrap_or_else(|| crate::atspi::render_grid(&g, 6000));
                    inn.cells_seen.insert(key.to_string(), (g.range(), g.items()));
                    inn.inc("cells_reads");
                    self.log.write(json!({"ev": "tree_extra", "kind": "cells", "pid": pid, "ms": ms, "range": g.range(), "cells": g.cells.len(), "chars": out.len(), "diff": out.contains("changed since") || out.contains("unchanged since")}));
                    Some(out)
                }
                Err(e) => {
                    self.log.write(json!({"ev": "tree_extra", "kind": "cells", "pid": pid, "ms": ms, "error": e}));
                    None
                }
            };
        }
        if crate::atspi::term_text_enabled() && crate::sidefx::is_terminal(pid) {
            let title = window_title_of(txt);
            let mut r = crate::atspi::terminal_text(pid, &title, budget);
            let mut running = false;
            if settle {
                // wait (<= ~2.4 s) until the text holds still AND ends at a shell prompt (a command may still print)
                for _ in 0..8 {
                    let Ok(prev) = r.clone() else { break };
                    if at_prompt(&prev) {
                        std::thread::sleep(Duration::from_millis(150));
                        let again = crate::atspi::terminal_text(pid, &title, budget);
                        let same = again.as_ref().ok() == Some(&prev);
                        r = again;
                        if same {
                            break;
                        }
                        continue;
                    }
                    std::thread::sleep(Duration::from_millis(300));
                    r = crate::atspi::terminal_text(pid, &title, budget);
                }
                running = r.as_ref().map(|t| !at_prompt(t)).unwrap_or(false);
            }
            let ms = round_to(t0.elapsed().as_secs_f64() * 1000.0, 1);
            return match r {
                Ok(t) => {
                    let same = inn.term_seen.get(key) == Some(&t);
                    inn.term_seen.insert(key.to_string(), t.clone());
                    inn.inc("term_reads");
                    let mut out = if diff && same { "[gate] Terminal text unchanged since the last read.".to_string() } else { t };
                    if running {
                        out.push_str("\n[gate] No shell prompt yet: the command may still be running (or waiting for input); read the window again to see the rest.");
                    }
                    self.log.write(json!({"ev": "tree_extra", "kind": "terminal", "pid": pid, "ms": ms, "chars": out.len(), "same": same}));
                    Some(out)
                }
                Err(e) => {
                    self.log.write(json!({"ev": "tree_extra", "kind": "terminal", "pid": pid, "ms": ms, "error": e}));
                    None
                }
            };
        }
        None
    }

    pub fn window_sig(&self, inn: &mut Inner, pid: i64) -> Option<String> {
        let r = self.tool(inn, "list_windows", &json!({"pid": pid}), 10.0);
        let ws = sc(&r).get("windows").and_then(|w| w.as_array()).cloned()?;
        let f = |w: &Value, k: &str| match w.get(k) { None | Some(Value::Null) => "None".to_string(), Some(v) => pystr(Some(v)) };
        let mut parts: Vec<String> = ws.iter().filter(|w| w.get("is_on_screen").map(|v| truthy(Some(v))).unwrap_or(true))
            .map(|w| format!("{}@{},{},{}x{}", f(w, "window_id"), f(w, "x"), f(w, "y"), f(w, "width"), f(w, "height"))).collect();
        parts.sort();
        Some(parts.join(";"))
    }

    /// (pid, window_id, title) of every listed window (desktop and panels included).
    pub fn all_windows(&self, inn: &mut Inner) -> Vec<(i64, i64, String)> {
        let r = self.tool(inn, "list_windows", &json!({}), 10.0);
        sc(&r).get("windows").and_then(|w| w.as_array()).map(|ws| ws.iter().map(|w| {
            let t = pystr(w.get("title"));
            (as_i64(w.get("pid")).unwrap_or(0), as_i64(w.get("window_id")).unwrap_or(0), if t.is_empty() { pystr(w.get("name")) } else { t })
        }).collect()).unwrap_or_default()
    }

    /// run_command: execute after the gate allowed it; the result says what changed and warns when a changed file is
    /// open in an app (the app's own save would write its old copy back over the change).
    fn exec_command(&self, inn: &mut Inner, args: &Value) -> Value {
        let cmd = pystr(args.get("command"));
        let cwd = crate::features::command_cwd(args);
        if cmd.trim().is_empty() {
            return text_result("[run_command] empty command", true);
        }
        if !std::path::Path::new(&cwd).is_dir() {
            return text_result(format!("[run_command] cwd {cwd} is not a directory"), true);
        }
        if crate::v097::on("JOBS") && s(args, "mode") == "start" {
            return match self.jobs.start(&cmd, &cwd) {
                Ok(id) => {
                    inn.inc("jobs_started");
                    self.log.write(json!({"ev": "job", "aid": inn.aid, "job": id, "cwd": cwd}));
                    let mut r = text_result(format!("[run_command] started job {id} in the background (cwd {cwd}). Check it with run_command mode=poll job={id}; stop it with mode=stop."), false);
                    r["structuredContent"] = json!({"job": id, "running": true});
                    r
                }
                Err(e) => text_result(format!("[run_command] could not start the job: {e}"), true),
            };
        }
        let tmo = args.get("timeout_s").and_then(|v| v.as_f64()).unwrap_or(30.0);
        let r = crate::command::run(&cmd, &cwd, tmo);
        inn.inc("commands_run");
        let (mut t, err) = crate::command::result_text(&r, &cwd, 4000);
        if !r.changed.is_empty() {
            let wins = self.all_windows(inn);
            for (p, _) in r.changed.iter().take(40) {
                let b = crate::sidefx::basename(p);
                if let Some((_, _, title)) = wins.iter().find(|(_, _, ti)| crate::authz::is_open_document(Some(&b), &[ti.clone()])) {
                    t.push_str(&format!("\n[gate] {b} is open in the window \"{}\": that app still holds its old copy, and its own save (or the task's final save) would write it back over your change. Reload it there (LibreOffice: File > Reload, do not save first) or close it without saving, then check the content.", trunc(title, 80)));
                    inn.inc("command_open_doc_warn");
                    break;
                }
            }
        }
        self.log.write(json!({"ev": "command", "aid": inn.aid, "cwd": cwd, "exit": r.code, "timed_out": r.timed_out,
            "ms": round_to(r.ms, 1), "out_bytes": r.out_bytes, "changed": r.changed.len()}));
        let mut res = text_result(t, err);
        res["structuredContent"] = json!({"exit_code": r.code, "timed_out": r.timed_out, "changed_files": r.changed.iter().map(|x| x.0.clone()).collect::<Vec<_>>()});
        res
    }

    pub fn titles(&self, inn: &mut Inner, pid: i64) -> Vec<String> {
        let r = self.tool(inn, "list_windows", &json!({"pid": pid}), 10.0);
        sc(&r).get("windows").and_then(|w| w.as_array()).map(|ws| ws.iter().map(|w| {
            let t = pystr(w.get("title"));
            if t.is_empty() { pystr(w.get("name")) } else { t }
        }).collect()).unwrap_or_default()
    }

    // ------------------------------------------------------------------ probes
    fn probe_window_changed(&self, inn: &mut Inner, ctx: &ActionCtx) -> (Option<bool>, Option<bool>) {
        let Some(pid) = ctx.target.pid else { return (None, None) };
        let Some(o) = inn.state.latest_for(Some(pid), ctx.target.window_id) else { return (None, None) };
        let (Some(wc), Some(opid)) = (inn.state.get(o).window_count.clone(), inn.state.get(o).pid) else { return (None, None) };
        let Some(nowsig) = self.window_sig(inn, opid) else { return (None, None) };
        let added = !ids_of(Some(&nowsig)).difference(&ids_of(Some(&wc))).collect::<Vec<_>>().is_empty();
        (Some(nowsig != wc), Some(added))
    }

    fn check_available(&self, inn: &Inner, ctx: &ActionCtx) -> bool {
        let t = &ctx.target;
        let wid = t.window_id.or(t.obs.and_then(|o| inn.state.get(o).window_id));
        t.is_elem() && !t.the_label().is_empty() && t.pid.map(|p| p != 0).unwrap_or(false) && wid.map(|w| w != 0).unwrap_or(false)
            && !t.the_role().is_empty()
    }

    /// Cheap probe, no model involved: tree-only refresh + re-bind for token targets (sets probe_disabled when
    /// the element is disabled), verify_state existence for name targets.
    fn check(&self, inn: &mut Inner, ctx: &ActionCtx, args: Option<&mut Value>) -> Option<bool> {
        let t = &ctx.target;
        inn.inc("checks");
        inn.probe_disabled = false;
        if let (true, Some(el), Some(args)) = (t.kind == "token" || t.kind == "index", t.element.as_ref(), args) {
            if !el.label.is_empty() {
                let Some((newtok, newel)) = self.refresh(inn, ctx) else { return Some(false) };
                let (Some(newtok), Some(newel)) = (newtok, newel) else { return Some(false) };
                if t.kind == "token" {
                    args["element_token"] = json!(newtok);
                } else {
                    args["element_index"] = json!(newtok.split(':').nth(1).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0));
                }
                inn.probe_disabled = newel.enabled == Some(false);
                return Some(newel.enabled != Some(false));
            }
        }
        let mut sel = json!({"label_contains": trunc(t.the_label(), 200)});
        if !t.the_role().is_empty() {
            sel["role"] = json!(t.the_role());
        }
        let wid = t.window_id.or(t.obs.and_then(|o| inn.state.get(o).window_id));
        let r = self.tool(inn, "verify_state", &json!({"pid": t.pid, "window_id": wid, "expect": [{"element": {"selector": sel, "exists": true}}],
                                                        "timeout_ms": 0, "stable_samples": 1}), 10.0);
        match s(&sc(&r), "status") {
            "satisfied" => Some(true),
            "unsatisfied" | "unknown" => Some(false),
            _ if is_error(&r) => Some(false),
            _ => None,
        }
    }

    /// Tree-only re-read; returns (new token or None, element) or None on failure.
    fn refresh(&self, inn: &mut Inner, ctx: &ActionCtx) -> Option<(Option<String>, Option<Element>)> {
        let t = &ctx.target;
        let old = t.element.clone()?;
        let wid = t.window_id.or(t.obs.and_then(|o| inn.state.get(o).window_id));
        let mut a = json!({"pid": t.pid, "include_screenshot": false, "tree_format": "elements", "max_elements": 600});
        if let Some(w) = wid.filter(|w| *w != 0) {
            a["window_id"] = json!(w);
        }
        let r = self.tool(inn, "get_window_state", &a, 20.0);
        let o = inn.state.record_observation(&a, &r, None);
        inn.inc("tree_reads");
        let o = o?;
        inn.state.get_mut(o).gate_read = true;
        if let Some(p) = inn.state.get(o).pid {
            let sig = self.window_sig(inn, p);
            inn.state.get_mut(o).window_count = sig;
        }
        let ob = inn.state.get(o);
        let mut cands: Vec<Element> = ob.elements.values().filter(|e| e.label == old.label && e.role == old.role).cloned().collect();
        if cands.len() > 1 {
            cands = nearest(&old, cands);
        }
        if cands.len() != 1 {
            return Some((None, None));
        }
        Some((Some(format!("{}:{}", ob.snapshot_id, cands[0].index)), Some(cands[0].clone())))
    }

    /// The driver refused a token click because another control owns the element's point. One gate-side retry:
    ///  * the owner is the same control (same label, a duplicate accessible object): click that desktop point;
    ///  * an open menu covers it: Escape, re-read, re-bind the token, click again;
    ///  * the target sat in a menu that has closed since the read: re-open that menu, re-bind, click the item.
    /// Returns the result to hand back (successful retry, or the original error plus a note) and what was tried.
    fn heal_occlusion(&self, inn: &mut Inner, ctx: &ActionCtx, args: &Value, err_txt: &str) -> Option<(Value, String)> {
        let (x, y, orole, olab) = occlusion(err_txt)?;
        let t = &ctx.target;
        let pid = t.pid?;
        let el = t.element.clone()?;
        let wid = t.window_id.or(t.obs.and_then(|o| inn.state.get(o).window_id)).filter(|w| *w != 0);
        let mut base = json!({"pid": pid});
        if let Some(w) = wid {
            base["window_id"] = json!(w);
        }
        for k in ["count", "button", "modifiers", "modifier"] {
            if let Some(v) = args.get(k) {
                base[k] = v.clone();
            }
        }
        let same = !olab.trim().is_empty() && authz::norm(&olab) == authz::norm(&el.label);
        inn.inc("occlusion_heals_tried");
        if same {
            let mut a = base.clone();
            a["x"] = json!(x);
            a["y"] = json!(y);
            a["coordinate_frame"] = json!("desktop");
            self.grant(inn, "click", &a);
            let r = self.tool(inn, "click", &a, 30.0);
            if !is_error(&r) {
                inn.inc("occlusion_heals");
                let mut r = r;
                push_text(&mut r, format!("[gate] The token click was refused because a duplicate \"{olab}\" object owns that point; the gate clicked the same control at its desktop point ({x},{y}) instead."));
                return Some((r, "same_control_pixel".into()));
            }
            return None;
        }
        let parent = t.obs.and_then(|o| parent_menu(&inn.state.get(o).text, el.index));
        let mut opened = String::new();
        if orole == "menu" {
            let mut k = base.clone();
            k["key"] = json!("Escape");
            if is_error(&self.tool(inn, "press_key", &k, 10.0)) {
                return None;
            }
            std::thread::sleep(Duration::from_millis(200));
        } else if let (Some(pm), true) = (parent.clone(), el.role.contains("item") || el.role == "menu") {
            // the menu that held the item has closed: open it again from a fresh read
            let mut menu_ctx = ctx.clone();
            if let Some(e) = menu_ctx.target.element.as_mut() {
                e.role = "menu".into();
                e.label = pm.clone();
            }
            let Some((Some(mtok), _)) = self.refresh(inn, &menu_ctx) else {
                return Some((self.closed_menu_note(err_txt, &el.label, Some(&pm)), "closed_menu_note".into()));
            };
            let mut a = base.clone();
            a["element_token"] = json!(mtok);
            self.grant(inn, "click", &a);
            if is_error(&self.tool(inn, "click", &a, 30.0)) {
                return Some((self.closed_menu_note(err_txt, &el.label, Some(&pm)), "closed_menu_note".into()));
            }
            std::thread::sleep(Duration::from_millis(250));
            opened = pm;
        } else {
            return None;
        }
        match self.refresh(inn, ctx) {
            Some((Some(tok), _)) => {
                let mut a = base.clone();
                a["element_token"] = json!(tok.clone());
                self.grant(inn, "click", &a);
                let r = self.tool(inn, "click", &a, 30.0);
                if is_error(&r) {
                    return None;
                }
                inn.inc("occlusion_heals");
                let mut r = r;
                let how = if opened.is_empty() {
                    format!("the open menu \"{olab}\" covered it; the gate pressed Escape, re-read the window and clicked it again ({tok})")
                } else {
                    format!("its menu \"{opened}\" had closed; the gate re-opened that menu and clicked the item ({tok})")
                };
                push_text(&mut r, format!("[gate] Your click on \"{}\" was first refused: {how}. Older tokens for this window are stale.", el.label));
                Some((r, if opened.is_empty() { "escape_retry".into() } else { "reopen_menu".into() }))
            }
            _ if !opened.is_empty() => Some((text_result(format!("{err_txt}\n[gate] The menu that held \"{}\" had closed; the gate opened \"{opened}\" again but the item is not in it now: read the fresh tree below and pick the item from it.", el.label), true), "reopened_item_missing".into())),
            _ => None,
        }
    }

    /// What the reviewer gets besides the action: the last 3 executed actions and, in a terminal, the command
    /// line Enter would submit.
    fn review_context(&self, inn: &Inner, ctx: &ActionCtx) -> (Value, Option<String>) {
        let line = ctx.target.pid.filter(|p| crate::sidefx::is_terminal(*p))
            .map(|p| inn.state.last_typed.get(&p).cloned().unwrap_or_default());
        (json!(inn.recent.clone()), line)
    }

    fn closed_menu_note(&self, err_txt: &str, label: &str, parent: Option<&str>) -> Value {
        let p = parent.map(|p| format!("open the menu \"{p}\" again and then click \"{label}\" in the same run_actions call, or use its keyboard shortcut"))
            .unwrap_or_else(|| format!("open its menu again, then click \"{label}\" in the same run_actions call"));
        text_result(format!("{err_txt}\n[gate] The menu that held \"{label}\" is closed now, so its old position belongs to another control: {p}."), true)
    }

    fn heal(&self, inn: &mut Inner, ctx: &ActionCtx) -> Option<String> {
        let el = ctx.target.element.as_ref()?;
        if el.label.is_empty() || ctx.target.pid.is_none() {
            return None;
        }
        let (tok, _) = self.refresh(inn, ctx)?;
        let tok = tok?;
        inn.inc("heals");
        Some(tok)
    }

    // ------------------------------------------------------------------ gating
    /// One gated action. Permits and the approval live only inside this call: whatever is left is cleared after it.
    pub fn gated(&self, inn: &mut Inner, name: &str, args: Value) -> Value {
        inn.permits.clear();
        inn.approval = None;
        let r = self.gated_inner(inn, name, args);
        inn.permits.clear();
        inn.approval = None;
        r
    }

    fn gated_inner(&self, inn: &mut Inner, name: &str, mut args: Value) -> Value {
        let tg0 = Instant::now();
        let ack = false; // the acting model never approves its own step (gate_ack is accepted and ignored)
        inn.cfg.reviewer = self.reviewer.available();
        if let Some(o) = args.as_object_mut() {
            // no agent-facing approval exists: a confirmation code the agent might send is dropped like gate_ack
            o.remove("gate_ack");
            o.remove("gate_confirm");
        }
        let snapped = if crate::v097::on("SNAP") { self.snap_pixel(inn, name, &mut args) } else { None };
        let mut ctx = build_ctx(name, &args, &inn.state);
        inn.inc("actions");
        inn.aid += 1;
        inn.risk_info = Value::Null;

        let (mut wc, mut wadd) = (None, None);
        if ctx.target.kind == "pixel" || !inn.state.actions.is_empty() {
            (wc, wadd) = self.probe_window_changed(inn, &ctx);
        }
        let mut f = features(&ctx, &inn.state, wc, wadd, None, None);
        if name == "run_steps" {
            f["token_stale"] = 0.0; // the driver re-binds stale tokens inside its own batch
        }
        let mut check_res: Option<bool> = None;
        let shadow = if inn.cfg.collect && self.check_available(inn, &ctx) { self.check(inn, &ctx, None) } else { None };

        let mut trail: Vec<String> = vec![];
        // the identical action came back with an unknown outcome: never re-send it blind, read the window first
        let unknown_prev = inn.unknown_sigs.remove(&ctx.signature);
        // a risky or hand-off step always goes through the safety rules first, whatever the baseline policy says
        let safety_first = ctx.risk.is_some() || ctx.handoff.is_some();
        let mut dec = match if let Some(why) = &unknown_prev {
            trail.push("outcome_unknown".into());
            Some(Decision::new("look_light", 0.0, format!("the same action just had an unknown outcome ({why}); it may already have happened, so the gate read the window instead of sending it again: check whether it took effect, then send it again only if it is still needed"), "rule"))
        } else if safety_first { None } else { policy_decision(&ctx, &f, &inn.cfg) } {
            Some(d) => d,
            None => {
                let mut d = hard_rules(&ctx, &f, &inn.cfg, false, false);
                if d.as_ref().map(|d| d.is(&["ask_human", "risk_review"])).unwrap_or(false) && ctx.risk.is_some() {
                    d = self.risk_gate(inn, &ctx, d.unwrap(), &mut trail);
                    if d.is_none() {
                        // authorized: the remaining zero-cost rules (stale token, no observation...) still apply
                        d = hard_rules(&ctx, &f, &inn.cfg, true, ack);
                    }
                }
                match d {
                    Some(d) => d,
                    None => {
                        let ca = self.check_available(inn, &ctx);
                        choose(&ctx, &f, &inn.cfg, ca, false)
                    }
                }
            }
        };
        let soft = |d: &Decision| d.tier != "rule" && d.tier != "policy";
        if dec.is(&["look_light", "look_full", "review", "check"]) && soft(&dec) && fget(&f, "token_stale") < 0.5 && ctx.risk_level.is_none() {
            if let Some(fp) = fast_path(&ctx, &f) {
                dec = Decision::new("direct", dec.p_direct, fp, "rule").with_scores(&dec.scores);
            }
        }
        // inside a batch the model chained these steps on purpose, and the batch itself stops on any surprise;
        // the predictor's "look again first" for a keyboard or freshly resolved element step does not break it
        if inn.in_batch && dec.is(&["look_light", "review", "check", "wait"]) && soft(&dec) && ctx.risk_level.is_none()
            && ctx.target.kind != "pixel" && fget(&f, "token_stale") < 0.5 {
            dec = Decision::new("direct", dec.p_direct, "planned step inside a guarded batch", "rule").with_scores(&dec.scores);
        }
        // the loop breaker asked for another mode: a step in a different mode is that switch, so the failure streak
        // of the old mode does not hold it back (the predictor's "3 failed in a row -> look again" would undo it)
        if inn.cfg.policy == "gate" && inn.lb.level >= 1 && dec.is(&["look_light", "look_full", "review", "check", "wait"]) && soft(&dec)
            && ctx.risk_level.is_none() && fget(&f, "token_stale") < 0.5 && ctx.target.kind != "pixel"
            && crate::loopbreak::mode_of(name, &ctx.target.kind) != inn.lb.dominant_mode() {
            dec = Decision::new("direct", dec.p_direct, format!("mode switch the loop breaker asked for (was {})", dec.option), "rule").with_scores(&dec.scores);
            inn.inc("loop_switch_direct");
        }
        // cheap options are resolved here and re-decided (each at most once)
        let (mut waited, mut checked) = (false, false);
        for _ in 0..5 {
            trail.push(dec.option.clone());
            if dec.option == "check" && checked {
                dec = Decision::new("look_light", dec.p_direct, "target still not confirmed after a fresh tree probe", "grey");
                break;
            }
            if dec.option == "wait" && waited {
                let why = if fget(&f, "loading_cue") > 0.5 { "UI still busy after waiting" } else { "the window had not settled after waiting" };
                dec = Decision::new("look_light", dec.p_direct, why, "grey");
                break;
            }
            match dec.option.as_str() {
                "check" => {
                    checked = true;
                    trail.push("check".into());
                    check_res = self.check(inn, &ctx, Some(&mut args));
                    ctx = build_ctx(name, &args, &inn.state);
                    f = features(&ctx, &inn.state, wc, wadd, check_res, None);
                    if waited {
                        // the wait already let the popup/loading settle; do not count it a second time
                        // (v094: 5 menu clicks went wait -> check(pass) -> wait -> refused "UI still busy")
                        f["last_popup"] = 0.0;
                        f["loading_cue"] = 0.0;
                    }
                    if inn.probe_disabled {
                        f["loading_cue"] = 1.0;
                    }
                    dec = choose(&ctx, &f, &inn.cfg, false, true);
                }
                "wait" => {
                    waited = true;
                    trail.push("wait".into());
                    let mut slept = 0.0;
                    while slept < 1.5 {
                        std::thread::sleep(Duration::from_millis(250));
                        slept += 0.25;
                        if self.check_available(inn, &ctx) && self.check(inn, &ctx, None) == Some(true) {
                            break;
                        }
                    }
                    if let Some(last) = inn.state.actions.last_mut() {
                        last.unverifiable = false;
                    }
                    (wc, wadd) = self.probe_window_changed(inn, &ctx);
                    f = features(&ctx, &inn.state, wc, wadd, None, None);
                    f["loading_cue"] = 0.0;
                    f["last_popup"] = 0.0;
                    let ca = self.check_available(inn, &ctx) && !checked;
                    dec = choose(&ctx, &f, &inn.cfg, ca, checked);
                    if dec.option == "wait" {
                        dec = Decision::new("direct", dec.p_direct, "settled after wait", "model").with_scores(&dec.scores);
                    }
                }
                "heal" => {
                    match self.heal(inn, &ctx) {
                        Some(newtok) => {
                            args["element_token"] = json!(newtok);
                            let was_risky = ctx.risk.is_some() || ctx.handoff.is_some();
                            ctx = build_ctx(name, &args, &inn.state);
                            f = features(&ctx, &inn.state, wc, wadd, None, None);
                            if !was_risky && (ctx.risk.is_some() || ctx.handoff.is_some()) {
                                // the re-bound target is a risky one: never run it on the strength of an earlier check
                                dec = Decision::new("look_light", 0.0, format!("your stale token re-binds to {newtok}, a risky element; read it and send the step again so the gate can check it"), "rule");
                                trail.push("heal_risky".into());
                            } else {
                                dec = Decision::new("direct", p_direct(&f, &inn.cfg), format!("token re-bound to {newtok}"), "rule");
                                trail.push("healed".into());
                            }
                        }
                        None => dec = Decision::new("look_light", 0.0, "stale token and no unique element with the same role+label", "rule"),
                    }
                }
                _ => break,
            }
        }
        if dec.is(&["check", "wait", "heal"]) {
            dec = Decision::new("look_light", dec.p_direct, "could not settle the target cheaply", "grey");
        }
        // review: the gate's own independent reviewer looks at this step (the acting model is not asked)
        let mut judge = Value::Null;
        if dec.option == "review" {
            dec = self.review_step(inn, &ctx, dec, &mut trail, &mut judge);
        }
        // a command has no GUI target to look at or re-aim: only the risk path (and the loop breaker) can hold it
        if name == "run_command" && !dec.is(&["ask_human", "risk_review", "deny", "handoff", "direct"]) {
            dec = Decision::new("direct", dec.p_direct, format!("command (no GUI target; predictor said {})", dec.option), "rule").with_scores(&dec.scores);
        }
        // loop breaker level >= 2: this exact step already made no progress in the current loop
        if inn.cfg.policy == "gate" && crate::loopbreak::enabled() && !inn.in_batch && inn.lb.blocks(&ctx.signature)
            && !crate::decide::is_nav_key(&ctx) && !dec.is(&["ask_human", "risk_review", "deny", "handoff"]) {
            let mode = inn.lb.dominant_mode();
            dec = Decision::new("stop_loop", dec.p_direct, format!("loop breaker level {}: this exact step already made no progress in the current loop; {}",
                inn.lb.level, crate::loopbreak::switch_advice(&mode)), "rule").with_scores(&dec.scores);
            inn.inc("loop_blocks");
            trail.push("loop_block".into());
        }
        let mut opt = dec.option.clone();
        {
            let bo = inn.stats.entry("by_option").or_insert(json!({}));
            let c = bo.get(&opt).and_then(|v| v.as_i64()).unwrap_or(0);
            bo[opt.as_str()] = json!(c + 1);
        }
        let gate_ms = tg0.elapsed().as_secs_f64() * 1000.0;
        let t = ctx.target.clone();
        let fjson: Obj = f.iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
        let mut event = json!({"ev": "action", "aid": inn.aid, "tool": name, "policy": inn.cfg.policy, "model": inn.cfg.model_name,
            "lam": inn.cfg.lam, "target": {"kind": t.kind, "role": trunc(t.the_role(), 40), "label": trunc(t.the_label(), 80)},
            "risk": ctx.risk, "sig": ctx.signature, "features": fjson, "p_direct": round_to(dec.p_direct, 4),
            "decision": opt, "tier": dec.tier, "trail": trail, "reason": dec.reason, "check": check_res,
            "shadow_check": shadow, "judge": judge, "window_changed": wc, "window_added": wadd,
            "ack": ack, "approval": inn.approval.as_ref().map(|a| a.1.clone()), "risk_level": ctx.risk_level, "handoff": ctx.handoff,
            "risk_gate": inn.risk_info, "batch": inn.batch_ctx});

        // a person decides / acts, when the gate can reach one itself
        if (opt == "ask_human" || opt == "handoff" || opt == "ask_help") && self.human.resolve_channel() != "chat" {
            let what0 = if t.the_label().is_empty() { name.to_string() } else { t.the_label().to_string() };
            let q = if opt == "ask_help" {
                format!("The agent is stuck: its last steps did not work ({}).\nIt is trying to {} \"{what0}\".\nPlease do this step yourself in the app window, then press Allow. Press Deny to let the agent try another way.", dec.reason, name.replace('_', " "))
            } else if opt == "ask_human" {
                format!("The agent wants to {} \"{what0}\".\nWhy the gate stopped it: {}.\nAllow it?", name.replace('_', " "), dec.reason)
            } else {
                format!("The agent needs you: {}.\nPlease do this step yourself in the app window, then press Allow. Press Deny to stop the agent here.", dec.reason)
            };
            let kind = match opt.as_str() { "ask_human" => "approve", "handoff" => "takeover", _ => "ask_help" };
            // the answer is bound to the effect as it is now: a person's yes does not carry over to another target/window
            let ekey = if ctx.risk.is_some() || ctx.handoff.is_some() { Some(self.effect_key(inn, &ctx)) } else { None };
            let mut meta = json!({"tool": name, "target": trunc(&what0, 80), "reason": trunc(&dec.reason, 300), "decision": opt});
            if let Some(k) = &ekey {
                meta["effect"] = k.to_json();
            }
            let h = self.human.ask_kind(kind, &q, if opt == "ask_human" { "gui-gate: approve action" } else if opt == "ask_help" { "gui-gate: the agent is stuck" } else { "gui-gate: your turn" }, meta);
            let ans = s(&h, "answer").to_string();
            let outcome = s(&h, "outcome").to_string();
            if ans == "input_required" {
                // MRTR: the client asks its user and retries this same call; nothing is decided or executed yet
                inn.inc("human_parked");
                event["human"] = h.clone();
                event["executed"] = json!(false);
                event["human_pending"] = json!(kind);
                self.log.write(event);
                return text_result("[gate] waiting for a person", true);
            }
            if ans == "batch" {
                event["executed"] = json!(false);
                event["human"] = h.clone();
                self.log.write(event);
                let mut r = text_result(format!("[gate] Not executed: {} on \"{what0}\". This step needs a person; send it again ON ITS OWN (not inside run_actions) so the gate can ask them.", dec.reason), true);
                r["structuredContent"] = json!({"status": "input_required", "gate": {"decision": opt, "executed": false, "reason": dec.reason, "needs_person": kind}});
                return r;
            }
            if h.get("mrtr").is_none() || h["ms"].as_f64().unwrap_or(0.0) == 0.0 {
                inn.inc("human_asked");
            }
            event["human"] = h.clone();
            event["human_outcome"] = json!(outcome);
            self.log.write(json!({"ev": "human_outcome", "aid": inn.aid, "kind": kind, "outcome": outcome, "answer": trunc(&ans, 80),
                                  "channel": h.get("channel"), "effect": ekey.as_ref().map(|k| k.to_json())}));
            if ans == "allow" && opt == "ask_human" {
                inn.inc("human_allowed");
                if let Some(k) = ekey {
                    inn.approval = Some((k, "person".into()));
                }
                dec = Decision::new("direct", dec.p_direct, "approved by a person", "human").with_scores(&dec.scores);
                opt = "direct".into();
                event["decision"] = json!("direct");
                event["tier"] = json!("human");
                event["approval"] = json!("person");
            } else if ans == "allow" {
                inn.inc("human_allowed");
                event["executed"] = json!(false);
                event["gate_ms"] = json!(round_to(tg0.elapsed().as_secs_f64() * 1000.0, 1));
                self.log.write(event);
                let mut res = text_result(format!("[gate] A person did this step themselves ({}). Do not repeat it; continue from the window's current state.", dec.reason), false);
                res["structuredContent"] = json!({"human_outcome": outcome, "gate": {"decision": opt, "executed": false, "done_by_person": true}});
                if let Some(p) = t.pid {
                    self.auto_observe(inn, p, t.window_id, &mut res, true);
                }
                return res;
            } else if opt == "ask_help" {
                event["executed"] = json!(false);
                event["gate_ms"] = json!(round_to(tg0.elapsed().as_secs_f64() * 1000.0, 1));
                self.log.write(event);
                let note = crate::human::answer_note();
                let said = if note.is_empty() { String::new() } else { format!(" The person says: \"{note}\".") };
                let mut r = text_result(format!("[gate] Not executed: {}. No person took this step.{said} Do not send the same action again; try a clearly different way (a keyboard shortcut, another menu path, or another element).", dec.reason), true);
                r["structuredContent"] = json!({"human_outcome": outcome, "gate": {"decision": opt, "executed": false, "human": ans, "human_outcome": outcome}});
                return r;
            } else {
                let verdict = crate::human::outcome_words(&outcome, inn.cfg.human_timeout);
                event["executed"] = json!(false);
                event["gate_ms"] = json!(round_to(tg0.elapsed().as_secs_f64() * 1000.0, 1));
                self.log.write(event);
                let txt = self.deny_text(inn, &ctx, &dec, &format!("the person {verdict}"));
                let mut extra = Obj::new();
                extra.insert("human".into(), json!(ans));
                extra.insert("human_outcome".into(), json!(outcome));
                let mut r = refusal(&dec, txt, Some(extra));
                r["structuredContent"]["human_outcome"] = json!(outcome);
                return r;
            }
        }

        if opt == "direct" {
            let t0 = Instant::now();
            self.grant(inn, name, &args);
            let mut res = if name == "run_command" {
                // a command is not sent to the driver, but it passes the same final check
                match self.dispatch_check(inn, name, &args) { Some(r) => r, None => self.exec_command(inn, &args) }
            } else {
                self.tool(inn, name, &args, 120.0)
            };
            if sc(&res).get("gate").and_then(|g| g.get("executed")) == Some(&json!(false)) {
                // the final dispatch check refused it (no permit for this exact effect): nothing ran
                event["executed"] = json!(false);
                event["dispatch_refused"] = json!(true);
                event["gate_ms"] = json!(round_to(tg0.elapsed().as_secs_f64() * 1000.0, 1));
                self.log.write(event);
                return res;
            }
            // (also inside a batch: a healed step lets the rest of the batch run instead of stopping it)
            // m3 run: 66 token clicks refused because another control owns the element's point (stale frame, the
            // item's menu closed, a toolbar/panel on top); the gate's pixel/menu heals fixed 1 of 8 and never ran for
            // unnamed owners. The focus-free path acts on the element itself (AT-SPI action), no pointer involved.
            if name == "click" && is_error(&res) && inn.cfg.policy == "gate" && args.get("element_token").is_some()
                && occluded_any(&text_of(&res)) && occlusion_bg_enabled() {
                let mut a2 = args.clone();
                a2["delivery_mode"] = json!("background");
                inn.inc("occlusion_bg_tried");
                self.grant(inn, name, &a2);
                let r2 = self.tool(inn, name, &a2, 120.0);
                if !is_error(&r2) {
                    inn.inc("occlusion_bg");
                    event["occlusion_heal"] = json!("background");
                    res = r2;
                    push_text(&mut res, "[gate] A pointer click was refused (another control owns that screen point; nothing was sent); the gate activated the element itself through its accessibility action instead. Check the result in the tree.".into());
                } else {
                    event["occlusion_bg"] = json!(false);
                }
            }
            if name == "click" && is_error(&res) && inn.cfg.policy == "gate" {
                if let Some((r2, how)) = self.heal_occlusion(inn, &ctx, &args, &text_of(&res)) {
                    event["occlusion_heal"] = json!(how);
                    res = r2;
                }
            }
            // m3 run: bring_to_front + the same foreground retry healed 0 of 41. When the input focus is already inside
            // the target (focus_within_target=true: a popup list / menu of the active window), a token click can go the
            // focus-free way (AT-SPI action / synthetic event) instead.
            if is_error(&res) && inn.cfg.policy == "gate" && name == "click" && args.get("element_token").is_some()
                && focus_lost(&text_of(&res)) && text_of(&res).contains("focus_within_target=true") && focus_bg_enabled() {
                let mut a2 = args.clone();
                a2["delivery_mode"] = json!("background");
                inn.inc("focus_heals_bg_tried");
                self.grant(inn, name, &a2);
                let r2 = self.tool(inn, name, &a2, 120.0);
                if !is_error(&r2) {
                    inn.inc("focus_heals_bg");
                    event["focus_heal"] = json!("background");
                    res = r2;
                    push_text(&mut res, "[gate] The window would not take focus (nothing was sent); the gate sent the click focus-free instead. Check the result in the tree.".into());
                } else {
                    event["focus_heal_bg"] = json!(false);
                }
            }
            if is_error(&res) && inn.cfg.policy == "gate" && FOCUS_HEAL_TOOLS.contains(&name) && focus_lost(&text_of(&res)) {
                if let Some(p) = t.pid {
                    let mut b = json!({"pid": p});
                    if let Some(w) = t.window_id.filter(|w| *w != 0) {
                        b["window_id"] = json!(w);
                    }
                    let _ = self.tool(inn, "bring_to_front", &b, 10.0);
                    std::thread::sleep(Duration::from_millis(300));
                    inn.inc("focus_heals_tried");
                    self.grant(inn, name, &args);
                    let r2 = self.tool(inn, name, &args, 120.0);
                    if !is_error(&r2) {
                        inn.inc("focus_heals");
                        event["focus_heal"] = json!(true);
                        res = r2;
                        push_text(&mut res, "[gate] The window did not take focus at first (nothing was sent); the gate brought it to the front and sent the step once more.".into());
                    } else {
                        event["focus_heal"] = json!(false);
                    }
                }
            }
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            inn.add("driver_ms", ms);
            inn.inc("executed");
            let scv = sc(&res);
            let txt = text_of(&res);
            let err = is_error(&res);
            let stale = txt.to_lowercase().contains("stale") && err;
            let rec = ActionRecord {
                t: now(), tool: name.into(), key: ctx.signature.clone(),
                window_key: if t.pid.is_some() { wkey(t.pid, t.window_id) } else { None },
                is_error: err, unverifiable: s(&scv, "effect") == "unverifiable",
                popup: truthy(scv.get("popup")) || txt.to_lowercase().contains("popup"), stale_error: stale, aid: inn.aid,
                no_effect: s(&scv, "effect") == "none", retro: false,
            };
            if rec.is_error {
                inn.inc("exec_errors");
            }
            // (walking a list with the same arrow/Tab key is not a repeat of a step that did not work)
            let retro_aid = inn.state.last_action().filter(|prev| prev.key == rec.key && !prev.is_error && rec.t - prev.t < 60.0)
                .filter(|_| !crate::decide::is_nav_key(&ctx)).map(|p| p.aid);
            if let Some(aid) = retro_aid {
                // the agent sent the same action again: the previous one most likely did not do what it wanted
                self.log.write(json!({"ev": "retro_repeat", "aid": aid}));
                if let Some(p) = inn.state.actions.last_mut() {
                    p.retro = true;
                }
            }
            let popup = rec.popup;
            inn.state.record_action(rec);
            let keys_v = if truthy(args.get("keys")) { args.get("keys").cloned() } else { args.get("key").cloned() };
            let typed_v = if truthy(args.get("text")) { pystr(args.get("text")) } else { pystr(args.get("value")) };
            inn.recent.push(json!({"tool": name, "target": format!("{} \"{}\"", t.the_role(), trunc(t.the_label(), 60)),
                "typed_text": trunc(&typed_v, 120), "keys": keys_v, "modifiers": args.get("modifiers"),
                "failed": err, "result": trunc(&txt.lines().next().unwrap_or("").to_string(), 140)}));
            if inn.recent.len() > 3 {
                inn.recent.remove(0);
            }
            {
                let what = if name == "run_command" { format!("run_command `{}`", trunc(&pystr(args.get("command")), 60)) }
                    else if !typed_v.is_empty() { format!("{name} \"{}\"", trunc(&typed_v, 40)) }
                    else if t.the_label().is_empty() { format!("{name} {}", trunc(&keys_v.as_ref().map(|k| k.to_string()).unwrap_or_default().replace('"', ""), 30)) }
                    else { format!("{name} {} \"{}\"", t.the_role(), trunc(t.the_label(), 40)) };
                let line = format!("{what} -> {}", if err { format!("FAILED: {}", trunc(txt.lines().next().unwrap_or(""), 70)) } else { "ok".into() });
                inn.history.push(line);
                if inn.history.len() > 200 { inn.history.remove(0); }
            }
            inn.denials_consec = 0;
            let has_xy = args.get("x").is_some() && args.get("y").is_some();
            if let Some(p) = t.pid {
                let age = if has_xy { 0 } else { inn.pix_age.get(&p).copied().unwrap_or(99) + 1 };
                inn.pix_age.insert(p, age);
                if has_xy {
                    inn.pixel_pids.insert(p);
                }
                if err && no_frame().is_match(&txt) && inn.cfg.text_only {
                    // a text-only client has no frame to aim in: point it at element tokens instead
                    push_text(&mut res, "[gate] x/y aiming needs a screenshot, which this text-only session does not get. Aim with an element_token from the tree below instead (or keyboard navigation).".into());
                    self.auto_observe(inn, p, t.window_id, &mut res, true);
                    inn.inc("frame_recoveries");
                } else if err && no_frame().is_match(&txt) {
                    // the driver lost its pixel frame: hand back a fresh screenshot + tree in this same result
                    inn.pixel_pids.insert(p);
                    push_text(&mut res, "[gate] Here is a fresh screenshot of the window; re-aim your x/y on it and retry.".into());
                    self.auto_observe(inn, p, t.window_id, &mut res, true);
                    inn.inc("frame_recoveries");
                }
                if !err {
                    if name == "type_text" || name == "set_value" {
                        let add = if truthy(args.get("text")) { pystr(args.get("text")) } else { pystr(args.get("value")) };
                        inn.state.last_typed.entry(p).or_default().push_str(&add);
                    } else if name == "press_key" || name == "hotkey" {
                        let mut keys: HashSet<String> = args.get("keys").and_then(|k| k.as_array())
                            .map(|a| a.iter().map(|k| pystr(Some(k)).to_lowercase()).collect()).unwrap_or_default();
                        keys.insert(pystr(args.get("key")).to_lowercase());
                        if ["return", "enter", "kp_enter"].iter().any(|k| keys.contains(*k)) {
                            inn.state.last_typed.insert(p, String::new());
                        }
                    }
                }
            }
            event["executed"] = json!(true);
            event["exec_ms"] = json!(round_to(ms, 1));
            event["is_error"] = json!(err);
            event["stale_error"] = json!(stale);
            event["effect"] = scv.get("effect").cloned().unwrap_or(Value::Null);
            event["popup"] = json!(popup);
            event["gate_ms"] = json!(round_to(gate_ms, 1));
            inn.add("gate_ms", gate_ms);
            self.log.write(event);
            if trail.iter().any(|x| x == "healed") {
                push_text(&mut res, format!("[gate] your token was stale; re-bound to {} from a fresh tree read. Older tokens for this window are now stale.", pystr(args.get("element_token"))));
            }
            // typing that commits (a newline / tab) into a terminal or a spreadsheet: hand back what it produced (the
            // command output / the cells that changed) instead of the agent spending its next turn on a re-read
            if name == "type_text" && !is_error(&res) && inn.cfg.policy == "gate" && inn.cfg.auto_observe && !inn.in_batch
                && commits_input(&pystr(args.get("text"))) && typed_look_enabled() {
                if let Some(p) = t.pid {
                    let term = crate::sidefx::is_terminal(p);
                    if term || is_spreadsheet_app(p) {
                        let wid = t.window_id.filter(|w| *w != 0).or(inn.state.window_of_pid.get(&p).copied()).unwrap_or(0);
                        let key = format!("{p}:{wid}");
                        let ctx_txt = newest_obs_text(&inn.state, p);
                        // the extra decides by app; a spreadsheet needs the grid marker it looks for
                        let probe = if term { ctx_txt.clone() } else { format!("{ctx_txt}\n(cell grid)") };
                        std::thread::sleep(Duration::from_millis(250));
                        if let Some(e) = self.tree_extra_s(inn, p, &key, &probe, !term, true) {
                            inn.inc("typed_looks");
                            push_text(&mut res, format!("[gate] After your typing:\n{e}"));
                        }
                    }
                }
            }
            // also after an error: the agent gets the fresh tree (and fresh tokens) in the same turn instead of spending one on a re-read
            if AUTO_OBS_TOOLS.contains(&name) && inn.cfg.policy == "gate" && inn.cfg.auto_observe && !inn.in_batch {
                if let Some(p) = t.pid {
                    self.auto_observe(inn, p, t.window_id, &mut res, false);
                }
            }
            if inn.cfg.policy == "gate" && crate::loopbreak::enabled() && !inn.in_batch && !crate::decide::is_nav_key(&ctx) {
                let failed = is_error(&res) || s(&scv, "effect") == "none";
                let mode = crate::loopbreak::mode_of(name, &t.kind);
                let rose = if crate::v097::on("TWO_STRIKES") {
                    let mech = format!("{mode}|{}", mech_target(&ctx));
                    let fp = t.pid.and_then(|p| state_fp(&inn.state, p));
                    inn.lb.push_ext(&ctx.signature, mode, &mech, failed, s(&scv, "effect") == "confirmed", fp)
                } else {
                    inn.lb.push(&ctx.signature, mode, failed, s(&scv, "effect") == "confirmed")
                };
                if let Some(l) = rose {
                    let n = if crate::v097::on("TWO_STRIKES") { crate::loopbreak::note_ext(l, &inn.lb) } else { crate::loopbreak::note(l, &inn.lb) };
                    inn.inc("loop_notes");
                    self.log.write(json!({"ev": "loop_breaker", "aid": inn.aid, "level": l, "mode": inn.lb.dominant_mode(), "reason": inn.lb.last_reason}));
                    push_text(&mut res, n);
                    if let Some(j) = self.loop_judge(inn, l) {
                        push_text(&mut res, j);
                    }
                }
            }
            if let Some(n) = snapped {
                push_text(&mut res, n);
            }
            self.micro_verify(inn, name, &args, &mut res);
            return res;
        }

        // an intervention: nothing executed
        inn.add("gate_ms", gate_ms);
        let mut loc = vec![];
        if let Some(p) = t.pid {
            loc.push(format!("pid={p}"));
        }
        if let Some(w) = t.window_id.filter(|w| *w != 0) {
            loc.push(format!("window_id={w}"));
        }
        let loc = loc.join(", ");
        let what = if t.element.is_some() || !t.label.is_empty() { format!("\"{}\"", t.the_label()) } else { "this target".into() };
        let mut extra = Obj::new();
        let alt = authz::alternative(ctx.risk.as_deref().unwrap_or(""));
        let text = match opt.as_str() {
            "look_light" => {
                inn.inc("model_turns_requested");
                if t.pid.is_some() && inn.cfg.auto_observe && inn.cfg.policy == "gate" {
                    format!("[gate] Not executed: {}. The window's fresh tree is attached: pick the target from it and send the action again (no need to read the window yourself).", dec.reason)
                } else {
                    format!("[gate] Not executed: {}. Re-read the window without a screenshot (get_window_state {loc}, include_screenshot:false, since:\"latest\") and retry with the fresh token.", dec.reason)
                }
            }
            "look_full" if inn.cfg.text_only => {
                inn.inc("model_turns_requested");
                if t.pid.is_some() && inn.cfg.auto_observe && inn.cfg.policy == "gate" {
                    format!("[gate] Not executed: {}. The window's fresh tree is attached: check the target in it and send the action again (no need to read the window yourself).", dec.reason)
                } else {
                    format!("[gate] Not executed: {}. Re-read the window (get_window_state {loc}) and check the target before retrying.", dec.reason)
                }
            }
            "look_full" => {
                inn.inc("model_turns_requested");
                if let Some(p) = t.pid {
                    inn.want_shot.insert(p);
                }
                if t.pid.is_some() && inn.cfg.auto_observe && inn.cfg.policy == "gate" {
                    format!("[gate] Not executed: {}. A fresh screenshot of the window is attached: re-aim from it and send the action again (no need to read the window yourself).", dec.reason)
                } else {
                    format!("[gate] Not executed: {}. Re-read the window WITH a screenshot (get_window_state {loc}, include_screenshot:true) and check the target before retrying.", dec.reason)
                }
            }
            "risk_review" => {
                // only reached when neither the user's words, the reviewer nor a person could settle it
                self.deny_text(inn, &ctx, &dec, "refused")
            }
            "handoff" => {
                inn.inc("model_turns_requested");
                extra.insert("human_outcome".into(), json!("channel_unavailable"));
                extra.insert("needs_person".into(), json!("takeover"));
                format!("[gate] Not executed: {}. A person must do this step, not you. Ask the user to do it themselves, wait until they say it is done, then re-read the window and continue.", dec.reason)
            }
            "ask_human" => {
                // a person must approve it and the gate cannot reach one (no elicitation client, no desktop dialog):
                // nothing the agent sends can approve it, so it is a refusal with the safe way on
                inn.inc("model_turns_requested");
                extra.insert("human_outcome".into(), json!("channel_unavailable"));
                self.log.write(json!({"ev": "human_outcome", "aid": inn.aid, "kind": "approve", "outcome": "channel_unavailable"}));
                format!("[gate] Not executed: {} on {what}. Only a person can approve this, and the gate has no way to reach one in this session; nothing you send can approve it. Do not retry it or reach the same result another way. Instead: {alt}. Say in your final answer what was left undone and why.", dec.reason)
            }
            "stop_loop" => {
                inn.inc("model_turns_requested");
                format!("[gate] Not executed: {}. Sending the same action again will not help. Try a clearly different way: a keyboard shortcut, another menu path, or another element.", dec.reason)
            }
            "ask_help" => {
                inn.inc("model_turns_requested");
                format!("[gate] Not executed: {}. This keeps failing. Ask your user for help (say what you are trying to do and what happens instead), or try a clearly different way: a keyboard shortcut, another menu path, or another element.", dec.reason)
            }
            "deny" => self.deny_text(inn, &ctx, &dec, "refused"),
            _ => format!("[gate] Not executed: {}.", dec.reason),
        };
        // a refused risky Enter in a terminal leaves the typed command on the input line: the next command the
        // agent types would be appended to it and refused again (og04 loop). Clear the line and say so.
        // (deny / risk_review go through deny_text, which clears it too)
        let mut text = text;
        if ctx.risk.is_some() && opt == "ask_human" {
            if let Some(p) = t.pid {
                if let Some(note) = self.clear_terminal_line(inn, p, t.window_id) {
                    text.push_str(&note);
                }
            }
        }
        event["executed"] = json!(false);
        event["gate_ms"] = json!(round_to(gate_ms, 1));
        self.log.write(event);
        let mut res = refusal(&dec, text, Some(extra));
        if opt == "handoff" {
            res["structuredContent"]["status"] = json!("input_required"); // the user must do this step themselves
        }
        if let Some(o) = res["structuredContent"]["gate"].get("human_outcome").cloned() {
            res["structuredContent"]["human_outcome"] = o;
        }
        // a step the gate sent back (look again / doubted / loop) is no progress either: the agent re-sending a refused
        // step over and over is the same loop as re-sending a failing one (risk refusals have their own escalation)
        if inn.cfg.policy == "gate" && crate::loopbreak::enabled() && !inn.in_batch && !crate::decide::is_nav_key(&ctx)
            && dec.is(&["look_light", "look_full", "stop_loop", "ask_help", "review"]) {
            let mode = crate::loopbreak::mode_of(name, &t.kind);
            let rose = if crate::v097::on("TWO_STRIKES") {
                let mech = format!("{mode}|{}", mech_target(&ctx));
                inn.lb.push_ext(&ctx.signature, mode, &mech, true, false, None)
            } else {
                inn.lb.push(&ctx.signature, mode, true, false)
            };
            if let Some(l) = rose {
                let n = if crate::v097::on("TWO_STRIKES") { crate::loopbreak::note_ext(l, &inn.lb) } else { crate::loopbreak::note(l, &inn.lb) };
                inn.inc("loop_notes");
                self.log.write(json!({"ev": "loop_breaker", "aid": inn.aid, "level": l, "mode": inn.lb.dominant_mode(), "refused": true}));
                push_text(&mut res, n);
                if let Some(j) = self.loop_judge(inn, l) {
                    push_text(&mut res, j);
                }
            }
        }
        // re-read / re-aim: the gate reads the window itself and attaches it, saving the agent a whole turn
        if let (Some(p), true) = (t.pid, opt == "look_light" || opt == "look_full") {
            if inn.cfg.auto_observe && !inn.in_batch && inn.cfg.policy == "gate" {
                if opt == "look_full" {
                    inn.pixel_pids.insert(p);
                    inn.pix_age.insert(p, 0);
                }
                self.auto_observe(inn, p, t.window_id, &mut res, true);
                inn.want_shot.remove(&p);
                if let Some(items) = res.get_mut("content").and_then(|c| c.as_array_mut()) {
                    for it in items.iter_mut() {
                        if let Some(tx) = it.get("text").and_then(|x| x.as_str()) {
                            let nt = tx.replace("Window right after the action", "Window now (nothing was executed)");
                            it["text"] = json!(nt);
                        }
                    }
                }
            }
        }
        res
    }

    /// Erase what the agent typed on a terminal's input line (End, then kill to line start). Returns the note for
    /// the agent, or None when this is not a terminal or nothing was typed since the last Enter.
    fn clear_terminal_line(&self, inn: &mut Inner, pid: i64, window_id: Option<i64>) -> Option<String> {
        let typed = inn.state.last_typed.get(&pid).cloned().unwrap_or_default();
        if typed.trim().is_empty() || !crate::sidefx::is_terminal(pid) {
            return None;
        }
        for k in clear_line_keys() {
            let mut a = json!({"pid": pid, "key": k.0, "modifiers": k.1});
            if let Some(w) = window_id.filter(|w| *w != 0) {
                a["window_id"] = json!(w);
            }
            let r = self.tool(inn, "press_key", &a, 10.0);
            if is_error(&r) {
                return None;
            }
        }
        inn.state.last_typed.insert(pid, String::new());
        inn.inc("lines_cleared");
        self.log.write(json!({"ev": "line_cleared", "aid": inn.aid, "pid": pid, "text": trunc(&typed, 200)}));
        Some(" The gate has cleared the refused command from the terminal's input line (it was not run); type your next command on the empty line.".into())
    }

    // ------------------------------------------------------------------ observe after an action
    /// Read the window again WITHOUT a screenshot right after an action and hand the fresh tree back in the same
    /// result. Waits briefly while the UI says it is loading; reads a newly opened dialog instead.
    pub fn auto_observe(&self, inn: &mut Inner, pid: i64, window_id: Option<i64>, res: &mut Value, _force: bool) {
        let t0 = Instant::now();
        let prev_ids: HashSet<String> = inn.state.latest_for(Some(pid), window_id)
            .map(|o| ids_of(inn.state.get(o).window_count.as_deref())).unwrap_or_default();
        std::thread::sleep(Duration::from_millis(150));
        let (mut md, mut scv, mut a, mut note, mut sig) = (String::new(), json!({}), json!({}), String::new(), String::new());
        let mut r = json!({});
        let mut shot = false;
        for attempt in 0..4 {
            sig = self.window_sig(inn, pid).unwrap_or_default();
            let now_ids = ids_list(&sig);
            let new: Vec<String> = if prev_ids.is_empty() { vec![] } else { now_ids.into_iter().filter(|w| !prev_ids.contains(w)).collect() };
            let wid = if let Some(n) = new.last() { n.parse::<i64>().ok() } else { window_id.filter(|w| *w != 0).or(inn.state.window_of_pid.get(&pid).copied()) };
            note = if !new.is_empty() { format!("A new window/dialog opened (window_id={}); this is its tree.\n", wid.map(|w| w.to_string()).unwrap_or_default()) } else { String::new() };
            // screenshot only while the agent aims by pixels in this app (an x/y action within the last two)
            shot = !inn.cfg.text_only && inn.pixel_pids.contains(&pid) && inn.pix_age.get(&pid).copied().unwrap_or(99) <= 1;
            a = json!({"pid": pid, "include_screenshot": shot});
            if let Some(w) = wid.filter(|w| *w != 0) {
                a["window_id"] = json!(w);
            }
            r = self.tool(inn, "get_window_state", &a, 20.0);
            scv = sc(&r);
            md = pystr(scv.get("tree_markdown"));
            if is_error(&r) || !truthy(scv.get("snapshot_id")) {
                return;
            }
            if loading_rx().is_match(&md) && attempt < 3 {
                std::thread::sleep(Duration::from_millis(400));
                continue;
            }
            break;
        }
        let partial = text_of(&r).contains("PARTIAL TREE");
        if partial && !inn.cfg.text_only {
            inn.pixel_pids.insert(pid);
        }
        if partial && !shot && !inn.cfg.text_only && inn.pix_age.get(&pid).copied().unwrap_or(99) <= 3 {
            // a grid/canvas the tree cannot show: the agent will aim by pixels, so give it the picture
            shot = true;
            a["include_screenshot"] = json!(true);
            let r2 = self.tool(inn, "get_window_state", &a, 20.0);
            let sc2 = sc(&r2);
            if !is_error(&r2) && truthy(sc2.get("snapshot_id")) {
                r = r2;
                scv = sc2;
                let m2 = pystr(scv.get("tree_markdown"));
                if !m2.is_empty() {
                    md = m2;
                }
            }
        }
        let extra = {
            let k = format!("{pid}:{}", as_i64(scv.get("window_id")).or(as_i64(a.get("window_id"))).unwrap_or(0));
            let txt = format!("{md}\n{}", text_of(&r));
            self.tree_extra_s(inn, pid, &k, &txt, true, true)
        };
        if let Some(e) = &extra {
            push_text(&mut r, e.clone());
        }
        if let Some(o) = inn.state.record_observation(&a, &r, None) {
            inn.state.get_mut(o).window_count = Some(sig.clone());
        }
        inn.seen_pids.insert(pid);
        if md.trim().is_empty() {
            let body = text_of(&r);
            md = body.trim().lines().filter(|x| !x.starts_with("window_id=") && !x.starts_with("element_token for row"))
                .collect::<Vec<_>>().join("\n");
        }
        let all_lines: Vec<&str> = md.lines().collect();
        let snap0 = pystr(scv.get("snapshot_id"));
        let wkey_s = format!("{pid}:{}", as_i64(scv.get("window_id")).or(as_i64(a.get("window_id"))).unwrap_or(0));
        // changes only: rows the agent already saw for this window (same text, same [N]) are left out
        let mut delta_note = String::new();
        let mut lines: Vec<&str> = all_lines.clone();
        if inn.cfg.tree_diff && note.is_empty() {
            if let Some((prev_snap, seen)) = inn.seen_rows.get(&wkey_s) {
                if let Some(idx) = tree_delta(seen, &all_lines, 0.5) {
                    let shown: Vec<&str> = idx.iter().map(|&i| all_lines[i]).collect();
                    delta_note = format!(" Only rows that changed since snapshot {prev_snap} are shown ({} of {} rows, with their parents); every other row is unchanged and keeps its [N], so {snap0}:N tokens work for those too.",
                                         shown.len(), all_lines.len());
                    if shown.is_empty() {
                        delta_note = format!(" Nothing in the tree changed since snapshot {prev_snap} ({} rows); its [N] are valid as {snap0}:N.", all_lines.len());
                    }
                    lines = shown;
                    inn.inc("tree_diffs");
                }
            }
        }
        let mut more = String::new();
        if lines.len() > MAX_TREE_LINES {
            more = format!("\n... ({} more rows; call get_window_state for the full tree)", lines.len() - MAX_TREE_LINES);
            lines.truncate(MAX_TREE_LINES);
        }
        if inn.cfg.tree_diff {
            // what the agent has now seen of this window: shown rows + earlier rows that are still identical
            let prev: HashSet<String> = inn.seen_rows.get(&wkey_s).filter(|_| note.is_empty()).map(|x| x.1.clone()).unwrap_or_default();
            let shown: HashSet<&str> = lines.iter().copied().collect();
            let seen: HashSet<String> = all_lines.iter().filter(|l| shown.contains(*l) || prev.contains(**l)).map(|l| l.to_string()).collect();
            inn.seen_rows.insert(wkey_s.clone(), (snap0.clone(), seen));
        }
        inn.add("auto_obs_chars", lines.iter().map(|l| l.len() + 1).sum::<usize>() as f64);
        inn.add("auto_obs_chars_full", all_lines.iter().take(MAX_TREE_LINES).map(|l| l.len() + 1).sum::<usize>() as f64);
        let mut busy = if loading_rx().is_match(&md) { " The UI still looks busy.".to_string() } else { String::new() };
        if partial && !shot && inn.cfg.text_only && extra.is_none() {
            busy.push_str(" The tree is partial here (grid/canvas): what is not listed cannot be read from the tree.");
        } else if partial && !shot {
            busy.push_str(" The tree is partial here (grid/canvas); read with include_screenshot:true before aiming by x/y.");
        }
        let snap = pystr(scv.get("snapshot_id"));
        let extra_s = extra.map(|e| format!("\n{e}")).unwrap_or_default();
        push_text(res, format!("[gate] Window right after the action (snapshot {snap}; tokens are {snap}:N, older tokens are stale).{busy}{delta_note}\n{note}{}{more}{extra_s}", lines.join("\n")));
        if shot {
            if let Some(imgs) = r.get("content").and_then(|c| c.as_array()) {
                let imgs: Vec<Value> = imgs.iter().filter(|c| s(c, "type") == "image").cloned().collect();
                if let Some(arr) = res.get_mut("content").and_then(|c| c.as_array_mut()) {
                    arr.extend(imgs);
                }
            }
        }
        inn.inc("auto_obs");
        let aid = inn.aid;
        inn.last_auto.insert(pid, json!({"aid": aid, "t": now(), "wid": a.get("window_id"), "shot": shot, "snap": snap, "truncated": !more.is_empty()}));
        self.log.write(json!({"ev": "obs", "tool": "get_window_state", "auto": true, "ms": round_to(t0.elapsed().as_secs_f64() * 1000.0, 1),
                              "screenshot": shot, "px": 0, "snapshot": snap, "pid": pid, "trimmed": !shot, "new_window": !note.is_empty(),
                              "rows": all_lines.len(), "rows_sent": lines.len(), "diff": !delta_note.is_empty()}));
    }

    // ------------------------------------------------------------------ risk
    fn dialog_text(&self, inn: &Inner, ctx: &ActionCtx) -> String {
        let t = &ctx.target;
        let mut txt = t.obs.map(|o| inn.state.get(o).text.clone()).unwrap_or_default();
        if let Some(p) = t.pid {
            txt = format!("{}\n{}", newest_obs_text(&inn.state, p), txt);
        }
        trunc(&txt, 6000)
    }

    /// A step the predictor doubts: the gate's own reviewer decides; without one, a person when the run is stuck,
    /// otherwise the formula's next-best option. The acting model is never asked to approve its own step.
    fn review_step(&self, inn: &mut Inner, ctx: &ActionCtx, dec: Decision, trail: &mut Vec<String>, judge: &mut Value) -> Decision {
        let t = &ctx.target;
        // the agent re-sends a step the reviewer doubted after the gate's fresh look: the step review only saves
        // turns (risky effects go through risk_gate), so a second doubt would just loop; let it run once more.
        if dec.tier != "rule" && inn.step_doubted.contains(&ctx.signature) && step_insist_enabled() {
            inn.step_doubted.remove(&ctx.signature);
            inn.inc("review_insist");
            trail.push("reviewer_insist".into());
            return Decision::new("direct", dec.p_direct, "re-sent after the reviewer's doubt and a fresh look; runs (step review is advisory)", "review").with_scores(&dec.scores);
        }
        if let Some(rv) = Some(&self.reviewer).filter(|r| r.available()) {
            let keys = if truthy(ctx.args.get("keys")) { ctx.args.get("keys").cloned() } else { ctx.args.get("key").cloned() };
            let action = json!({"tool": ctx.tool, "target_kind": t.kind, "target_role": t.the_role(), "target_label": t.the_label(),
                "x": t.x, "y": t.y, "typed_text": trunc(&pystr(ctx.args.get("text")), 300), "keys": keys, "why_doubted": dec.reason});
            let mut action = action;
            let (recent, line) = self.review_context(inn, ctx);
            action["recent_actions"] = recent;
            if let Some(l) = line {
                action["terminal_input_line"] = json!(trunc(&l, 300));
            }
            let window = t.pid.and_then(|p| inn.state.latest_for(Some(p), t.window_id)).map(|o| inn.state.get(o).text.clone()).unwrap_or_default();
            inn.inc("reviews");
            let r = rv.check_step(&Gate::ctx_task(inn), &action, &window);
            *judge = r.clone().unwrap_or(json!({"error": "reviewer did not answer"}));
            judge["sent"] = json!({"action": action, "window_tail": tail(&window, 4000)});
            if let Some(r) = r {
                if s(&r, "decision") == "allow" {
                    inn.inc("review_allowed");
                    trail.push("reviewer_allow".into());
                    return Decision::new("direct", dec.p_direct, format!("independent reviewer: {}", s(&r, "reason")), "review").with_scores(&dec.scores);
                }
                trail.push("reviewer_deny".into());
                inn.step_doubted.insert(ctx.signature.clone());
                return Decision::new("look_light", dec.p_direct, format!("the gate's reviewer doubts this step: {}", s(&r, "reason")), "review").with_scores(&dec.scores);
            }
            trail.push("reviewer_down".into());
        }
        if self.human.resolve_channel() != "deny" && dec.tier == "rule" {
            return Decision::new("ask_help", dec.p_direct, dec.reason.clone(), "rule").with_scores(&dec.scores);
        }
        if dec.tier == "rule" {
            return Decision::new("stop_loop", dec.p_direct, dec.reason.clone(), "rule").with_scores(&dec.scores);
        }
        // formula picked review but no reviewer answered: take the best remaining option
        let best = dec.scores.iter().filter(|(k, _)| k.as_str() != "review").fold(("direct".to_string(), f64::INFINITY),
            |acc, (k, v)| if *v < acc.1 { (k.clone(), *v) } else { acc }).0;
        Decision::new(&best, dec.p_direct, format!("{} (reviewer unavailable)", dec.reason), "model").with_scores(&dec.scores)
    }

    /// Authorization (+ independent reviewer for medium risk). None = let it through (and `inn.approval` names the exact
    /// effect that was approved and by whom; gated() turns it into one-shot dispatch permits).
    fn risk_gate(&self, inn: &mut Inner, ctx: &ActionCtx, dec: Decision, trail: &mut Vec<String>) -> Option<Decision> {
        let t = &ctx.target;
        let risk = ctx.risk.clone().unwrap_or_default();
        let dialog = self.dialog_text(inn, ctx);
        let key = self.effect_key(inn, ctx);
        let hz = key.hazard.clone();
        inn.risk_info = json!({"effect": key.to_json(), "hazard": hz});
        // a denial is a record covering the same effect by any route (another button, key, command, tool)
        if let Some((dk, prev)) = inn.denials.iter().find(|(d, _)| d.denial_covers(&key)).cloned() {
            trail.push("denied_before".into());
            inn.risk_info["denied_before"] = json!(dk.to_json());
            return Some(Decision::new("deny", 0.0, format!("{}; the same effect was already refused ({prev})", dec.reason), "rule"));
        }
        // a command has no window: the working-document exception looks at every open window's title
        let titles = if ctx.tool == "run_command" { self.all_windows(inn).into_iter().map(|w| w.2).collect() }
            else { t.pid.map(|p| self.titles(inn, p)).unwrap_or_default() };
        let label = t.the_label().to_string();
        // only the operator's request (--task / GUI_GATE_TASK) can authorize; what the agent stated never does
        let cwd = if ctx.tool == "run_command" { Some(crate::features::command_cwd(&ctx.args)) }
            else { t.pid.filter(|p| crate::sidefx::is_terminal(*p)).map(crate::sidefx::shell_cwd) };
        let (ok, why) = authz::authorized(&key, &inn.cfg.task, &titles, cwd.as_deref());
        inn.risk_info["authorized"] = json!(ok);
        inn.risk_info["why"] = json!(why);
        if ok {
            trail.push("authorized".into());
            inn.inc("authorized");
            inn.approval = Some((key, if why.starts_with("replaces the document") { "working_document".into() } else { "operator_task".into() }));
            return None;
        }
        if let (Some(rv), true) = (Some(&self.reviewer).filter(|r| r.available()), dec.option == "risk_review") {
            let keys = if truthy(ctx.args.get("keys")) { ctx.args.get("keys").cloned() } else { ctx.args.get("key").cloned() };
            let action = json!({"tool": ctx.tool, "target_role": t.the_role(), "target_label": label, "rule_that_fired": risk,
                "risk_level": ctx.risk_level, "typed_text": trunc(&pystr(ctx.args.get("text")), 300), "keys": keys,
                "command": if ctx.tool == "run_command" { json!(trunc(&pystr(ctx.args.get("command")), 1500)) } else { Value::Null },
                "command_cwd": if ctx.tool == "run_command" { json!(crate::features::command_cwd(&ctx.args)) } else { Value::Null },
                "window_titles": titles.iter().take(4).collect::<Vec<_>>(), "dialog_text": tail(&dialog, 1500)});
            let mut action = action;
            let (recent, line) = self.review_context(inn, ctx);
            action["recent_actions"] = recent;
            if let Some(l) = line {
                action["terminal_input_line"] = json!(trunc(&l, 300));
            }
            if inn.cfg.task.trim().is_empty() && !inn.agent_task.is_empty() {
                action["agent_stated_request"] = json!(format!("(the agent's own account of the request, NOT the user's verified words; it cannot authorize anything) {}", trunc(&inn.agent_task, 1500)));
            }
            action["gate_authorization_check"] = json!(why);
            let r = rv.review(&inn.cfg.task, &action);
            inn.risk_info["reviewer"] = r.clone().unwrap_or(Value::Null);
            inn.risk_info["reviewer_sent"] = action.clone();
            inn.inc("reviews");
            if let Some(r) = r {
                if s(&r, "decision") == "allow" {
                    trail.push("reviewer_allow".into());
                    inn.approval = Some((key, "reviewer".into()));
                    return None;
                }
                trail.push("reviewer_deny".into());
                return Some(Decision::new("deny", 0.0, format!("{}; independent reviewer: {}", dec.reason, s(&r, "reason")), "review"));
            }
            trail.push("reviewer_down".into());
        }
        if dec.option == "risk_review" {
            // no reviewer API configured: fail closed, nobody else (and never the acting agent) may approve it
            if !self.reviewer.available() {
                trail.push("no_reviewer".into());
                return Some(Decision::new("deny", 0.0, format!("{}; {}, so nobody approved it", dec.reason, crate::reviewer::NOT_CONFIGURED), "rule"));
            }
            // the reviewer did not answer: a person if one is reachable, otherwise refuse with the safe way on
            if self.human.resolve_channel() != "deny" {
                return Some(Decision::new("ask_human", 0.0, dec.reason.clone(), "rule"));
            }
            return Some(Decision::new("deny", 0.0, format!("{}; the reviewer did not answer, so nobody approved it", dec.reason), "rule"));
        }
        Some(dec)
    }

    /// Deny-and-continue: say no, remember the effect, give the safe way on; escalate after repeated denials.
    fn deny_text(&self, inn: &mut Inner, ctx: &ActionCtx, dec: &Decision, who: &str) -> String {
        inn.denials_consec += 1;
        inn.denials_total += 1;
        inn.inc("denials");
        let risk = ctx.risk.clone().unwrap_or_default();
        if ctx.risk.is_some() || ctx.handoff.is_some() {
            // a record, never a credential: it blocks this effect by any route and is never cleared by a later approval
            let key = self.effect_key(inn, ctx);
            self.log.write(json!({"ev": "denial", "aid": inn.aid, "effect": key.to_json(), "who": who}));
            inn.denials.push((key, who.to_string()));
        }
        let cleared = match (ctx.target.pid, ctx.risk.is_some()) {
            (Some(p), true) => self.clear_terminal_line(inn, p, ctx.target.window_id).unwrap_or_default(),
            _ => String::new(),
        };
        if inn.denials_consec >= inn.cfg.deny_consecutive || inn.denials_total >= inn.cfg.deny_total {
            inn.inc("deny_escalations");
            return format!("[gate] Not executed: {who} ({}). The gate has now refused {} risky step(s) ({} in a row): stop here and tell the user what you wanted to do and why.{cleared}",
                           dec.reason, inn.denials_total, inn.denials_consec);
        }
        format!("[gate] Not executed: {who} ({}). Do not retry it or reach the same result another way (another button, key or command). Instead: {}.{cleared}",
                dec.reason, authz::alternative(&risk))
    }

    /// The agent re-reads a window right after the auto-observe that came with its last action.
    fn duplicate_read(&self, inn: &mut Inner, args: &Value) -> Option<Value> {
        let pid = as_i64(args.get("pid"))?;
        let c = inn.last_auto.get(&pid)?.clone();
        if as_i64(c.get("aid")) != Some(inn.aid as i64) || now() - c["t"].as_f64().unwrap_or(0.0) > 20.0
            || truthy(c.get("truncated")) || !truthy(c.get("snap")) {
            return None;
        }
        if truthy(args.get("window_id")) && truthy(c.get("wid")) && as_i64(args.get("window_id")) != as_i64(c.get("wid")) {
            return None;
        }
        if truthy(args.get("include_screenshot")) && !truthy(c.get("shot")) {
            return None;
        }
        if ["since", "tree_format", "max_elements", "query", "capture_mode"].iter().any(|k| args.get(*k).is_some()) {
            return None;
        }
        inn.inc("dup_reads_saved");
        let snap = pystr(c.get("snap"));
        self.log.write(json!({"ev": "obs", "tool": "get_window_state", "cached": true, "snapshot": snap, "pid": pid, "screenshot": false, "ms": 0}));
        Some(json!({"content": [{"type": "text", "text": format!("[gate] Nothing has been done since snapshot {snap}, which came back with your last action (same window, its tokens {snap}:N are current). Use that tree (when it listed only changed rows, the other rows are as in your earlier reads, same [N]). Call again with fresh:true only if you really need a new read.")}],
                    "structuredContent": {"snapshot_id": snap, "pid": pid, "window_id": c.get("wid"), "cached": true}}))
    }

    // ------------------------------------------------------------------ gate tools
    /// The request as context (briefing, checks, roles): the operator's, else what the agent stated.
    pub fn ctx_task(inn: &Inner) -> String {
        if inn.cfg.task.trim().is_empty() { inn.agent_task.clone() } else { inn.cfg.task.clone() }
    }

    fn set_task(&self, args: &Value) -> Value {
        let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let req = pystr(args.get("request")).trim().to_string();
        if inn.task_locked {
            return text_result("[gate] The user's request is already set; it cannot be changed.", true);
        }
        if req.is_empty() {
            return text_result("[gate] request is empty", true);
        }
        if inn.stat("actions") > 0.0 {
            return text_result("[gate] Too late: the request can only be given before the first action.", true);
        }
        // context only: briefing, plan checks, the step reviewer. It never authorizes a risky step - only the operator's
        // --task / GUI_GATE_TASK does (a model could put anything here, or copy text a web page planted).
        inn.task_source = Some("agent".into());
        inn.agent_task = trunc(&req, 4000);
        inn.task_locked = true;
        self.log.write(json!({"ev": "task", "source": "agent", "chars": req.chars().count()}));
        text_result("[gate] Noted (as context; it does not authorize risky steps).", false)
    }

    fn set_plan(&self, args: &Value) -> Value {
        let state_plan = crate::v097::on("STATE_PLAN");
        let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut notes: Vec<String> = vec![];
        if let Some(ms) = args.get("milestones").and_then(|v| v.as_array()) {
            // 0.9.7: a milestone may be {state, check}; re-sent milestones keep their done marks
            let mut items: Vec<(String, Option<Value>)> = vec![];
            for m in ms.iter().take(12) {
                let (t, c) = if m.is_object() && state_plan {
                    (pystr(m.get("state").or(m.get("milestone")).or(m.get("text"))), m.get("check").filter(|c| c.is_object()).cloned())
                } else {
                    (pystr(Some(m)), None)
                };
                let t = trunc(t.trim(), 160);
                if !t.is_empty() {
                    items.push((t, c));
                }
            }
            if !items.is_empty() {
                let old: Vec<(String, bool)> = std::mem::take(&mut inn.plan);
                let old_checks = std::mem::take(&mut inn.plan_checks);
                inn.plan = items.iter().map(|(t, _)| (t.clone(), state_plan && old.iter().any(|o| o.0 == *t && o.1))).collect();
                inn.plan_checks = items.iter().map(|(t, c)| c.clone().or_else(|| old.iter().position(|o| o.0 == *t).and_then(|i| old_checks.get(i).cloned().flatten()))).collect();
                if state_plan {
                    let recipe: Vec<usize> = inn.plan.iter().enumerate().filter(|(_, m)| crate::plancheck::recipe_like(&m.0)).map(|(i, _)| i + 1).collect();
                    if !recipe.is_empty() {
                        inn.inc("plan_recipe_warn");
                        notes.push(format!("milestone(s) {} read like clicks; restate them as the END STATE they produce (what will be true / saved / shown), so another route still counts if this one breaks", recipe.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", ")));
                    }
                }
            }
        }
        if state_plan {
            for c in args.get("checks").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
                let n = as_i64(c.get("n")).unwrap_or(0);
                if n >= 1 && (n as usize) <= inn.plan.len() && c.get("check").map(|x| x.is_object()).unwrap_or(false) {
                    let i = n as usize - 1;
                    if inn.plan_checks.len() < inn.plan.len() { let l = inn.plan.len(); inn.plan_checks.resize(l, None); }
                    inn.plan_checks[i] = c.get("check").cloned();
                }
            }
            if let Some(f) = args.get("facts").and_then(|v| v.as_object()) {
                for (k, v) in f {
                    inn.facts.retain(|x| x.0 != *k);
                    if !v.is_null() {
                        inn.facts.push((trunc(k, 60), trunc(&pystr(Some(v)), 300)));
                    }
                }
                inn.facts.truncate(30);
            }
        }
        for d in args.get("done").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
            if let Some(i) = as_i64(Some(&d)) {
                if i >= 1 && (i as usize) <= inn.plan.len() {
                    inn.plan[i as usize - 1].1 = true;
                }
            }
        }
        let feasible = args.get("feasible").map(|v| truthy(Some(v))).unwrap_or(true);
        let reason = trunc(pystr(args.get("reason")).trim(), 400);
        inn.inc("plan_calls");
        if !feasible {
            inn.plan_verdict = format!("infeasible: {reason}");
            inn.inc("plan_infeasible");
            self.log.write(json!({"ev": "plan", "feasible": false, "reason": reason, "milestones": inn.plan.len()}));
            return text_result(format!("[gate] Noted: you judge this task infeasible here ({}). If you are sure, stop and reply FAIL with that reason; if not, set_plan again with feasible:true and continue.", if reason.is_empty() { "no reason given" } else { &reason }), false);
        }
        if args.get("feasible").is_some() || inn.plan_verdict.is_empty() {
            inn.plan_verdict = "feasible".into();
        }
        let status = if state_plan && inn.plan_checks.iter().any(|c| c.is_some()) { self.plan_check_status(&mut inn) } else { vec![] };
        self.log.write(json!({"ev": "plan", "feasible": true, "milestones": inn.plan.len(), "done": inn.plan.iter().filter(|m| m.1).count(),
                              "checks": inn.plan_checks.iter().filter(|c| c.is_some()).count(), "facts": inn.facts.len(), "notes": notes.len()}));
        let list: Vec<String> = inn.plan.iter().enumerate().map(|(i, m)| {
            let st = status.iter().find(|x| x.0 == i).map(|x| format!("  [check: {}]", x.1)).unwrap_or_default();
            format!("{}. [{}] {}{st}", i + 1, if m.1 { "x" } else { " " }, m.0)
        }).collect();
        let mut t = format!("[gate] Plan noted:\n{}", list.join("\n"));
        if !inn.facts.is_empty() {
            t.push_str(&format!("\nFacts: {}", inn.facts.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("; ")));
        }
        for n in notes {
            t.push_str(&format!("\n[gate] {n}"));
        }
        text_result(t, false)
    }

    /// What the milestone predicates say now: (milestone index, "holds: ..." / "not yet: ..." / "unknown: ...").
    fn plan_check_status(&self, inn: &mut Inner) -> Vec<(usize, String)> {
        let ctx = self.plan_ctx(inn);
        let mut out = vec![];
        for (i, c) in inn.plan_checks.clone().iter().enumerate() {
            if let Some(c) = c {
                let (r, why) = crate::plancheck::eval(c, &ctx);
                out.push((i, format!("{} {why}", match r { Some(true) => "holds:", Some(false) => "not yet:", None => "unknown:" })));
            }
        }
        out
    }

    fn plan_ctx(&self, inn: &mut Inner) -> crate::plancheck::Ctx {
        let home = home();
        let wins = self.all_windows(inn);
        let mut roots = vec![home.clone(), format!("{home}/Desktop"), format!("{home}/Documents")];
        let mut terminal = String::new();
        let mut cells = String::new();
        let need_cells = inn.plan_checks.iter().flatten().any(|c| c.get("cell").is_some());
        for (p, w, title) in &wins {
            if crate::sidefx::is_terminal(*p) {
                let c = crate::sidefx::shell_cwd(*p);
                if !roots.contains(&c) { roots.push(c); }
                if terminal.is_empty() {
                    terminal = crate::atspi::terminal_text(*p, title, Duration::from_millis(800)).unwrap_or_default();
                }
            }
            if need_cells && cells.is_empty() && title.contains("LibreOffice") && is_spreadsheet_app(*p) {
                let key = format!("{p}:{w}");
                cells = self.tree_extra(inn, *p, &key, &format!("frame = \"{title}\"\n(cell grid)\ncell grid not expanded"), false).unwrap_or_default();
            }
        }
        crate::plancheck::Ctx { titles: wins.into_iter().map(|w| w.2).collect(), terminal, cells, roots, start: crate::sidefx::session_start() }
    }

    /// 0.9.7 task briefing (Intelligence-Indeed dynamic skill + Pointer feasibility gate / planner + HIPPO memory):
    /// built once when the task is known, from read-only facts, the feasibility and planner roles when configured,
    /// the built-in skills and the experience memory. None when no feature that contributes to it is on.
    fn briefing(&self) -> Option<String> {
        let feas = crate::v097::on("FEASIBILITY_FIRST");
        let skills = crate::v097::on("SKILLS");
        let planr = crate::v097::on("STATE_PLAN") && self.roles.get("planner").is_some();
        let mem = crate::memory::path().is_some();
        if !(feas || skills || planr || mem) {
            return None;
        }
        let t0 = Instant::now();
        let (task, titles, wtext) = {
            let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if inn.briefed || Gate::ctx_task(&inn).trim().is_empty() {
                return None;
            }
            inn.briefed = true;
            let w = self.all_windows(&mut inn);
            let titles: Vec<String> = w.iter().map(|x| x.2.clone()).filter(|t| !DESKTOP_WINDOWS.contains(&t.as_str())).collect();
            let wtext = w.iter().filter(|x| !DESKTOP_WINDOWS.contains(&x.2.as_str())).map(|(p, id, t)| format!("pid {p} window {id}: \"{t}\"")).collect::<Vec<_>>().join("\n");
            (Gate::ctx_task(&inn), titles, wtext)
        };
        let wt = wtext.clone();
        let windows = move || if wt.is_empty() { "(no app windows open)".to_string() } else { wt.clone() };
        let facts = crate::inspect::env_facts(&task, "");
        let mut parts: Vec<String> = vec![];
        let mut ev = json!({"ev": "briefing"});
        let mut feas_notes = String::new();
        if feas {
            let mut sec = String::new();
            if let Some(role) = self.roles.get("feasibility") {
                let prompt = format!("TASK (verbatim): {task}\n\nOPEN WINDOWS:\n{}\n\nFACTS THE GATE ALREADY PROBED:\n{facts}", windows());
                let (fin, log, calls, err) = crate::roles::probe_agent(role, crate::roles::FEASIBILITY_SYSTEM, &prompt, &windows, true);
                ev["feasibility"] = json!({"calls": calls, "probes": log.len(), "error": err, "verdict": fin.as_ref().and_then(|f| f.get("feasible")).cloned()});
                if let Some(f) = fin {
                    let ok = f.get("feasible").and_then(|b| b.as_bool()).unwrap_or(true);
                    let notes: Vec<String> = f.get("notes").and_then(|n| n.as_array()).map(|a| a.iter().map(|x| pystr(Some(x))).collect()).unwrap_or_default();
                    let evid: Vec<String> = f.get("evidence").and_then(|n| n.as_array()).map(|a| a.iter().map(|x| pystr(Some(x))).collect()).unwrap_or_default();
                    feas_notes = format!("{} {}", pystr(f.get("reason")), notes.join(" "));
                    sec.push_str(&format!("Feasibility check by the gate's {} ({}): {} - {}\n", role.name, role.backend(), if ok { "FEASIBLE" } else { "INFEASIBLE" }, pystr(f.get("reason"))));
                    if !evid.is_empty() { sec.push_str(&format!("Evidence: {}\n", evid.join("; "))); }
                    if !notes.is_empty() { sec.push_str(&format!("Notes for the work: {}\n", notes.join("; "))); }
                    if !ok {
                        sec.push_str("If you agree, call set_plan with feasible:false and the reason, and reply FAIL. If you have evidence it can be done, say so in set_plan and go on.\n");
                        let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                        inn.inc("briefing_infeasible");
                    }
                } else {
                    sec.push_str(&format!("(the feasibility role gave no verdict: {})\n", err.unwrap_or_default()));
                }
            }
            sec.push_str(&facts);
            sec.push_str("\nDecide first whether the task can be done here: call set_plan with feasible true/false (required before your first action). Infeasible = the app lacks the feature, the files / hardware are missing, the thing asked for does not exist, or it needs a workaround.");
            parts.push(format!("## Feasibility\n{sec}"));
        }
        if skills {
            let sk = crate::inspect::select_skills(&task, &titles);
            let mut sec = String::new();
            for (n, b) in &sk {
                sec.push_str(&format!("[{n}] {b}\n"));
            }
            ev["skills"] = json!(sk.iter().map(|x| x.0.clone()).collect::<Vec<_>>());
            parts.push(format!("## Skills\n{}", sec.trim_end()));
        }
        if planr {
            let role = self.roles.get("planner").unwrap();
            let prompt = format!("TASK (verbatim): {task}\n\nOPEN WINDOWS:\n{}\n\nFACTS:\n{facts}\n\nFEASIBILITY NOTES: {feas_notes}", windows());
            let (fin, log, calls, err) = crate::roles::probe_agent(role, crate::roles::PLANNER_SYSTEM, &prompt, &windows, true);
            ev["planner"] = json!({"calls": calls, "probes": log.len(), "error": err});
            if let Some(f) = fin {
                let ms: Vec<Value> = f.get("milestones").and_then(|m| m.as_array()).cloned().unwrap_or_default();
                let items: Vec<(String, Option<Value>)> = ms.iter().take(12).map(|m| if m.is_object() {
                    (trunc(pystr(m.get("state")).trim(), 160), m.get("check").filter(|c| c.is_object()).cloned())
                } else { (trunc(pystr(Some(m)).trim(), 160), None) }).filter(|x| !x.0.is_empty()).collect();
                if !items.is_empty() {
                    let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    if inn.plan.is_empty() {
                        inn.plan = items.iter().map(|x| (x.0.clone(), false)).collect();
                        inn.plan_checks = items.iter().map(|x| x.1.clone()).collect();
                    }
                    let list: Vec<String> = items.iter().enumerate().map(|(i, x)| format!("{}. {}{}", i + 1, x.0, x.1.as_ref().map(|c| format!("  (check {c})")).unwrap_or_default())).collect();
                    let inf = pystr(f.get("infeasible_reason"));
                    parts.push(format!("## Plan (from the gate's planner {}; it is now your plan - revise it with set_plan when you learn more)\n{}{}", role.backend(), list.join("\n"),
                        if inf.trim().is_empty() { String::new() } else { format!("\nThe planner suspects it may be infeasible: {inf}") }));
                    ev["planner"]["milestones"] = json!(items.len());
                }
            }
        }
        if mem {
            let txt = crate::memory::lookup_text(&task, 2);
            if !txt.contains("no earlier") && !txt.contains("not configured") {
                parts.push(format!("## Experience\n{txt}"));
            }
        }
        if parts.is_empty() {
            return None;
        }
        let out = trunc(&format!("[gate] Task briefing (read once; the gate's notes for this task):\n{}", parts.join("\n")), 6000);
        ev["chars"] = json!(out.chars().count());
        ev["ms"] = json!(round_to(t0.elapsed().as_secs_f64() * 1000.0, 1));
        self.log.write(ev);
        let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inn.inc("briefings");
        Some(out)
    }

    /// Evidence for the agent's own completion decision (no model; the gate never says "done" or "not done").
    fn check_done(&self, args: &Value) -> Value {
        let t0 = Instant::now();
        let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let task = Gate::ctx_task(&inn);
        let start = crate::sidefx::session_start();
        let home = home();
        let mut ev: Vec<String> = vec![];
        let mut gaps: Vec<String> = vec![];
        let mut kinds: Vec<&str> = vec![];
        // the folders a bare name in the task usually means
        let mut roots = vec![home.clone(), format!("{home}/Desktop"), format!("{home}/Documents")];
        let wins = self.all_windows(&mut inn);
        for (p, _, _) in &wins {
            if crate::sidefx::is_terminal(*p) {
                let c = crate::sidefx::shell_cwd(*p);
                if !roots.contains(&c) {
                    roots.push(c);
                }
            }
        }
        let mut named: Vec<String> = crate::done::task_paths(&task);
        for f in args.get("files").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
            let f = pystr(Some(&f));
            if !f.trim().is_empty() && !named.contains(&f) {
                named.push(f);
            }
        }
        if task.is_empty() {
            ev.push("Task: not known to the gate (no --task / gate_task), so only the general checks below ran.".into());
        }
        if !named.is_empty() {
            ev.push("Paths named in the task (or passed in files):".into());
            let mut any_changed = false;
            let mut pending: Vec<(String, &str)> = vec![];
            for n in named.iter().take(8) {
                let (line, gap) = crate::done::path_evidence(n, &roots, start);
                ev.push(line);
                match gap {
                    Some(g) if g.contains("does not exist") => pending.push((g, "missing_path")),
                    Some(g) => {
                        if crate::done::asks_to_produce(&task) && named.len() == 1 {
                            pending.push((g, "path_unchanged"));
                        }
                    }
                    None => any_changed = true,
                }
            }
            // a rename / move leaves its source missing by design: a missing path is a gap only when nothing else
            // the task names changed
            for (g, k) in pending {
                if k == "missing_path" && any_changed {
                    continue;
                }
                gaps.push(g);
                kinds.push(k);
            }
            if !any_changed && named.len() > 1 && crate::done::asks_to_produce(&task) {
                gaps.push("the task asks to produce or change something, but none of the paths it names changed during this session".into());
                kinds.push("path_unchanged");
            }
        }
        if let Some(st) = start {
            let ch = crate::done::changed_since(&home, st, 10);
            if ch.is_empty() {
                ev.push("Files changed under the home folder during this session: none".into());
            } else {
                ev.push(format!("Files changed under the home folder during this session (newest first): {}",
                    ch.iter().map(|(p, n, _)| format!("{} ({n} B)", crate::sidefx::relpath(p, &home))).collect::<Vec<_>>().join(", ")));
            }
        }
        let shown: Vec<&(i64, i64, String)> = wins.iter().filter(|w| !DESKTOP_WINDOWS.contains(&w.2.as_str())).collect();
        if !shown.is_empty() {
            ev.push(format!("Open windows: {}", shown.iter().map(|w| format!("\"{}\"", trunc(&w.2, 70))).collect::<Vec<_>>().join(", ")));
        }
        if let Ok(Some(a)) = crate::atspi::active_window(Duration::from_millis(800)) {
            ev.push(format!("Active window: \"{}\"", trunc(&a, 80)));
        }
        for (p, w, title) in shown.iter().take(6) {
            if crate::done::title_unsaved(title) {
                gaps.push(format!("window \"{}\" shows an unsaved-changes marker", trunc(title, 60)));
                kinds.push("unsaved_title");
            }
            if title.contains("LibreOffice") {
                match crate::atspi::status_bar_texts(*p, title, Duration::from_millis(800)).ok().and_then(|t| crate::done::status_says_modified(&t)) {
                    Some(true) => {
                        gaps.push(format!("LibreOffice reports unsaved changes in \"{}\" (save it if the task's result must be in the file)", trunc(title, 60)));
                        kinds.push("unsaved_doc");
                    }
                    Some(false) => {}
                    // LibreOffice 25.2 shows its modified flag only as an image: say that it is not known
                    None => ev.push(format!("Unsaved changes in \"{}\": not visible to the gate (LibreOffice does not expose its modified flag); if you edited it after the last save and the result must be in the file, save it.", trunc(title, 60))),
                }
                if is_spreadsheet_app(*p) {
                    let key = format!("{p}:{w}");
                    if let Some(c) = self.tree_extra(&mut inn, *p, &key, &format!("frame = \"{title}\"\n(cell grid)\ncell grid not expanded"), false) {
                        ev.push(trunc(&c, 1800));
                    }
                }
            }
            if crate::sidefx::is_terminal(*p) {
                if let Ok(tt) = crate::atspi::terminal_text(*p, title, Duration::from_millis(800)) {
                    ev.push(crate::atspi::tail_lines(&tt, 10));
                    let low = tt.to_lowercase();
                    if ["no such file", "permission denied", "command not found", "error:", "traceback (most recent call last)", "fatal:"].iter().any(|k| low.contains(k)) {
                        gaps.push("the terminal text shows an error message (see the tail above)".into());
                        kinds.push("terminal_error");
                    }
                }
            }
        }
        let executed = inn.stat("executed") as i64;
        ev.push(format!("This session: {executed} action(s) executed, {} failed, {} refused by the gate.", inn.stat("exec_errors") as i64, inn.stat("denials") as i64));
        if executed == 0 {
            gaps.push("no action has been executed in this session".into());
            kinds.push("no_action");
        }
        if let Some(last) = inn.recent.last() {
            if truthy(last.get("failed")) {
                gaps.push(format!("your last action failed: {} {} -> {}", s(last, "tool"), s(last, "target"), s(last, "result")));
                kinds.push("last_failed");
            }
        }
        if !inn.denials.is_empty() {
            let eff: Vec<String> = inn.denials.iter().take(3).map(|d| d.0.short()).collect();
            ev.push(format!("The gate refused (not done, and must not be reached another way): {}", eff.join("; ")));
        }
        // 0.9.7: milestone predicates, working memory, saved-document read-back
        let status = if (crate::v097::on("STATE_PLAN") || crate::v097::on("VERIFY_PLUS")) && inn.plan_checks.iter().any(|c| c.is_some()) { self.plan_check_status(&mut inn) } else { vec![] };
        for (i, st) in &status {
            let m = inn.plan.get(*i).map(|m| (m.0.clone(), m.1)).unwrap_or_default();
            ev.push(format!("Milestone {} \"{}\" check: {st}", i + 1, trunc(&m.0, 70)));
            if st.starts_with("not yet") {
                gaps.push(format!("milestone {} check does not hold: {}", i + 1, st.trim_start_matches("not yet: ")));
                kinds.push("plan_check");
            }
        }
        if !inn.facts.is_empty() {
            ev.push(format!("Your facts: {}", inn.facts.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("; ")));
        }
        if crate::v097::on("VERIFY_PLUS") {
            let mut shown_docs = 0;
            let mut cands: Vec<String> = named.iter().filter_map(|n| {
                let e = crate::sidefx::expanduser(n);
                if e.starts_with('/') { Some(e) } else { roots.iter().map(|r| format!("{r}/{e}")).find(|c| std::path::Path::new(c).is_file()) }
            }).collect();
            if let Some(st) = start {
                for (p, _, _) in crate::done::changed_since(&home, st, 10) {
                    if !cands.contains(&p) { cands.push(p); }
                }
            }
            for p in cands {
                let ext = crate::inspect::ext_of(&p);
                if shown_docs >= 2 || !std::path::Path::new(&p).is_file() || !["docx", "xlsx", "pptx", "odt", "ods", "odp", "csv", "txt", "md", "json", "html", "py", "sh"].contains(&ext.as_str()) {
                    continue;
                }
                shown_docs += 1;
                ev.push(format!("Saved content of {} (what a checker reading the file sees):\n{}", crate::sidefx::relpath(&p, &home), trunc(&crate::inspect::read_file_text(&p, 900), 1100)));
            }
        }
        let open_ms: Vec<String> = inn.plan.iter().enumerate().filter(|(i, m)| !m.1 && !status.iter().any(|x| x.0 == *i && x.1.starts_with("holds"))).map(|(_, m)| m.0.clone()).collect();
        let held: Vec<usize> = inn.plan.iter().enumerate().filter(|(i, m)| !m.1 && status.iter().any(|x| x.0 == *i && x.1.starts_with("holds"))).map(|(i, _)| i + 1).collect();
        if !held.is_empty() {
            ev.push(format!("Milestone(s) {} are not marked done but their checks hold (mark them with set_plan done).", held.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", ")));
        }
        if !open_ms.is_empty() {
            gaps.push(format!("plan milestones not marked done: {}", open_ms.join("; ")));
            kinds.push("plan_open");
        }
        let mut txt = String::from("[check_done] Evidence for your own decision (the gate does not judge the task; it cannot see formatting, colours or whether content is right):\n");
        txt.push_str(&ev.join("\n"));
        if gaps.is_empty() {
            txt.push_str("\nPossible gaps: none found in what the gate can see.");
        } else {
            txt.push_str("\nPossible gaps (check each before replying DONE):");
            for g in &gaps {
                txt.push_str(&format!("\n- {g}"));
            }
        }
        // 0.9.7 independent end-state verifier (Pointer verifier / VLAA verifier judge), optional model role
        let mut verdict = Value::Null;
        if crate::v097::on("VERIFY_PLUS") && inn.stat("verifier_calls") < 2.0 {
            if let Some(role) = self.roles.get("verifier") {
                inn.inc("verifier_calls");
                let plan: Vec<String> = inn.plan.iter().enumerate().map(|(i, m)| format!("{}. [{}] {}", i + 1, if m.1 { "x" } else { " " }, m.0)).collect();
                let wt: String = wins.iter().filter(|w| !DESKTOP_WINDOWS.contains(&w.2.as_str())).map(|(p, id, t)| format!("pid {p} window {id}: \"{t}\"")).collect::<Vec<_>>().join("\n");
                let prompt = format!("TASK (verbatim): {task}\n\nAGENT'S PLAN:\n{}\n\nGATE EVIDENCE:\n{}\n\nOPEN WINDOWS:\n{wt}", plan.join("\n"), trunc(&txt, 6000));
                let w2 = wt.clone();
                let (fin, log, calls, err) = crate::roles::probe_agent(role, crate::roles::VERIFIER_SYSTEM, &prompt, &move || w2.clone(), true);
                self.log.write(json!({"ev": "role", "role": "verifier", "calls": calls, "probes": log.len(), "error": err, "decision": fin.as_ref().map(|f| pystr(f.get("decision")))}));
                if let Some(f) = fin {
                    let d = pystr(f.get("decision")).to_lowercase();
                    txt.push_str(&format!("\n\nIndependent verifier ({}): {}", role.backend(), d.to_uppercase()));
                    match d.as_str() {
                        "done" => txt.push_str(&format!(" - {}", pystr(f.get("reasoning")))),
                        "infeasible" => txt.push_str(&format!(" - {} If you agree, reply FAIL with this reason.", pystr(f.get("reasoning")))),
                        _ => txt.push_str(&format!("\n  diagnosis: {}\n  suggestion: {}", pystr(f.get("diagnosis")), pystr(f.get("suggestion")))),
                    }
                    if inn.stat("verifier_calls") >= 2.0 {
                        txt.push_str("\n  (verifier budget used: further check_done calls give evidence only)");
                    }
                    verdict = f;
                }
            }
        }
        inn.inc("check_done");
        self.log.write(json!({"ev": "check_done", "gaps": gaps.len(), "kinds": kinds, "named": named.len(), "ms": round_to(t0.elapsed().as_secs_f64() * 1000.0, 1)}));
        let mut r = text_result(txt, false);
        r["structuredContent"] = json!({"gaps": gaps, "kinds": kinds, "verifier": verdict});
        r
    }

    // ------------------------------------------------------------------ 0.9.7 (docs/sota-absorb-2026-10-22.md)

    /// Phantom tools (GUI_GATE_PHANTOM): Ok((tool, args, note)) to run, or Err(result) to answer at once.
    fn phantom(&self, name: &str, args: &Value) -> Result<(String, Value, Option<String>), Value> {
        let known = self.shared.lock().unwrap().tool_names.clone();
        if !crate::v097::on("PHANTOM") || known.is_empty() || known.iter().any(|k| k == name) || name.starts_with("__") {
            return Ok((name.to_string(), args.clone(), None));
        }
        let mapped = crate::v097::phantom_alias(name, args, &known);
        {
            let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inn.inc("phantom_calls");
            if mapped.is_some() { inn.inc("phantom_mapped"); }
        }
        self.log.write(json!({"ev": "phantom", "name": name, "to": mapped.as_ref().map(|m| m.0.clone())}));
        match mapped {
            Some((t, _, _)) if t == "__wait" => {
                let secs = as_f64(args.get("seconds")).or(as_f64(args.get("duration"))).or(as_f64(args.get("time"))).or(as_f64(args.get("ms")).map(|m| m / 1000.0)).unwrap_or(1.0).clamp(0.0, 10.0);
                std::thread::sleep(Duration::from_secs_f64(secs));
                Err(text_result(format!("[gate] there is no tool \"{name}\"; waited {secs:.1} s. Read the window again if you expect a change."), false))
            }
            Some((t, _, _)) if t == "__final" => Err(text_result(format!("[gate] there is no tool \"{name}\": finish by ending your turn with the final reply (DONE, or FAIL with the reason when the task cannot be done here).{}", if check_done_enabled() { " Call check_done before DONE." } else { "" }), false)),
            Some((t, a, n)) => Ok((t, a, if n.is_empty() { None } else { Some(n) })),
            None => {
                let near = crate::v097::nearest_tools(name, &known, 4);
                Err(text_result(format!("[gate] there is no tool \"{name}\" and nothing it maps to; do not try variants of the name. Closest real tools: {}.", near.join(", ")), true))
            }
        }
    }

    /// triple_click (a phantom): click with a click count when the driver has one, else click + Home + shift+End.
    fn triple_click(&self, args: &Value) -> Value {
        let props = self.shared.lock().unwrap().click_props.clone();
        let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut a = args.clone();
        if let Some(p) = ["click_count", "count", "clicks"].iter().find(|p| props.iter().any(|x| x == *p)) {
            a[*p] = json!(3);
            let mut r = self.gated(&mut inn, "click", a);
            push_text(&mut r, "[gate] there is no tool \"triple_click\"; ran click with a click count of 3.".into());
            return r;
        }
        let r = self.gated(&mut inn, "click", a.clone());
        if is_error(&r) || text_of(&r).starts_with("[gate]") {
            return r;
        }
        let pw = json!({"pid": a.get("pid"), "window_id": a.get("window_id")});
        let mut k1 = pw.clone();
        k1["key"] = json!("Home");
        let _ = self.gated(&mut inn, "press_key", k1);
        let mut k2 = pw;
        k2["keys"] = json!(["shift", "End"]);
        let mut r2 = self.gated(&mut inn, "hotkey", k2);
        push_text(&mut r2, "[gate] there is no tool \"triple_click\"; emulated it: click, then Home and shift+End (selects the line). For a whole paragraph use ctrl+shift+down after Home.".into());
        r2
    }

    fn job_poll(&self, args: &Value) -> Value {
        let id = as_i64(args.get("job")).unwrap_or(-1);
        if id < 0 {
            return text_result("[run_command] mode=poll / stop needs the job number that mode=start returned", true);
        }
        match self.jobs.poll(id as u32, s(args, "mode") == "stop", 4000) {
            Ok((t, v)) => {
                let mut r = text_result(t, false);
                r["structuredContent"] = v;
                r
            }
            Err(e) => text_result(format!("[run_command] {e}"), true),
        }
    }

    /// inspect (GUI_GATE_INSPECT): read-only views; see inspect.rs.
    fn inspect(&self, args: &Value) -> Value {
        let kind = s(args, "kind").to_string();
        let cap = as_i64(args.get("max_chars")).unwrap_or(12000).clamp(500, 60000) as usize;
        let t0 = Instant::now();
        let (task, text_only) = {
            let inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            (Gate::ctx_task(&inn), inn.cfg.text_only)
        };
        let query = pystr(args.get("query"));
        let res = match kind.as_str() {
            "file" => crate::inspect::file_result(s(args, "path"), cap, text_only),
            "env" => text_result(crate::inspect::env_facts(&task, &query), false),
            "page" => match crate::inspect::page_info(&query, cap) {
                Ok(t) => text_result(t, false),
                Err(e) => text_result(format!("[inspect page] {e}"), true),
            },
            "howto" => self.howto(&query),
            "memory" => text_result(crate::memory::lookup_text(if query.trim().is_empty() { &task } else { &query }, 3), false),
            "region" => self.inspect_region(args, text_only),
            "locate" => self.inspect_locate(args, text_only),
            _ => text_result(format!("[inspect] kind must be one of {}", crate::inspect::KINDS.join(", ")), true),
        };
        let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inn.inc("inspect_calls");
        self.log.write(json!({"ev": "inspect", "kind": kind, "ms": round_to(t0.elapsed().as_secs_f64() * 1000.0, 1), "is_error": is_error(&res)}));
        res
    }

    /// 0.9.7 GUI_GATE_SNAP (UiPath crop-and-refine, model-free): a pixel click that lands in no element of the
    /// window's newest read but within 10 px of a labelled one is sent to that element instead (element_index).
    /// Only when the read's element frames live in the screenshot's pixel space (same width bound).
    fn snap_pixel(&self, inn: &mut Inner, name: &str, args: &mut Value) -> Option<String> {
        if !matches!(name, "click" | "double_click" | "right_click") || args.get("element_token").is_some() || args.get("element_index").is_some() {
            return None;
        }
        let (x, y) = (as_f64(args.get("x"))?, as_f64(args.get("y"))?);
        if s(args, "coordinate_frame") == "desktop" {
            return None;
        }
        let pid = as_i64(args.get("pid"))?;
        let o = inn.state.latest_for(Some(pid), as_i64(args.get("window_id")))?;
        let ob = inn.state.get(o);
        let sw = ob.shot_w?;
        let maxx = ob.elements.values().filter_map(|e| e.frame.as_ref().map(|f| f.x + f.w)).fold(0.0, f64::max);
        if maxx <= 0.0 || maxx > sw + 2.0 {
            return None;
        }
        if crate::inspect::element_at(x, y, ob.elements.values()).is_some() {
            return None;
        }
        let (e, d) = crate::inspect::nearest_element(x, y, 10.0, ob.elements.values())?;
        let (idx, role, label) = (e.index, e.role.clone(), e.label.clone());
        if let Some(a) = args.as_object_mut() {
            a.remove("x");
            a.remove("y");
        }
        args["element_index"] = json!(idx);
        inn.inc("snapped");
        self.log.write(json!({"ev": "snap", "x": x, "y": y, "to": idx, "dist": round_to(d, 1)}));
        Some(format!("[gate] your click at ({x:.0},{y:.0}) hit no element; it was {d:.0} px from [{idx}] {role} \"{}\", so the gate clicked that element.", trunc(&label, 50)))
    }

    /// 0.9.7 GUI_GATE_MICRO_VERIFY (VLAA micro-verification rules): what a save or a typing step should have produced,
    /// checked right after it. Notes only.
    fn micro_verify(&self, inn: &mut Inner, name: &str, args: &Value, res: &mut Value) {
        if !crate::v097::on("MICRO_VERIFY") || is_error(res) {
            return;
        }
        let keys = match name {
            "hotkey" => args.get("keys").and_then(|k| k.as_array()).map(|a| a.iter().map(|x| pystr(Some(x)).to_lowercase()).collect::<Vec<_>>().join("+")).unwrap_or_default(),
            "press_key" => format!("{}{}", args.get("modifiers").and_then(|m| m.as_array()).map(|a| a.iter().map(|x| pystr(Some(x)).to_lowercase() + "+").collect::<String>()).unwrap_or_default(), pystr(args.get("key")).to_lowercase()),
            _ => String::new(),
        };
        if keys == "ctrl+s" || keys == "control+s" || keys == "ctrl+shift+s" {
            std::thread::sleep(Duration::from_millis(300));
            let wins = self.all_windows(inn);
            let dlg: Vec<&String> = wins.iter().map(|w| &w.2).filter(|t| { let l = t.to_lowercase(); l.contains("keep current format") || l.contains("save as") || l.contains("save document") || l == "save" || l.contains("export") || l.contains("overwrite") || l.contains("replace") }).collect();
            let unsaved: Vec<&String> = wins.iter().map(|w| &w.2).filter(|t| crate::done::title_unsaved(t)).collect();
            let note = if let Some(d) = dlg.first() {
                format!("[gate] Save check: a dialog \"{}\" is open - the file is not written until you answer it (keep the format the task needs).", trunc(d, 60))
            } else if let Some(u) = unsaved.first() {
                format!("[gate] Save check: \"{}\" still shows an unsaved-changes marker; the save may not have happened (focus elsewhere?).", trunc(u, 60))
            } else {
                "[gate] Save check: no dialog and no unsaved marker in the window titles (the gate cannot see LibreOffice's own modified flag; check_done lists the file's age).".into()
            };
            inn.inc("micro_verify");
            push_text(res, note);
        } else if name == "type_text" && args.get("element_token").or(args.get("element_index")).is_some() {
            let typed = pystr(args.get("text"));
            let probe: String = typed.trim().chars().take(24).collect();
            let rt = text_of(res);
            if probe.chars().count() >= 3 && !probe.contains('\n') && rt.contains("[gate] Window") && !rt.contains(&probe) {
                inn.inc("micro_verify");
                push_text(res, format!("[gate] Typing check: \"{}\" does not appear in the window read after the step; the text may have gone to another field.", trunc(&probe, 24)));
            }
        }
    }

    /// 0.9.7 GUI_GATE_DIGEST (Pointer's compaction summary, OpenAPA session pruning, HIPPO memory-driven context):
    /// every GUI_GATE_DIGEST_EVERY (8) executed steps a compact digest of the task state; the compact role writes it
    /// when configured. The gate cannot drop what the client already holds; the digest is what a client-side
    /// compaction can keep instead of older tool results.
    fn digest(&self) -> Option<String> {
        let every = env("GUI_GATE_DIGEST_EVERY").and_then(|v| v.parse::<usize>().ok()).unwrap_or(8).max(2);
        let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let n = inn.history.len();
        let done_at = inn.stat("digest_at") as usize;
        if n < every || n < done_at + every {
            return None;
        }
        inn.stats.insert("digest_at".into(), json!(n));
        inn.inc("digests");
        let plan: Vec<String> = inn.plan.iter().enumerate().map(|(i, m)| format!("{}. [{}] {}", i + 1, if m.1 { "x" } else { " " }, trunc(&m.0, 90))).collect();
        let facts = inn.facts.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("; ");
        let steps = inn.history.iter().rev().take(every).rev().cloned().collect::<Vec<_>>();
        let failed = inn.history.iter().filter(|h| h.contains("-> FAILED")).count();
        let task = Gate::ctx_task(&inn);
        if let Some(role) = self.roles.get("compact") {
            let prompt = format!("TASK: {task}\n\nPLAN:\n{}\nFACTS: {facts}\n\nALL STEPS SO FAR (oldest first):\n{}", plan.join("\n"), inn.history.join("\n"));
            drop(inn);
            let (fin, _, calls, err) = crate::roles::probe_agent(role, crate::roles::COMPACT_SYSTEM, &prompt, &|| String::new(), false);
            self.log.write(json!({"ev": "role", "role": "compact", "calls": calls, "ok": fin.is_some(), "error": err}));
            if let Some(d) = fin.and_then(|f| f.get("digest").map(|d| pystr(Some(d)))).filter(|d| !d.trim().is_empty()) {
                return Some(format!("[gate] Progress digest after {n} steps ({}; older tool results are superseded by it):\n{}", role.backend(), trunc(&d, 2500)));
            }
            let inn2 = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            return Some(self.digest_text(&inn2, n, &plan, &facts, &steps, failed));
        }
        Some(self.digest_text(&inn, n, &plan, &facts, &steps, failed))
    }

    fn digest_text(&self, inn: &Inner, n: usize, plan: &[String], facts: &str, steps: &[String], failed: usize) -> String {
        let mut d = format!("[gate] Progress digest after {n} steps ({failed} failed; older tool results are superseded by it):\nTask: {}", trunc(&Gate::ctx_task(inn), 200));
        if !plan.is_empty() { d.push_str(&format!("\nPlan:\n{}", plan.join("\n"))); }
        if !facts.is_empty() { d.push_str(&format!("\nFacts: {facts}")); }
        d.push_str(&format!("\nLast {} steps:\n- {}", steps.len(), steps.join("\n- ")));
        if inn.lb.level > 0 { d.push_str(&format!("\nLoop breaker level {}/3 (mostly {} steps without progress).", inn.lb.level, inn.lb.dominant_mode())); }
        d
    }

    /// VLAA tier 3: at loop level 3 the judge role (when configured, with TWO_STRIKES on) says KEEP or SWITCH.
    fn loop_judge(&self, inn: &mut Inner, level: u8) -> Option<String> {
        if level < 3 || !crate::v097::on("TWO_STRIKES") {
            return None;
        }
        let role = self.roles.get("judge")?;
        let recent: Vec<String> = inn.recent.iter().rev().take(10).rev().map(|r| r.to_string()).collect();
        let prompt = format!("TASK: {}\n\nRECENT STEPS (oldest first):\n{}\n\nGATE: loop level {level}/3, mostly {} steps without progress.", Gate::ctx_task(inn), recent.join("\n"), inn.lb.dominant_mode());
        let (fin, _, calls, err) = crate::roles::probe_agent(role, crate::roles::JUDGE_LOOP_SYSTEM, &prompt, &|| String::new(), false);
        self.log.write(json!({"ev": "role", "role": "judge", "use": "loop", "calls": calls, "verdict": fin.as_ref().map(|f| pystr(f.get("verdict"))), "error": err}));
        let f = fin?;
        let v = pystr(f.get("verdict")).to_uppercase();
        inn.inc("judge_loop");
        if v == "SWITCH" {
            Some(format!("[gate] Loop judge ({}): SWITCH - {} Stop repeating: {}.", role.backend(), pystr(f.get("advice")), pystr(f.get("blacklist"))))
        } else {
            Some(format!("[gate] Loop judge ({}): KEEP - the repetition looks intended; go on.", role.backend()))
        }
    }

    /// kind=howto: the search role when configured, else the built-in skills.
    fn howto(&self, query: &str) -> Value {
        if query.trim().is_empty() {
            return text_result("[inspect howto] say what you want to do, e.g. query: \"change the slide number colour in LibreOffice Impress\"", true);
        }
        if let Some(role) = self.roles.get("search") {
            let (fin, _, calls, err) = crate::roles::probe_agent(role, crate::roles::SEARCH_SYSTEM, &format!("Question: {query}"), &|| String::new(), false);
            self.log.write(json!({"ev": "role", "role": "search", "calls": calls, "ok": fin.is_some(), "error": err}));
            if let Some(f) = fin {
                let exists = f.get("exists").and_then(|e| e.as_bool());
                return text_result(format!("[inspect howto] ({}){}\n{}", role.backend(), if exists == Some(false) { " the search role says this feature does not exist in the app" } else { "" }, pystr(f.get("answer"))), false);
            }
        }
        text_result(crate::inspect::howto_from_skills(query), false)
    }

    fn window_shot(&self, inn: &mut Inner, pid: i64, wid: Option<i64>) -> Option<(String, Value)> {
        let mut a = json!({"pid": pid, "include_screenshot": true});
        if let Some(w) = wid { a["window_id"] = json!(w); }
        let r = self.tool(inn, "get_window_state", &a, 60.0);
        if is_error(&r) { return None; }
        if let Some(o) = inn.state.record_observation(&a, &r, None) { let _ = o; }
        let img = r.get("content")?.as_array()?.iter().find(|c| s(c, "type") == "image").map(|c| pystr(c.get("data")))?;
        Some((img, sc(&r)))
    }

    fn inspect_region(&self, args: &Value, text_only: bool) -> Value {
        if text_only {
            return text_result("[inspect region] this session is text-only (the model cannot see images); use the tree rows or kind=locate.", true);
        }
        let Some(pid) = as_i64(args.get("pid")) else { return text_result("[inspect region] needs pid (and window_id) of the window", true) };
        let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some((img, _)) = self.window_shot(&mut inn, pid, as_i64(args.get("window_id"))) else { return text_result("[inspect region] could not take a screenshot of that window", true) };
        let g = |k: &str, d: f64| as_f64(args.get(k)).unwrap_or(d);
        match crate::inspect::crop_zoom(&img, g("x", 0.0), g("y", 0.0), g("w", 200.0), g("h", 150.0), g("scale", 2.0)) {
            Ok((b, (x, y, w, h))) => {
                let z = g("scale", 2.0).clamp(1.0, 4.0);
                json!({"content": [{"type": "text", "text": format!("[inspect region] x={x} y={y} w={w} h={h} of the window screenshot, zoomed {z}x. A point (px, py) in this image is window pixel ({x} + px/{z}, {y} + py/{z}).")},
                                   {"type": "image", "data": b, "mimeType": "image/png"}]})
            }
            Err(e) => text_result(format!("[inspect region] {e}"), true),
        }
    }

    fn inspect_locate(&self, args: &Value, text_only: bool) -> Value {
        let describe = pystr(args.get("describe"));
        let Some(pid) = as_i64(args.get("pid")) else { return text_result("[inspect locate] needs pid (and window_id) and describe", true) };
        if describe.trim().is_empty() {
            return text_result("[inspect locate] describe the element, e.g. \"the Bold button in the toolbar\"", true);
        }
        let wid = as_i64(args.get("window_id"));
        let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut lines = vec![];
        // grounder role (vision): x,y on a fresh screenshot, then snapped to the element under it
        if let Some(role) = self.roles.get("grounder") {
            if let Some((img, scv)) = self.window_shot(&mut inn, pid, wid) {
                let (fin, _, _, err) = crate::roles::probe_agent_image(role, crate::roles::GROUNDER_SYSTEM, &format!("Element to find: {describe}"), &img);
                self.log.write(json!({"ev": "role", "role": "grounder", "ok": fin.is_some(), "error": err}));
                if let Some(f) = fin.filter(|f| f.get("found").and_then(|b| b.as_bool()).unwrap_or(false)) {
                    let (x, y) = (as_f64(f.get("x")).unwrap_or(-1.0), as_f64(f.get("y")).unwrap_or(-1.0));
                    let snap = inn.state.latest_for(Some(pid), wid).and_then(|o| crate::inspect::element_at(x, y, inn.state.get(o).elements.values()).map(|e| (e.index, e.role.clone(), e.label.clone())));
                    lines.push(format!("grounder ({}): x={x:.0} y={y:.0} in the screenshot{} - {}", role.backend(),
                        snap.map(|(i, r, l)| format!(", on element [{i}] {r} \"{l}\" (use element_index {i})")).unwrap_or_default(), pystr(f.get("why"))));
                    let _ = scv;
                }
            }
        }
        let _ = text_only;
        let obs = inn.state.latest_for(Some(pid), wid);
        let Some(o) = obs else {
            return text_result("[inspect locate] read the window first (get_window_state), then locate", true);
        };
        let ranked = crate::inspect::rank_elements(&describe, inn.state.get(o).elements.values());
        for (sc_, i, r, l) in &ranked {
            lines.push(format!("[{i}] {r} \"{l}\" (match {:.0}%)", (sc_.min(1.5) / 1.5) * 100.0));
        }
        if lines.is_empty() {
            return text_result(format!("[inspect locate] nothing in the last read of this window matches \"{describe}\" (snapshot {}); it may be off-screen or in a closed menu.", inn.state.get(o).snapshot_id), false);
        }
        text_result(format!("[inspect locate] candidates for \"{describe}\" in snapshot {} (click with element_index from this snapshot):\n{}", inn.state.get(o).snapshot_id, lines.join("\n")), false)
    }

    fn request_human(&self, args: &Value) -> Value {
        let q0 = pystr(args.get("question")).trim().to_string();
        let q = if q0.is_empty() { "The agent needs your help.".to_string() } else { q0 };
        let kind = { let k = pystr(args.get("kind")); if k.is_empty() { "answer".to_string() } else { k } };
        let ch = self.human.resolve_channel();
        let timeout = {
            let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inn.inc("human_asked");
            inn.cfg.human_timeout
        };
        if ch == "chat" {
            let mut r = text_result("[gate] There is no direct line to a person from here. Ask your user in your reply and wait for their answer.", false);
            r["structuredContent"] = json!({"status": "input_required", "human_outcome": "channel_unavailable"});
            return r;
        }
        let text = if kind != "takeover" { q.clone() } else { format!("{q}\n\nDo it in the app yourself, then confirm when you are done.") };
        let kind = if ["approve", "answer", "takeover"].contains(&kind.as_str()) { kind } else { "answer".to_string() };
        let title = match kind.as_str() { "approve" => "gui-gate: approve?", "takeover" => "gui-gate: your turn", _ => "gui-gate: question" };
        let h = self.human.ask_kind(&kind, &text, title, json!({"tool": "request_human", "reason": "asked by the agent"}));
        if s(&h, "answer") == "input_required" {
            return text_result("[gate] waiting for a person", true);
        }
        self.log.write(json!({"ev": "request_human", "kind": kind, "question": trunc(&q, 200), "human": h, "human_outcome": h.get("outcome")}));
        let ans = s(&h, "answer").to_string();
        let msg = if ans == "timeout" {
            format!("No answer from a person within {} s. Do not guess; stop and report.", timeout as i64)
        } else if kind == "takeover" {
            if ans == "allow" { "The person says the step is done. Re-read the window and continue.".into() } else { "The person declined to take over. Stop and report.".into() }
        } else if kind == "approve" {
            if ans == "allow" { "Approved (this answers your question only; it does not unlock any step the gate stops).".into() } else { "Not approved. Do not do it.".into() }
        } else if ans != "deny" {
            format!("The person answered: {ans}")
        } else {
            "The person cancelled. Stop and report.".into()
        };
        json!({"content": [{"type": "text", "text": format!("[human via {}] {msg}", s(&h, "channel"))}],
               "structuredContent": {"answer": ans, "channel": h["channel"], "ms": h["ms"], "human_outcome": h.get("outcome")}})
    }

    fn report(&self) -> Value {
        let inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut st = inn.stats.clone();
        let g = |k: &str| inn.stat(k);
        st.insert("session_s".into(), json!(round_to(now() - self.started, 1)));
        st.insert("policy".into(), json!(inn.cfg.policy));
        st.insert("lambda".into(), json!(inn.cfg.lam));
        st.insert("model".into(), json!(inn.cfg.model_name));
        let img = (g("screenshot_px") / 750.0) as i64;
        st.insert("approx_image_tokens".into(), json!(img));
        let per = if g("actions") > 0.0 { round_to(g("gate_ms") / g("actions"), 1) } else { 0.0 };
        st.insert("gate_ms_per_action".into(), json!(per));
        st.insert("log".into(), json!(self.log.path));
        st.insert("reviewer".into(), json!(self.reviewer.backend()));
        st.insert("screenshot_format".into(), json!(self.shot.name()));
        st.insert("features_097".into(), json!(crate::v097::active()));
        st.insert("roles".into(), json!(self.roles.configured()));
        let lines = [
            format!("policy={} lambda={} model={}", inn.cfg.policy, inn.cfg.lam, inn.cfg.model_name),
            format!("actions={} executed={} errors={}", g("actions"), g("executed"), g("exec_errors")),
            format!("decisions={}", st.get("by_option").cloned().unwrap_or(json!({}))),
            format!("observations={} (screenshots={}, ~{img} image tokens)", g("obs_calls"), g("screenshots")),
            format!("extra model turns asked by gate={} | cheap checks={} heals={}", g("model_turns_requested"), g("checks"), g("heals")),
            format!("gate overhead={per} ms/action"),
            format!("human: asked={} allowed={} channel={} | auto observations={} | driver restarts={}", g("human_asked"), g("human_allowed"),
                    self.human.resolve_channel(), g("auto_obs"), self.restarts.load(Ordering::SeqCst)),
            format!("reviewer={} reviews={} (allowed {}) | screenshots skipped={}", self.reviewer.backend().unwrap_or_else(|| "none".into()), g("reviews"), g("review_allowed"), g("screenshots_skipped")),

            format!("screenshot format={} bytes {} -> {} ({} ms re-encoding)", self.shot.name(), g("img_bytes_in"), g("img_bytes_out"), round_to(g("img_ms"), 1)),
            format!("0.9.7 features={:?} roles={:?}", crate::v097::active(), self.roles.configured()),
        ];
        json!({"content": [{"type": "text", "text": lines.join("\n")}], "structuredContent": st})
    }

    fn config(&self, args: &Value) -> Value {
        let mut inn = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(p) = args.get("policy").and_then(|v| v.as_str()) { inn.cfg.policy = p.into(); }
        if let Some(l) = as_f64(args.get("lambda")) { inn.cfg.lam = l; }
        if args.get("collect").is_some() { inn.cfg.collect = truthy(args.get("collect")); }
        if args.get("risk_confirm").is_some() { inn.cfg.risk_confirm = truthy(args.get("risk_confirm")); }
        let rm = s(args, "risk_mode");
        if ["auto", "reviewer", "human"].contains(&rm) { inn.cfg.risk_mode = rm.into(); }
        self.log.write(json!({"ev": "config", "policy": inn.cfg.policy, "lam": inn.cfg.lam, "collect": inn.cfg.collect}));
        let d = json!({"policy": inn.cfg.policy, "lambda": inn.cfg.lam, "collect": inn.cfg.collect, "risk_confirm": inn.cfg.risk_confirm,
                       "model": inn.cfg.model_name, "reviewer": self.reviewer.backend(), "risk_mode": inn.cfg.risk_mode});
        json!({"content": [{"type": "text", "text": d.to_string()}], "structuredContent": d})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_2026_07_28_meta_rules() {
        let m = |meta: Value| json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "x", "_meta": meta}});
        assert_eq!(classify(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})), Ok(false));
        assert_eq!(classify(&m(json!({PV_KEY: "2026-07-28", CC_KEY: {}}))), Ok(true));
        let e = classify(&m(json!({PV_KEY: "2025-06-18", CC_KEY: {}}))).unwrap_err();
        assert_eq!(e["error"]["code"], -32022);
        assert_eq!(e["error"]["data"]["supported"], json!(["2026-07-28"]));
        assert_eq!(classify(&m(json!({PV_KEY: "2026-07-28"}))).unwrap_err()["error"]["code"], -32602);
        assert_eq!(classify(&m(json!([]))).unwrap_err()["error"]["code"], -32602);
        // server/discover is always modern: no metadata is an error
        assert!(classify(&json!({"jsonrpc": "2.0", "id": 1, "method": "server/discover", "params": {}})).is_err());
        let mut msg = m(json!({PV_KEY: "2026-07-28", CC_KEY: {}, "progressToken": 3}));
        strip_modern_meta(&mut msg);
        assert_eq!(msg["params"]["_meta"], json!({"progressToken": 3}));
        let mut msg = m(json!({PV_KEY: "2026-07-28", CC_KEY: {}}));
        strip_modern_meta(&mut msg);
        assert!(msg["params"].get("_meta").is_none());
        let sh = Shared { pending_methods: HashMap::new(), driver_init: None, update_note: String::new(), update_told: false,
                          driver_version: Some("0.38.0".into()), delivery_tools: HashSet::new(), compat_warned: false,
                          tool_names: vec![], click_props: vec![] };
        let mut r = json!({"content": [], "structuredContent": {"resultType": "domain"}});
        modern_envelope(&mut r, "tools/call", &sh);
        assert_eq!(r["resultType"], "complete");
        assert_eq!(r["structuredContent"]["resultType"], "domain");
        assert_eq!(r["_meta"]["io.modelcontextprotocol/serverInfo"]["version"], VERSION);
        assert!(r.get("ttlMs").is_none());
        let mut r = json!({"tools": []});
        modern_envelope(&mut r, "tools/list", &sh);
        assert_eq!(r["ttlMs"], 0);
    }

    #[test]
    fn nearest_and_opener() {
        let fr = |x: f64, y: f64| Some(Frame { x, y, w: 40.0, h: 20.0 });
        let old = Element { index: 5, role: "menu".into(), label: "Insert".into(), frame: fr(100.0, 10.0), enabled: None, value: None };
        let a = Element { index: 7, frame: fr(101.0, 10.0), ..old.clone() };
        let b = Element { index: 90, frame: fr(600.0, 300.0), ..old.clone() };
        assert_eq!(nearest(&old, vec![b, a.clone()])[0].index, 7);
        let e = |i| Element::new(i, "menu", "Insert");
        assert_eq!(nearest(&e(5), vec![e(6), e(7)]).len(), 2);
        assert_eq!(nearest(&e(5), vec![e(6), e(70)])[0].index, 6);
        let mut ctx = ActionCtx { tool: "click".into(), target: Target { kind: "token".into(), pid: Some(1), element: Some(old), ..Default::default() }, ..Default::default() };
        assert!(fast_path(&ctx, &Feats::new()).is_some());
        ctx.target = Target { kind: "pixel".into(), pid: Some(1), ..Default::default() };
        let mut f = Feats::new();
        f.insert("obs_had_shot", 1.0);
        f.insert("acts_since_obs", 0.0);
        assert!(fast_path(&ctx, &f).is_some());
        f.insert("obs_had_shot", 0.0);
        assert!(fast_path(&ctx, &f).is_none());
    }

    #[test]
    fn occlusion_parse_and_parent_menu() {
        let e = "click: element [95] (pid 48238) has a snapshot frame at (161, 71), but that screen point is owned by menu \"Insert\" rather than the observed element; a real pointer click there would hit the wrong control.";
        assert_eq!(occlusion(e), Some((161, 71, "menu".into(), "Insert".into())));
        let e2 = "click: element [3] (pid 1) has a snapshot frame at (5, 6), but that screen point is owned by table cell \"B4\" rather than the observed element";
        assert_eq!(occlusion(e2).unwrap().2, "table cell");
        assert!(occlusion("click failed: target_occluded").is_none());
        let tree = "- [0] menu bar \"\"\n  - [1] menu \"File\" [actions=[click]]\n    - [2] menu item \"Open...\"\n    - [3] menu \"Export As\"\n      - [4] menu item \"Export Directly as PDF\"\n  - [5] menu \"Edit\"";
        assert_eq!(parent_menu(tree, 2).as_deref(), Some("File"));
        assert_eq!(parent_menu(tree, 4).as_deref(), Some("Export As"));
        assert_eq!(parent_menu(tree, 1), None);
        assert_eq!(parent_menu(tree, 99), None);
    }

    #[test]
    fn text_only_rewrites_screenshot_advice() {
        let mut r = json!({"content": [{"type": "text", "text": "Pressed key 'a'; not verified \u{2014} confirm with a screenshot."}]});
        text_only_rewrite(&mut r);
        assert!(r["content"][0]["text"].as_str().unwrap().ends_with("confirm from the tree."));
    }

    #[test]
    fn stall_hints() {
        let mut h = HashSet::new();
        let grid = "PARTIAL TREE: a container with more children than can be enumerated (e.g. a spreadsheet's cell grid) was not expanded";
        assert_eq!(read_hints(grid, false, "1:2", &mut h), vec![HINT_GRID]);
        assert!(read_hints(grid, false, "1:2", &mut h).is_empty()); // once per window
        assert_eq!(read_hints(grid, false, "1:3", &mut h), vec![HINT_GRID]);
        assert_eq!(read_hints("- [0] menu \"File\"", true, "7:8", &mut h), vec![HINT_TERMINAL]);
        assert!(read_hints("- [0] menu \"File\"", false, "9:9", &mut h).is_empty());
        assert!(streak_hint(2).is_none() && streak_hint(4).is_none());
        assert!(streak_hint(3).unwrap().contains("3 reads in a row"));
        assert!(streak_hint(6).is_some());
    }

    #[test]
    fn partial_advice() {
        let mut r = json!({"content": [{"type": "text", "text": "PARTIAL TREE: a container with more children than can be enumerated (e.g. a spreadsheet's cell grid) was not expanded after 336 node(s) (0 discovered but not visited). Every element listed is real; elements after the cut are missing. If the element you need is absent, retry with a larger timeout_ms (e.g. 5000) or narrow with query / max_depth.\n\n- frame = \"x\""}]});
        assert!(rewrite_partial_advice(&mut r));
        let t = r["content"][0]["text"].as_str().unwrap();
        assert!(t.contains("listed by the gate") && !t.contains("timeout_ms (e.g. 5000)") && t.ends_with("- frame = \"x\""), "{t}");
        assert!(!rewrite_partial_advice(&mut json!({"content": [{"type": "text", "text": "no partial"}]})));
    }

    #[test]
    fn typed_commit() {
        assert!(commits_input("ls -la\n") && commits_input("3\t4\t5") && !commits_input("hello"));
        assert!(at_prompt("x\nsandbox@h:/tmp$\n\n"));
        assert!(!at_prompt("sandbox@h:/tmp$ sleep 1; echo slow"));
        assert!(at_prompt("root@h:~#"));
    }

    #[test]
    fn occlusion_and_window_rows() {
        assert!(occluded_any("click: element [15] (pid 96634) has a snapshot frame at (151, 114), but that screen point is owned by panel (unnamed) rather than the observed element"));
        assert!(occlusion("click: element [15] (pid 96634) has a snapshot frame at (151, 114), but that screen point is owned by panel (unnamed) rather than the observed element").is_none());
        assert!(!occluded_any("foreground_unavailable: window 0x1 did not become the active"));
        let lw = "Found 3 windows:\n  window_id=1 pid=6982 \"Desktop\" 1440x900+0+0 app=Xfdesktop\n  window_id=2 pid=7 \"f.xlsx \u{2014} LibreOffice Calc\" 1440x849+0+51 app=libreoffice-calc\n  window_id=3 pid=6966 \"xfce4-panel\" 1440x27+0+0 app=Xfce4-panel";
        assert_eq!(window_rows_for_agent(lw), "window_id=2 pid=7 \"f.xlsx \u{2014} LibreOffice Calc\" 1440x849+0+51 app=libreoffice-calc");
    }

    #[test]
    fn tree_extra_helpers() {
        let md = "window_id=1 pid=2\n\n- frame = \"f.xlsx \u{2014} LibreOffice Calc\"\n  - [0] menu \"File\"";
        assert_eq!(window_title_of(md), "f.xlsx \u{2014} LibreOffice Calc");
        assert_eq!(window_title_of("- [0] menu \"File\""), "");
        let mut h = HashSet::new();
        let grid = "PARTIAL TREE: ... (e.g. a spreadsheet's cell grid) was not expanded";
        // cells already attached from AT-SPI: no "cells are not in the tree" note
        assert!(read_hints(&format!("{grid}\n[gate] Visible cells of \"Sheet1\", range A1:P32"), false, "1:2", &mut h).is_empty());
        assert!(read_hints("[gate] Terminal text (last lines ...):\n$ ls", true, "7:8", &mut h).is_empty());
        assert!(grid_not_listed(grid));
        assert!(!is_spreadsheet_app(std::process::id() as i64));
    }

    #[test]
    fn tree_delta_rows() {
        let prev = "- frame \"A\"\n  - [0] button \"OK\"\n  - [1] text \"count=0\"\n  - [2] button \"Cancel\"\n  - [3] list \"L\"\n    - [4] item \"x\"";
        let seen: HashSet<String> = prev.lines().map(String::from).collect();
        let cur_s = prev.replace("count=0", "count=1").replace("item \"x\"", "item \"y\"");
        let cur: Vec<&str> = cur_s.lines().collect();
        let idx = tree_delta(&seen, &cur, 0.5).unwrap();
        let rows: Vec<&str> = idx.iter().map(|&i| cur[i]).collect();
        assert_eq!(rows, vec!["- frame \"A\"", "  - [1] text \"count=1\"", "  - [3] list \"L\"", "    - [4] item \"y\""]);
        // a shifted index counts as changed (its token would point elsewhere)
        let shifted: Vec<&str> = vec!["- frame \"A\"", "  - [1] button \"OK\"", "  - [2] text \"count=0\"", "  - [3] button \"Cancel\"", "  - [4] list \"L\"", "    - [5] item \"x\""];
        assert!(tree_delta(&seen, &shifted, 0.5).is_none());
        assert!(tree_delta(&HashSet::new(), &cur, 0.5).is_none());
        let same: Vec<&str> = prev.lines().collect();
        assert_eq!(tree_delta(&seen, &same, 0.5), Some(vec![]));
    }

    /// Offline harness: GG_DIFFSIM=<jsonl of recorded tree reads> GG_DIFFSIM_OUT=<out> cargo test -- --ignored diffsim
    /// Feeds recorded auto-observe blocks and agent reads through tree_delta with the same seen-row bookkeeping
    /// as auto_observe/passthrough and writes the chars each auto-observe would have sent.
    #[test]
    #[ignore]
    fn diffsim() {
        let (Ok(inp), Ok(outp)) = (std::env::var("GG_DIFFSIM"), std::env::var("GG_DIFFSIM_OUT")) else { return };
        let thr: f64 = std::env::var("GG_DIFFSIM_THR").ok().and_then(|v| v.parse().ok()).unwrap_or(0.5);
        let mut seen: HashMap<String, HashSet<String>> = HashMap::new();
        let mut out = String::new();
        for line in std::fs::read_to_string(inp).unwrap().lines() {
            let r: Value = serde_json::from_str(line).unwrap();
            let key = r["key"].as_str().unwrap().to_string();
            let rows: Vec<String> = r["rows"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
            let all: Vec<&str> = rows.iter().map(|x| x.as_str()).collect();
            if r["kind"] == "read" {
                seen.insert(key, rows.iter().cloned().collect());
                continue;
            }
            let nw = r["new_window"].as_bool().unwrap_or(false);
            let prev = if nw { HashSet::new() } else { seen.get(&key).cloned().unwrap_or_default() };
            let mut lines: Vec<&str> = all.clone();
            let mut extra = 0;
            if !nw {
                if let Some(idx) = tree_delta(&prev, &all, thr) {
                    lines = idx.iter().map(|&i| all[i]).collect();
                    extra = 230; // the "only rows that changed" note
                }
            }
            lines.truncate(MAX_TREE_LINES);
            let shown: HashSet<&str> = lines.iter().copied().collect();
            seen.insert(key, all.iter().filter(|l| shown.contains(*l) || prev.contains(**l)).map(|l| l.to_string()).collect());
            let chars = r["head"].as_u64().unwrap_or(0) as usize + extra + lines.iter().map(|l| l.chars().count() + 1).sum::<usize>();
            out.push_str(&format!("{}\t{}\n", r["id"].as_str().unwrap(), chars));
        }
        std::fs::write(outp, out).unwrap();
    }

    #[test]
    fn list_apps_noise_is_hidden() {
        let txt = "Found 7 app(s)\n- systemd (pid 1)\n- kworker/0:1-events (pid 24585)\n- dbus-daemon (pid 6879)\n- LibreOffice Calc (pid 51200)\n- bash (pid 7671)\n- instabox-term (pid 7657)\n- weird-thread (pid 99)";
        let mut r = json!({"content": [{"type": "text", "text": txt}], "structuredContent": {"apps": [
            {"name": "systemd", "pid": 1}, {"name": "LibreOffice Calc", "pid": 51200}, {"name": "weird-thread", "pid": 99}]}});
        let n = filter_apps(&mut r, &|p| p == 99);
        assert_eq!(n, 4);
        let t = r["content"][0]["text"].as_str().unwrap();
        assert!(t.contains("LibreOffice Calc") && t.contains("bash") && t.contains("instabox-term") && t.starts_with("Found"));
        assert!(!t.contains("kworker") && !t.contains("systemd") && !t.contains("weird-thread"));
        assert_eq!(r["structuredContent"]["apps"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn plan_progress_line() {
        assert_eq!(plan_line(&[]), None);
        let p = vec![("a".to_string(), true), ("b".to_string(), false)];
        assert_eq!(plan_line(&p).unwrap(), "[plan] 1/2 done; next: 2. b");
        let p = vec![("a".to_string(), true)];
        assert!(plan_line(&p).unwrap().contains("all milestones done"));
    }

    #[test]
    fn short_cuts_at_sentence() {
        assert_eq!(short("abc", 10), "abc");
        let t = "First sentence here. Second sentence is long enough to be cut off somewhere.";
        assert_eq!(short(t, 40), "First sentence here.");
    }
}
