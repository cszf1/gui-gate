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
    "gate_report", "request_human", "gate_task", "run_actions"];
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

const INSTRUCTIONS_NOTE: &str = "\n\n[gui-gate] Actions go through a value gate before they run. Usually they execute at once. If a call returns a message starting with \"[gate]\", nothing was executed: do exactly what it says (re-read, re-aim with a screenshot, take the safe way it names, or ask the user and retry with gate_confirm). The gate settles doubtful and risky steps itself (rules, the user's own words, its own independent reviewer, or a person); it never asks you to approve your own action. To save time the gate omits the screenshot from get_window_state when the element tree is enough; pass include_screenshot:true when you really need pixels. Each action's result already carries a fresh read of the window: do not read it again before the next step. Several steps whose targets you can already see go in ONE run_actions call. If the user's request is known, pass it verbatim once with gate_task. When the gate refuses a risky step it tells you a safe way to go on: take it and finish the task.";

fn gate_tools() -> Vec<Value> {
    vec![
        json!({"name": "request_human",
               "description": "Ask the person at this computer. kind=approve: a yes/no on what you are about to do. kind=answer: a short free-text answer (e.g. which option they want). kind=takeover: ask them to do a step themselves (log in, solve a captcha, anything you cannot or should not do); it returns when they are done. Use only when a person is really needed.",
               "inputSchema": {"type": "object", "properties": {
                   "question": {"type": "string", "description": "What you need from them, in one or two plain sentences."},
                   "kind": {"type": "string", "enum": ["approve", "answer", "takeover"]}}, "required": ["question"]}}),
        json!({"name": "gate_task",
               "description": "Tell the gate the user's request, VERBATIM, once at the start (copy their words; do not summarise or add to them). The gate lets a risky step through without stopping you when the request explicitly asks for it (e.g. \"overwrite report.pdf\"). It can be set only once.",
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

/// A token click the driver refused because another control owns the element's point:
/// (desktop x, desktop y, owner role, owner label).
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
    pub pending_methods: HashMap<String, String>,
    pub update_note: String,
    pub update_told: bool,
    pub driver_version: Option<String>,
    pub delivery_tools: HashSet<String>,
    pub compat_warned: bool,
}

/// Everything a gated call reads and writes, under one lock.
pub struct Inner {
    pub cfg: GateConfig,
    pub state: GateState,
    pub stats: Obj,
    pub aid: u64,
    pub confirm_codes: HashMap<String, String>,
    pub want_shot: HashSet<i64>,
    pub seen_pids: HashSet<i64>,
    pub fg_sticky: bool,
    pub pixel_pids: HashSet<i64>,
    pub probe_disabled: bool,
    pub denied_effects: HashMap<String, String>,
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
    init_params: Mutex<Option<Value>>,
    client_elicit: Arc<AtomicBool>,
    client_pending: Arc<Mutex<HashMap<String, Sender<Value>>>>,
    pub restarts: AtomicU64,
    pub human: Human,
    pub reviewer: Reviewer,
    started: f64,
    toolset: String,
    batch_mode: String,
    pub shot: crate::shot::ShotCfg,
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

/// Responses from the driver to the agent's own requests: rewrite initialize and tools/list.
fn from_child(mut msg: Value, shared: &Arc<Mutex<Shared>>, out: &Arc<Out>, toolset: &str, batch_mode: &str) {
    let mid = msg.get("id").map(|v| v.to_string());
    if let (Some(mid), true) = (mid, msg.get("method").is_none()) {
        let meth = shared.lock().unwrap().pending_methods.remove(&mid);
        match meth.as_deref() {
            Some("tools/list") if msg.get("result").is_some() => augment_tools(&mut msg["result"], shared, toolset, batch_mode),
            Some("initialize") if msg.get("result").is_some() => {
                let mut sh = shared.lock().unwrap();
                let r = &mut msg["result"];
                let mut ins = pystr(r.get("instructions")) + INSTRUCTIONS_NOTE;
                if !sh.update_note.is_empty() {
                    ins.push('\n');
                    ins.push_str(&sh.update_note);
                    sh.update_told = true;
                }
                r["instructions"] = json!(ins);
                let mut si = r.get("serverInfo").cloned().filter(|v| v.is_object()).unwrap_or(json!({}));
                let dv = si.get("version").map(|v| pystr(Some(v)));
                sh.driver_version = dv.clone();
                si["name"] = json!(format!("gui-gate (cua-driver {})", dv.unwrap_or_else(|| "?".into())));
                si["version"] = json!(VERSION);
                r["serverInfo"] = si;
            }
            _ => {}
        }
    }
    out.send(&msg);
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
        if is_gated(&name) {
            props.insert("gate_confirm".into(), json!({"type": "string", "description": "Confirmation code the gate gave for a high-risk action, only after the user approved it."}));
        }
    }
    let names: Vec<String> = tools.iter().map(|t| s(t, "name").to_string()).collect();
    let miss = updates::missing_tools(&names);
    if !miss.is_empty() && !sh.compat_warned {
        sh.compat_warned = true;
        log_err(&format!("this Cua Driver lacks tools the gate needs: {}; run `gui-gate update --to {}`", miss.join(", "), updates::TESTED));
    }
    let mut all = tools;
    all.extend(gate_tools());
    if batch_mode != "off" {
        all.push(batch::run_actions_tool());
    }
    if toolset == "core" {
        all.retain(|t| CORE_TOOLS.contains(&s(t, "name")));
    }
    result["tools"] = Value::Array(all);
}

impl Gate {
    pub fn new(child_cmd: Vec<String>, mut cfg: GateConfig, log_path: Option<String>) -> Arc<Gate> {
        let out = Arc::new(Out::new());
        let shared = Arc::new(Mutex::new(Shared {
            pending_methods: HashMap::new(), update_note: String::new(), update_told: false, driver_version: None,
            delivery_tools: ["click", "double_click", "right_click", "drag", "type_text", "press_key", "hotkey", "set_value", "scroll"]
                .iter().map(|s| s.to_string()).collect(),
            compat_warned: false,
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
        let task_locked = !cfg.task.is_empty();
        let task_source = if task_locked { Some("operator".to_string()) } else { None };
        Arc::new(Gate {
            inner: Mutex::new(Inner {
                cfg, state: GateState::default(), stats: new_stats(), aid: 0, confirm_codes: HashMap::new(),
                want_shot: HashSet::new(), seen_pids: HashSet::new(), fg_sticky: false, pixel_pids: HashSet::new(),
                probe_disabled: false, denied_effects: HashMap::new(), denials_consec: 0, denials_total: 0,
                task_locked, task_source, in_batch: false, batch_ctx: Value::Null, pix_age: HashMap::new(),
                last_auto: HashMap::new(), seen_rows: HashMap::new(), recent: vec![], risk_info: Value::Null, cur_ack: false,
            }),
            shared, out, log: Arc::new(EventLog::new(log_path)), child: Mutex::new(child), child_cmd,
            init_params: Mutex::new(None), client_elicit, client_pending, restarts: AtomicU64::new(0), human,
            reviewer: rv, started: now(), toolset, batch_mode, shot: crate::shot::ShotCfg::from_env(),
        })
    }

    fn child(&self) -> Arc<ChildMCP> {
        self.child.lock().unwrap().clone()
    }

    /// The driver died: start a new one and replay the handshake (the agent never notices).
    fn respawn(&self) {
        let old = self.child();
        old.close();
        let c = spawn_child(&self.child_cmd, self.shared.clone(), self.out.clone(), self.toolset.clone(), self.batch_mode.clone());
        *self.child.lock().unwrap() = c.clone();
        let n = self.restarts.fetch_add(1, Ordering::SeqCst) + 1;
        let params = self.init_params.lock().unwrap().clone()
            .unwrap_or_else(|| json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "gui-gate", "version": VERSION}}));
        c.call("initialize", Some(params), 30.0);
        let _ = c.forward(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        self.log.write(json!({"ev": "driver_restart", "n": n}));
        log_err("driver restarted");
    }

    /// A driver tool call with input delivery handled: background when the desktop supports it, else a sticky
    /// fallback to foreground for the rest of the session.
    pub fn tool(&self, inn: &mut Inner, name: &str, args: &Value, timeout: f64) -> Value {
        let delivery_tool = self.shared.lock().unwrap().delivery_tools.contains(name);
        let mut args = args.clone();
        if delivery_tool && args.get("delivery_mode").is_none() {
            let mode = inn.cfg.delivery.clone();
            if mode == "foreground" || (mode == "auto" && inn.fg_sticky) {
                args["delivery_mode"] = json!("foreground");
            }
        }
        let mut r = self.tool_raw(name, &args, timeout);
        if inn.cfg.delivery == "auto" && is_error(&r) && delivery_tool && s(&args, "delivery_mode") != "foreground"
            && bg_unavailable().is_match(&text_of(&r)) {
            if !inn.fg_sticky {
                inn.fg_sticky = true;
                self.log.write(json!({"ev": "delivery_fallback", "tool": name, "to": "foreground"}));
            }
            inn.inc("delivery_fallbacks");
            args["delivery_mode"] = json!("foreground");
            r = self.tool_raw(name, &args, timeout);
        }
        r
    }

    fn tool_raw(&self, name: &str, args: &Value, timeout: f64) -> Value {
        if !self.child().alive() {
            self.respawn();
        }
        let r = self.child().tool(name, args, timeout);
        if is_error(&r) && (!self.child().alive() || text_of(&r) == "driver exited") {
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
            if method.as_deref() == Some("initialize") {
                let p = msg.get("params").cloned().unwrap_or(json!({}));
                self.client_elicit.store(p.get("capabilities").and_then(|c| c.get("elicitation")).is_some(), Ordering::SeqCst);
                self.log.write(json!({"ev": "client", "name": p.get("clientInfo").and_then(|c| c.get("name")),
                                      "reviewer": self.reviewer.backend()}));
                *self.init_params.lock().unwrap() = Some(p);
            }
            if method.as_deref() == Some("tools/call") && has_id {
                let me = self.clone();
                std::thread::spawn(move || me.handle_call(msg));
                continue;
            }
            if matches!(method.as_deref(), Some("tools/list") | Some("initialize")) && has_id {
                self.shared.lock().unwrap().pending_methods.insert(msg["id"].to_string(), method.clone().unwrap());
            }
            if !self.child().alive() && method.as_deref() != Some("initialize") {
                self.respawn();
            }
            if self.child().forward(&msg).is_err() {
                self.respawn();
                let _ = self.child().forward(&msg);
            }
        }
        self.child().close();
    }

    fn handle_call(self: &Arc<Self>, msg: Value) {
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        let name = s(&params, "name").to_string();
        let args = params.get("arguments").cloned().filter(|a| a.is_object()).unwrap_or(json!({}));
        let me = self.clone();
        let nm = name.clone();
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            match nm.as_str() {
                "gate_report" => me.report(),
                "gate_config" => me.config(&args),
                "request_human" => me.request_human(&args),
                "gate_task" => me.set_task(&args),
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
        let mut result = result;
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
        self.out.send(&json!({"jsonrpc": "2.0", "id": msg["id"], "result": result}));
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
        if name == "get_window_state" && text_of(&res).contains("PARTIAL TREE") {
            if let Some(p) = pid {
                inn.pixel_pids.insert(p);
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

    pub fn window_sig(&self, inn: &mut Inner, pid: i64) -> Option<String> {
        let r = self.tool(inn, "list_windows", &json!({"pid": pid}), 10.0);
        let ws = sc(&r).get("windows").and_then(|w| w.as_array()).cloned()?;
        let f = |w: &Value, k: &str| match w.get(k) { None | Some(Value::Null) => "None".to_string(), Some(v) => pystr(Some(v)) };
        let mut parts: Vec<String> = ws.iter().filter(|w| w.get("is_on_screen").map(|v| truthy(Some(v))).unwrap_or(true))
            .map(|w| format!("{}@{},{},{}x{}", f(w, "window_id"), f(w, "x"), f(w, "y"), f(w, "width"), f(w, "height"))).collect();
        parts.sort();
        Some(parts.join(";"))
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
    pub fn gated(&self, inn: &mut Inner, name: &str, mut args: Value) -> Value {
        let tg0 = Instant::now();
        let ack = false; // the acting model never approves its own step (gate_ack is accepted and ignored)
        inn.cfg.reviewer = self.reviewer.available();
        let code = args.get("gate_confirm").map(|v| pystr(Some(v))).filter(|c| !c.is_empty());
        if let Some(o) = args.as_object_mut() {
            o.remove("gate_ack");
            o.remove("gate_confirm");
        }
        let mut ctx = build_ctx(name, &args, &inn.state);
        let confirmed = code.as_ref().map(|c| inn.confirm_codes.remove(c).as_deref() == Some(ctx.signature.as_str())).unwrap_or(false);
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
        let mut dec = match policy_decision(&ctx, &f, &inn.cfg) {
            Some(d) => d,
            None => {
                let mut d = hard_rules(&ctx, &f, &inn.cfg, confirmed, false);
                if d.as_ref().map(|d| d.option == "ask_human").unwrap_or(false) && confirmed {
                    d = None;
                }
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
        // cheap options are resolved here and re-decided (each at most once)
        let (mut waited, mut checked) = (false, false);
        for _ in 0..5 {
            trail.push(dec.option.clone());
            if dec.option == "check" && checked {
                dec = Decision::new("look_light", dec.p_direct, "target still not confirmed after a fresh tree probe", "grey");
                break;
            }
            if dec.option == "wait" && waited {
                dec = Decision::new("look_light", dec.p_direct, "UI still busy after waiting", "grey");
                break;
            }
            match dec.option.as_str() {
                "check" => {
                    checked = true;
                    check_res = self.check(inn, &ctx, Some(&mut args));
                    ctx = build_ctx(name, &args, &inn.state);
                    f = features(&ctx, &inn.state, wc, wadd, check_res, None);
                    if inn.probe_disabled {
                        f["loading_cue"] = 1.0;
                    }
                    dec = choose(&ctx, &f, &inn.cfg, false, true);
                }
                "wait" => {
                    waited = true;
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
                            ctx = build_ctx(name, &args, &inn.state);
                            f = features(&ctx, &inn.state, wc, wadd, None, None);
                            dec = Decision::new("direct", p_direct(&f, &inn.cfg), format!("token re-bound to {newtok}"), "rule");
                            trail.push("healed".into());
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
            "ack": ack, "confirmed": confirmed, "risk_level": ctx.risk_level, "handoff": ctx.handoff,
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
            inn.inc("human_asked");
            let h = self.human.ask(&q, false, if opt == "ask_human" { "gui-gate: approve action" } else { "gui-gate: your turn" });
            event["human"] = h.clone();
            let ans = s(&h, "answer").to_string();
            if ans == "allow" && opt == "ask_human" {
                inn.inc("human_allowed");
                dec = Decision::new("direct", dec.p_direct, "approved by a person", "human").with_scores(&dec.scores);
                opt = "direct".into();
                event["decision"] = json!("direct");
                event["tier"] = json!("human");
            } else if ans == "allow" {
                inn.inc("human_allowed");
                event["executed"] = json!(false);
                event["gate_ms"] = json!(round_to(tg0.elapsed().as_secs_f64() * 1000.0, 1));
                self.log.write(event);
                let mut res = text_result(format!("[gate] A person did this step themselves ({}). Do not repeat it; continue from the window's current state.", dec.reason), false);
                if let Some(p) = t.pid {
                    self.auto_observe(inn, p, t.window_id, &mut res, true);
                }
                return res;
            } else if opt == "ask_help" {
                event["executed"] = json!(false);
                event["gate_ms"] = json!(round_to(tg0.elapsed().as_secs_f64() * 1000.0, 1));
                self.log.write(event);
                return text_result(format!("[gate] Not executed: {}. No person took this step. Do not send the same action again; try a clearly different way (a keyboard shortcut, another menu path, or another element).", dec.reason), true);
            } else {
                let verdict = if ans == "deny" { "declined".to_string() } else { format!("did not answer within {} s", inn.cfg.human_timeout as i64) };
                event["executed"] = json!(false);
                event["gate_ms"] = json!(round_to(tg0.elapsed().as_secs_f64() * 1000.0, 1));
                self.log.write(event);
                let txt = self.deny_text(inn, &ctx, &dec, &format!("the person {verdict}"));
                let mut extra = Obj::new();
                extra.insert("human".into(), json!(ans));
                return refusal(&dec, txt, Some(extra));
            }
        }

        if opt == "direct" {
            let t0 = Instant::now();
            let mut res = self.tool(inn, name, &args, 120.0);
            if name == "click" && is_error(&res) && !inn.in_batch && inn.cfg.policy == "gate" {
                if let Some((r2, how)) = self.heal_occlusion(inn, &ctx, &args, &text_of(&res)) {
                    event["occlusion_heal"] = json!(how);
                    res = r2;
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
            let retro_aid = inn.state.last_action().filter(|prev| prev.key == rec.key && !prev.is_error && rec.t - prev.t < 60.0).map(|p| p.aid);
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
            inn.denials_consec = 0;
            let has_xy = args.get("x").is_some() && args.get("y").is_some();
            if let Some(p) = t.pid {
                let age = if has_xy { 0 } else { inn.pix_age.get(&p).copied().unwrap_or(99) + 1 };
                inn.pix_age.insert(p, age);
                if has_xy {
                    inn.pixel_pids.insert(p);
                }
                if err && no_frame().is_match(&txt) {
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
            // also after an error: the agent gets the fresh tree (and fresh tokens) in the same turn instead of spending one on a re-read
            if AUTO_OBS_TOOLS.contains(&name) && inn.cfg.policy == "gate" && inn.cfg.auto_observe && !inn.in_batch {
                if let Some(p) = t.pid {
                    self.auto_observe(inn, p, t.window_id, &mut res, false);
                }
            }
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
                format!("[gate] Not executed: {}. A person must do this step, not you. Ask the user to do it themselves, wait until they say it is done, then re-read the window and continue.", dec.reason)
            }
            "ask_human" => {
                inn.inc("model_turns_requested");
                let c = rand_hex(3);
                inn.confirm_codes.insert(c.clone(), ctx.signature.clone());
                extra.insert("confirm_code".into(), json!(c));
                format!("[gate] Not executed: {} on {what}. Either ask the user to approve this exact action and, only after they approve, call the same tool again with the same arguments plus gate_confirm:\"{c}\"; or, without waiting for them: {alt}.", dec.reason)
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
            shot = inn.pixel_pids.contains(&pid) && inn.pix_age.get(&pid).copied().unwrap_or(99) <= 1;
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
        if partial {
            inn.pixel_pids.insert(pid);
        }
        if partial && !shot && inn.pix_age.get(&pid).copied().unwrap_or(99) <= 3 {
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
        if partial && !shot {
            busy.push_str(" The tree is partial here (grid/canvas); read with include_screenshot:true before aiming by x/y.");
        }
        let snap = pystr(scv.get("snapshot_id"));
        push_text(res, format!("[gate] Window right after the action (snapshot {snap}; tokens are {snap}:N, older tokens are stale).{busy}{delta_note}\n{note}{}{more}", lines.join("\n")));
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
            let r = rv.check_step(&inn.cfg.task, &action, &window);
            *judge = r.clone().unwrap_or(json!({"error": "reviewer did not answer"}));
            judge["sent"] = json!({"action": action, "window_tail": tail(&window, 4000)});
            if let Some(r) = r {
                if s(&r, "decision") == "allow" {
                    inn.inc("review_allowed");
                    trail.push("reviewer_allow".into());
                    return Decision::new("direct", dec.p_direct, format!("independent reviewer: {}", s(&r, "reason")), "review").with_scores(&dec.scores);
                }
                trail.push("reviewer_deny".into());
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

    /// Authorization (+ independent reviewer for medium risk). None = let it through.
    fn risk_gate(&self, inn: &mut Inner, ctx: &ActionCtx, dec: Decision, trail: &mut Vec<String>) -> Option<Decision> {
        let t = &ctx.target;
        let risk = ctx.risk.clone().unwrap_or_default();
        let dialog = self.dialog_text(inn, ctx);
        let key = authz::effect_key(&risk, &dialog);
        let hz = authz::hazard_of(&risk);
        inn.risk_info = json!({"effect": key, "hazard": hz});
        if let Some(prev) = inn.denied_effects.get(&key).cloned() {
            trail.push("denied_before".into());
            inn.risk_info["denied_before"] = json!(true);
            return Some(Decision::new("deny", 0.0, format!("{}; the same effect was already refused ({prev})", dec.reason), "rule"));
        }
        let titles = t.pid.map(|p| self.titles(inn, p)).unwrap_or_default();
        let label = t.the_label().to_string();
        let (mut ok, mut why) = authz::authorized(&risk, &inn.cfg.task, &label, &dialog, &titles);
        if ok && inn.task_source.as_deref() == Some("agent") && authz::OPERATOR_ONLY.contains(&hz) && !why.starts_with("replaces the document") {
            ok = false;
            why = "a request relayed by the agent cannot authorize this kind of action".into();
        }
        inn.risk_info["authorized"] = json!(ok);
        inn.risk_info["why"] = json!(why);
        if ok {
            trail.push("authorized".into());
            inn.inc("authorized");
            return None;
        }
        if let (Some(rv), true) = (Some(&self.reviewer).filter(|r| r.available()), dec.option == "risk_review") {
            let keys = if truthy(ctx.args.get("keys")) { ctx.args.get("keys").cloned() } else { ctx.args.get("key").cloned() };
            let action = json!({"tool": ctx.tool, "target_role": t.the_role(), "target_label": label, "rule_that_fired": risk,
                "risk_level": ctx.risk_level, "typed_text": trunc(&pystr(ctx.args.get("text")), 300), "keys": keys,
                "window_titles": titles.iter().take(4).collect::<Vec<_>>(), "dialog_text": tail(&dialog, 1500)});
            let mut action = action;
            let (recent, line) = self.review_context(inn, ctx);
            action["recent_actions"] = recent;
            if let Some(l) = line {
                action["terminal_input_line"] = json!(trunc(&l, 300));
            }
            let r = rv.review(&inn.cfg.task, &action);
            inn.risk_info["reviewer"] = r.clone().unwrap_or(Value::Null);
            inn.risk_info["reviewer_sent"] = action.clone();
            inn.inc("reviews");
            if let Some(r) = r {
                if s(&r, "decision") == "allow" {
                    trail.push("reviewer_allow".into());
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
        let key = inn.risk_info.get("effect").and_then(|e| e.as_str()).map(String::from)
            .unwrap_or_else(|| authz::effect_key(&risk, &self.dialog_text(inn, ctx)));
        inn.denied_effects.insert(key, who.to_string());
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
        inn.task_source = Some("agent".into());
        inn.cfg.task = trunc(&req, 4000);
        inn.task_locked = true;
        self.log.write(json!({"ev": "task", "source": "agent", "chars": req.chars().count()}));
        text_result("[gate] Noted.", false)
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
            return text_result("[gate] There is no direct line to a person from here. Ask your user in your reply and wait for their answer.", false);
        }
        let text = if kind != "takeover" { q.clone() } else { format!("{q}\n\nDo it in the app yourself, then press Allow when you are done.") };
        let title = match kind.as_str() { "approve" => "gui-gate: approve?", "takeover" => "gui-gate: your turn", _ => "gui-gate: question" };
        let h = self.human.ask(&text, kind == "answer", title);
        self.log.write(json!({"ev": "request_human", "kind": kind, "question": trunc(&q, 200), "human": h}));
        let ans = s(&h, "answer").to_string();
        let msg = if ans == "timeout" {
            format!("No answer from a person within {} s. Do not guess; stop and report.", timeout as i64)
        } else if kind == "takeover" {
            if ans == "allow" { "The person says the step is done. Re-read the window and continue.".into() } else { "The person declined to take over. Stop and report.".into() }
        } else if kind == "approve" {
            if ans == "allow" { "Approved.".into() } else { "Not approved. Do not do it.".into() }
        } else if ans != "deny" {
            format!("The person answered: {ans}")
        } else {
            "The person cancelled. Stop and report.".into()
        };
        json!({"content": [{"type": "text", "text": format!("[human via {}] {msg}", s(&h, "channel"))}],
               "structuredContent": {"answer": ans, "channel": h["channel"], "ms": h["ms"]}})
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
    fn nearest_and_opener() {
        let fr = |x: f64, y: f64| Some(Frame { x, y, w: 40.0, h: 20.0 });
        let old = Element { index: 5, role: "menu".into(), label: "Insert".into(), frame: fr(100.0, 10.0), enabled: None };
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
    fn short_cuts_at_sentence() {
        assert_eq!(short("abc", 10), "abc");
        let t = "First sentence here. Second sentence is long enough to be cut off somewhere.";
        assert_eq!(short(t, 40), "First sentence here.");
    }
}
