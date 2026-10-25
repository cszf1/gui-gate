//! Batch mode ("codemode inside the gate"), so every MCP client gets it, not only Pi.
//!
//! run_actions takes a small JSON program: action steps plus four control steps (find / for_each / if / read).
//! Several tool calls per model turn, only a compact result comes back, and it cannot run arbitrary code: it
//! can only call the GUI tools, each through the gate.
//!  * every GUI step goes through Gate::gated() exactly like a single call; a refusal stops the batch;
//!  * no auto-observe between steps, one observation at the end (or where the batch stopped);
//!  * blind-batch guard: stop on a driver error, an unexpected new window/dialog (a popup menu is fine), a
//!    failed `expect`, or a pixel (x/y) step after the first executed step;
//!  * before every step after the first: a fresh tree read of the step's window. While the window shows a loading
//!    cue (progress bar, "Loading", "please wait"...) or the step's target is disabled, the batch waits up to
//!    SETTLE_S; still busy / disabled -> the batch stops there and says so (a click on a disabled button would
//!    "succeed" at the driver and do nothing). A window/dialog that appeared since the last step also stops it;
//!  * at most MAX_STEPS GUI actions, MAX_NODES interpreted steps, MAX_DEPTH nesting.
use crate::features::is_gated;
use crate::server::{ids_of, Gate, Inner};
use crate::state::{Element, ObsId};
use crate::util::*;
use regex::{Regex, RegexBuilder};
use serde_json::{json, Value};
use std::collections::HashSet;

pub const MAX_STEPS: usize = 20;
const MAX_NODES: usize = 200;
const MAX_DEPTH: usize = 3;
/// How long a batch step waits for a busy window / a disabled target before the batch stops.
pub const SETTLE_S: f64 = 3.0;
const NOT_IN_BATCH: &[&str] = &["run_steps", "run_script", "kill_app"];
const MENU_ROLES: &[&str] = &["menu", "popup menu", "menu item", "check menu item", "radio menu item", "menu bar", "list",
    "list item", "combo box", "tear off menu item", "separator"];

pub fn run_actions_tool() -> Value {
    json!({
        "name": "run_actions",
        "description": "Batch mode: run several GUI steps in ONE call (single-step tools remain available). Each GUI step is checked by the gate like a single call. Steps, in order:\n  {tool, args, expect?}  a GUI action (click, type_text, press_key, hotkey, scroll, set_value, ...) or a run_command (its output comes back in the result; a failing command stops the batch). Name things that appear only after an earlier step with args.target = {label, role?}; the gate finds them on screen when the step runs. x/y only in the first GUI step.\n  {find: SELECTOR, as: \"rows\", max?: 20}  read the screen now and keep the matching elements.\n  {for_each: \"rows\", do: [steps]}  repeat steps per element; inside, args.target = \"$item\" points at it and \"$item.label\" in any string is its label.\n  {if: {exists: SELECTOR} | {not_exists: SELECTOR}, then: [steps], else?: [steps]}\n  {read: SELECTOR, as?: \"name\"}  report the matching elements' labels/values in the result.\nSELECTOR = {label} exact, {label_contains}, {label_regex} and/or {role}. The batch stops at the first refused or failed step, when a dialog opens unexpectedly (a popup menu is fine), or when an expect fails ({title_contains} or {new_window: true}). Before each step after the first the gate re-reads the window: while it is still loading or the step's target is disabled it waits up to 3 s, then stops the batch there (that step NOT run). You get what ran plus one fresh read of the window at the end. Max 20 GUI actions.",
        "inputSchema": {"type": "object", "properties": {
            "pid": {"type": "integer", "description": "Default pid for steps that omit it."},
            "window_id": {"type": "integer", "description": "Default window_id for steps that omit it."},
            "steps": {"type": "array", "description": "The program, see the tool description.", "items": {"type": "object"}}},
            "required": ["steps"]}
    })
}

/// 0.9.7 GUI_GATE_PICK adds pick_one to run_actions.
pub fn add_pick_one(t: &mut Value) {
    t["inputSchema"]["properties"]["pick_one"] = json!({"type": "boolean", "description": "true: the steps are 2-6 ALTERNATIVE candidates for one next action (e.g. a toolbar click, a shortcut, a menu path); the gate runs only the one most likely to work."});
    let d = t["description"].as_str().unwrap_or("").to_string();
    t["description"] = json!(format!("{d} With pick_one:true the steps are alternatives for ONE next action and only the best-rated one runs (use it when you are unsure which way works)."));
}

pub fn norm_label(s: &str) -> String {
    let s = s.replace(['_', '&', '\u{2026}'], "").replace("...", "");
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn menu_like(inn: &Inner, o: Option<ObsId>) -> bool {
    let Some(o) = o else { return false };
    let roles: Vec<&str> = inn.state.get(o).elements.values().map(|e| e.role.as_str()).collect();
    !roles.is_empty() && roles.iter().all(|r| MENU_ROLES.contains(r))
}

struct Batch<'a> {
    g: &'a Gate,
    pid: Option<i64>,
    wid: Option<i64>,
    stop_on_new_window: bool,
    executed: usize,
    lines: Vec<String>,
    stopped: Option<String>,
    last_pid: Option<i64>,
    last_wid: Option<i64>,
    seen: Option<HashSet<String>>,
    bid: String,
    menu_open: bool,
    /// the step whose outcome is unknown (the driver died / timed out during it): the batch stops there
    unknown_step: Option<usize>,
    /// the windows of the pid as of the end of the last step (a window not in it appeared since)
    known: Option<HashSet<String>>,
}

impl<'a> Batch<'a> {
    fn read(&self, inn: &mut Inner, pid: i64, wid: Option<i64>) -> Option<ObsId> {
        let mut a = json!({"pid": pid, "include_screenshot": false, "tree_format": "elements", "max_elements": 800});
        if let Some(w) = wid.filter(|w| *w != 0) {
            a["window_id"] = json!(w);
        }
        let r = self.g.tool(inn, "get_window_state", &a, 20.0);
        let o = inn.state.record_observation(&a, &r, None);
        inn.inc("tree_reads");
        if let Some(o) = o {
            inn.state.get_mut(o).gate_read = true;
            if let Some(p) = inn.state.get(o).pid {
                let sig = self.g.window_sig(inn, p);
                inn.state.get_mut(o).window_count = sig;
            }
        }
        o
    }

    /// Windows to search: ones opened during the batch first, then the batch's own, then the rest.
    fn windows(&self, inn: &mut Inner, pid: i64, base: Option<i64>) -> Vec<Option<i64>> {
        let r = self.g.tool(inn, "list_windows", &json!({"pid": pid}), 10.0);
        let mut ws: Vec<Value> = sc(&r).get("windows").and_then(|w| w.as_array()).cloned().unwrap_or_default()
            .into_iter().filter(|w| w.get("is_on_screen").map(|v| truthy(Some(v))).unwrap_or(true)).collect();
        ws.sort_by_key(|w| -(as_i64(w.get("z_index")).unwrap_or(0)));
        let wins: Vec<Option<i64>> = ws.iter().map(|w| as_i64(w.get("window_id"))).collect();
        let empty = HashSet::new();
        let seen = self.seen.as_ref().unwrap_or(&empty);
        let fresh: Vec<Option<i64>> = wins.iter().filter(|w| !seen.contains(&w.map(|x| x.to_string()).unwrap_or_default()) && **w != base).cloned().collect();
        let mut order = fresh.clone();
        if base.map(|b| b != 0).unwrap_or(false) {
            order.push(base);
        }
        order.extend(wins.iter().filter(|w| !fresh.contains(w) && **w != base).cloned());
        order.truncate(4);
        order
    }

    /// Elements matching a selector, read fresh now: (obs, elements, labels seen).
    fn select(&self, inn: &mut Inner, pid: i64, base: Option<i64>, sel: &Value) -> Result<(Option<ObsId>, Vec<Element>, Vec<String>), String> {
        if !sel.is_object() || !["label", "label_contains", "label_regex", "role"].iter().any(|k| sel.get(*k).is_some()) {
            return Err("a selector needs label, label_contains, label_regex or role".into());
        }
        let want = if truthy(sel.get("label")) { Some(norm_label(&pystr(sel.get("label")))) } else { None };
        let sub = pystr(sel.get("label_contains")).to_lowercase();
        let rx: Option<Regex> = if truthy(sel.get("label_regex")) {
            Some(RegexBuilder::new(&pystr(sel.get("label_regex"))).case_insensitive(true).build().map_err(|e| e.to_string())?)
        } else {
            None
        };
        let role = pystr(sel.get("role")).to_lowercase();
        let mut seen = vec![];
        for wid in self.windows(inn, pid, base) {
            let Some(o) = self.read(inn, pid, wid) else { continue };
            let mut els: Vec<Element> = inn.state.get(o).elements.values().cloned().collect();
            els.sort_by_key(|e| e.index);
            seen.extend(els.iter().filter(|e| !e.label.is_empty()).take(12).map(|e| e.label.clone()));
            let hits: Vec<Element> = els.into_iter().filter(|e| {
                want.as_ref().map(|w| norm_label(&e.label) == *w).unwrap_or(true)
                    && (sub.is_empty() || e.label.to_lowercase().contains(&sub))
                    && rx.as_ref().map(|r| r.is_match(&e.label)).unwrap_or(true)
                    && (role.is_empty() || e.role.to_lowercase().contains(&role))
            }).collect();
            if !hits.is_empty() {
                return Ok((Some(o), hits, seen));
            }
        }
        Ok((None, vec![], seen))
    }

    /// `target: {label, role?}` -> a fresh element_token read when the step runs. Err = message.
    fn resolve(&self, inn: &mut Inner, a: &mut Value) -> Result<(), String> {
        let tgt = a.as_object_mut().unwrap().remove("target").unwrap_or(Value::Null);
        if tgt.get("_token").is_some() {
            a["element_token"] = tgt["_token"].clone();
            if truthy(tgt.get("_wid")) {
                a["window_id"] = tgt["_wid"].clone();
            }
            let o = a.as_object_mut().unwrap();
            o.remove("x");
            o.remove("y");
            return Ok(());
        }
        if !tgt.is_object() || !truthy(tgt.get("label")) {
            return Err("target needs {\"label\": ..., \"role\": optional}".into());
        }
        let Some(pid) = as_i64(a.get("pid")) else { return Err("target needs a pid".into()) };
        let label = pystr(tgt.get("label"));
        let mut sel = json!({"label": label});
        if truthy(tgt.get("role")) {
            sel["role"] = tgt["role"].clone();
        }
        let (o, hits, seen) = self.select(inn, pid, as_i64(a.get("window_id")), &sel)?;
        if hits.len() == 1 {
            let ob = inn.state.get(o.unwrap());
            a["element_token"] = json!(format!("{}:{}", ob.snapshot_id, hits[0].index));
            let w = ob.window_id.or(as_i64(a.get("window_id")));
            a["window_id"] = json!(w);
            let ao = a.as_object_mut().unwrap();
            ao.remove("x");
            ao.remove("y");
            return Ok(());
        }
        if hits.len() > 1 {
            return Err(format!("\"{label}\" matches {} elements; add a role or use an element_token", hits.len()));
        }
        Err(format!("no element named \"{label}\" is on screen (seen: {})", seen.iter().take(10).cloned().collect::<Vec<_>>().join(", ")))
    }

    /// Before step `n` (not the first): is the window still loading, is the target disabled, did a dialog appear?
    /// Waits up to SETTLE_S for a busy window / disabled target. Ok(note) = go on (the target is re-bound to the
    /// fresh read); Err(why) = do not run this step.
    fn preflight(&mut self, inn: &mut Inner, n: usize, tool: &str, a: &mut Value) -> Result<Option<String>, String> {
        let Some(pid) = as_i64(a.get("pid")) else { return Ok(None) };
        if self.stop_on_new_window && !self.menu_open {
            if let Some(known) = self.known.clone() {
                let now_ids = ids_of(self.g.window_sig(inn, pid).as_deref());
                let mut new: Vec<String> = now_ids.difference(&known).cloned().collect();
                new.sort();
                if let Some(nw) = new.last().cloned() {
                    let o = nw.parse::<i64>().ok().and_then(|w| self.read(inn, pid, Some(w)));
                    if !menu_like(inn, o) {
                        if let Ok(w) = nw.parse::<i64>() {
                            self.last_wid = Some(w);
                        }
                        return Err(format!("a new window/dialog (window_id={nw}) appeared after step {} and before this one; read it before going on", n - 1));
                    }
                }
            }
        }
        // the step's target as the batch names it (label + role from the read it was taken from)
        let ctx = crate::features::build_ctx(tool, a, &inn.state);
        let tgt = ctx.target.element.clone().filter(|e| !e.label.trim().is_empty() && ctx.target.is_elem());
        let tidx = ctx.target.index;
        let by_index = a.get("element_index").is_some() && a.get("element_token").is_none();
        let wid = as_i64(a.get("window_id")).or(ctx.target.window_id).or(self.last_wid).or(self.wid).filter(|w| *w != 0);
        let t0 = now();
        let mut waited = false;
        loop {
            let Some(o) = self.read(inn, pid, wid) else { return Ok(None) }; // no read: the gate decides as for any step
            let ob = inn.state.get(o);
            let busy = ob.elements.values().find(|e| e.role.to_lowercase().contains("progress bar")
                || (!e.label.trim().is_empty() && crate::state::loading_re().is_match(&e.label)))
                .map(|e| if e.label.trim().is_empty() { e.role.clone() } else { trunc(e.label.trim(), 60).to_string() });
            let mut disabled = None;
            let mut fresh: Option<(String, i64, Option<i64>)> = None;
            let mut lost = false;
            if let Some(t) = &tgt {
                let want = norm_label(&t.label);
                let same = |e: &Element| norm_label(&e.label) == want && e.role.eq_ignore_ascii_case(&t.role);
                let hit = tidx.and_then(|i| ob.elements.get(&i)).filter(|e| same(e)).cloned().or_else(|| {
                    let v: Vec<&Element> = ob.elements.values().filter(|e| same(e)).collect();
                    if v.len() == 1 { Some(v[0].clone()) } else { None }
                });
                match hit {
                    Some(e) => {
                        if e.enabled == Some(false) {
                            disabled = Some(trunc(&e.label, 60).to_string());
                        }
                        fresh = Some((ob.snapshot_id.clone(), e.index, ob.window_id));
                    }
                    None => lost = by_index, // an index means "of the newest read": it would now point elsewhere
                }
            }
            if lost {
                return Err(format!("its target \"{}\" is not in the window any more at a unique place; re-read the window and aim again",
                                   tgt.as_ref().map(|t| trunc(&t.label, 60).to_string()).unwrap_or_default()));
            }
            if busy.is_none() && disabled.is_none() {
                if let Some((snap, idx, w)) = fresh {
                    if a.get("element_token").is_some() {
                        a["element_token"] = json!(format!("{snap}:{idx}"));
                        if let Some(w) = w {
                            a["window_id"] = json!(w);
                        }
                    } else if a.get("element_index").is_some() {
                        a["element_index"] = json!(idx);
                    }
                }
                return Ok(if waited { Some(format!("waited {:.1} s for the window to settle", now() - t0)) } else { None });
            }
            if now() - t0 >= SETTLE_S {
                inn.inc("batch_settle_stops");
                return Err(match (busy, disabled) {
                    (Some(b), Some(d)) => format!("the window is still busy (\"{b}\") and its target \"{d}\" is disabled after waiting {SETTLE_S:.0} s"),
                    (Some(b), None) => format!("the window is still busy (\"{b}\") after waiting {SETTLE_S:.0} s"),
                    (None, Some(d)) => format!("its target \"{d}\" is disabled (greyed out) after waiting {SETTLE_S:.0} s; a click would do nothing"),
                    (None, None) => unreachable!(),
                });
            }
            waited = true;
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
    }

    fn fill(&self, args: &Value) -> Value {
        let mut a = if args.is_object() { args.clone() } else { json!({}) };
        if let (Some(p), None) = (self.pid, a.get("pid")) {
            a["pid"] = json!(p);
        }
        if let Some(w) = self.wid {
            if a.get("window_id").is_none() && a.get("element_token").is_none() {
                a["window_id"] = json!(w);
            }
        }
        a
    }

    fn stop(&mut self, why: String) -> Value {
        self.stopped = Some(why.clone());
        text_result(why, true)
    }

    fn step(&mut self, inn: &mut Inner, tool: &str, args: &Value, expect: Option<&Value>, last: bool) -> Value {
        let g = self.g;
        let n = self.executed + 1;
        if NOT_IN_BATCH.contains(&tool) || !is_gated(tool) {
            return self.stop(format!("step {n}: `{tool}` cannot be used inside a batch"));
        }
        if self.executed >= MAX_STEPS {
            return self.stop(format!("the batch limit of {MAX_STEPS} GUI actions was reached"));
        }
        let mut a = self.fill(args);
        let pid = as_i64(a.get("pid"));
        if self.seen.is_none() {
            if let Some(p) = pid {
                self.seen = Some(ids_of(g.window_sig(inn, p).as_deref())); // windows that were there before the batch
                self.known = self.seen.clone();
            }
        }
        if a.get("target").is_some() {
            if let Err(e) = self.resolve(inn, &mut a) {
                return self.stop(format!("step {n}: {e}"));
            }
        } else if self.menu_open && a.get("element_token").is_some() {
            return self.stop(format!("step {n}: a menu opened in the previous step, and an element_token from before the batch cannot point into it; name the item with target:{{label}}"));
        }
        if self.executed > 0 && a.get("x").is_some() && a.get("y").is_some() && a.get("element_token").is_none() {
            return self.stop(format!("step {n}: pixel x/y after the batch already changed the screen; those coordinates are stale. Re-read the window and aim again"));
        }
        let mut settled = None;
        if self.executed > 0 {
            match self.preflight(inn, n, tool, &mut a) {
                Ok(note) => settled = note,
                Err(why) => {
                    self.lines.push(format!("{n}. {tool}: NOT RUN - {why}"));
                    return self.stop(format!("step {n} ({tool}) was not run: {why}"));
                }
            }
        }
        let before = match (pid, self.stop_on_new_window) { (Some(p), true) => g.window_sig(inn, p), _ => None };
        inn.batch_ctx = json!({"bid": self.bid, "step": n});
        let res = g.gated(inn, tool, a.clone());
        inn.batch_ctx = Value::Null;
        let refused = sc(&res).get("gate").and_then(|x| x.get("executed")) == Some(&Value::Bool(false));
        if crate::server::outcome_unknown(&res) {
            self.unknown_step = Some(n);
            self.lines.push(format!("{n}. {tool}: OUTCOME UNKNOWN - {}", trunc(text_of(&res).trim().lines().next().unwrap_or(""), 200)));
            self.stopped = Some(format!("step {n} ({tool}) may or may not have happened (the driver did not answer); read the window before anything else"));
            return res;
        }
        // a command's output is its result: keep it (capped), not only the status line
        let first = if tool == "run_command" { trunc(text_of(&res).trim(), 2500) } else { trunc(text_of(&res).trim().lines().next().unwrap_or(""), 200) };
        if is_error(&res) {
            self.lines.push(format!("{n}. {tool}: {} - {first}", if refused { "REFUSED" } else { "FAILED" }));
            self.stopped = Some(format!("step {n} ({tool}) was {}", if refused { "refused by the gate" } else { "not completed" }));
            return res;
        }
        self.executed += 1;
        if pid.is_some() {
            self.last_pid = pid;
        }
        if truthy(a.get("window_id")) {
            self.last_wid = as_i64(a.get("window_id"));
        }
        self.lines.push(format!("{n}. {tool}: ok - {first}{}", settled.map(|s| format!(" ({s})")).unwrap_or_default()));
        let empty = json!({});
        let expect = expect.filter(|e| e.is_object()).unwrap_or(&empty);
        if let (Some(p), Some(before)) = (pid, before.as_ref()) {
            let after = g.window_sig(inn, p);
            self.known = Some(ids_of(after.as_deref()));
            let bi = ids_of(Some(before));
            let mut new: Vec<String> = ids_of(after.as_deref()).into_iter().filter(|x| !bi.contains(x)).collect();
            new.sort();
            self.menu_open = false;
            if !new.is_empty() && !truthy(expect.get("new_window")) && !last {
                let nw = new.last().unwrap().clone();
                let o = nw.parse::<i64>().ok().and_then(|w| self.read(inn, p, Some(w)));
                if menu_like(inn, o) {
                    self.menu_open = true; // a menu or dropdown opened: expected, the next step names its item
                    return res;
                }
                self.stopped = Some(format!("step {n} opened a new window/dialog (window_id={nw}) that the batch did not expect; read it before going on"));
                if let Ok(w) = nw.parse::<i64>() {
                    self.last_wid = Some(w);
                }
                return res;
            }
            if truthy(expect.get("new_window")) && new.is_empty() {
                self.stopped = Some(format!("step {n} was expected to open a window/dialog, but none appeared"));
                return res;
            }
        }
        let tc = pystr(expect.get("title_contains"));
        if !tc.is_empty() {
            if let Some(p) = pid {
                let titles = g.titles(inn, p);
                if !titles.iter().any(|t| t.to_lowercase().contains(&tc.to_lowercase())) {
                    let shown: Vec<String> = titles.iter().take(4).map(|t| format!("'{t}'")).collect();
                    self.stopped = Some(format!("after step {n} no window title contains \"{tc}\" (titles: [{}])", shown.join(", ")));
                }
            }
        }
        res
    }
}

fn line_of(text: &str, idx: i64) -> String {
    Regex::new(&format!(r"\[{idx}\] ([^\n]{{0,200}})")).ok().and_then(|r| r.captures(text).map(|c| c[1].trim().to_string())).unwrap_or_default()
}

/// $item -> the element (as a target), "$item.label" inside strings -> its label.
fn subst(v: &Value, item: Option<&Value>) -> Value {
    let Some(item) = item else { return v.clone() };
    match v {
        Value::String(s) if s == "$item" => item.clone(),
        Value::String(s) => Value::String(s.replace("$item.label", &pystr(item.get("label")))),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), subst(x, Some(item)))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(|x| subst(x, Some(item))).collect()),
        other => other.clone(),
    }
}

struct Program<'a> {
    b: Batch<'a>,
    vars: std::collections::HashMap<String, Vec<Value>>,
    nodes: usize,
    out: Vec<String>,
}

impl<'a> Program<'a> {
    fn sel(&mut self, inn: &mut Inner, sel: &Value, item: Option<&Value>) -> Result<(Option<ObsId>, Vec<Element>), String> {
        let pid = self.b.pid.or(self.b.last_pid).ok_or("find/if/read need a pid (give one at the top level)")?;
        if self.b.seen.is_none() {
            self.b.seen = Some(ids_of(self.b.g.window_sig(inn, pid).as_deref()));
        }
        let base = self.b.last_wid.or(self.b.wid);
        let (o, hits, _) = self.b.select(inn, pid, base, &subst(sel, item))?;
        Ok((o, hits))
    }

    fn run(&mut self, inn: &mut Inner, steps: &[Value], depth: usize, item: Option<&Value>, top: bool) {
        if depth > MAX_DEPTH {
            self.b.stopped = Some(format!("nesting deeper than {MAX_DEPTH}"));
            return;
        }
        for (i, st) in steps.iter().enumerate() {
            if self.b.stopped.is_some() {
                return;
            }
            self.nodes += 1;
            if self.nodes > MAX_NODES {
                self.b.stopped = Some(format!("more than {MAX_NODES} steps were interpreted (loop too long)"));
                return;
            }
            if !st.is_object() {
                self.b.stopped = Some(format!("a step is not an object: {}", trunc(&st.to_string(), 60)));
                return;
            }
            if let Err(e) = self.one(inn, st, depth, item, top && i == steps.len() - 1) {
                self.b.stopped = Some(format!("bad step {}: {e}", trunc(&st.to_string(), 80)));
                return;
            }
        }
    }

    fn one(&mut self, inn: &mut Inner, st: &Value, depth: usize, item: Option<&Value>, last: bool) -> Result<(), String> {
        if let Some(sel) = st.get("find") {
            let (o, hits) = self.sel(inn, sel, item)?;
            let max = as_i64(st.get("max")).unwrap_or(20).max(0) as usize;
            let hits: Vec<Element> = hits.into_iter().take(max).collect();
            let name = { let n = pystr(st.get("as")); if n.is_empty() { "found".to_string() } else { n } };
            let (snap, wid) = o.map(|o| (inn.state.get(o).snapshot_id.clone(), inn.state.get(o).window_id)).unwrap_or_default();
            let items: Vec<Value> = hits.iter().map(|e| json!({"_token": format!("{snap}:{}", e.index), "_wid": wid, "label": e.label, "role": e.role})).collect();
            let preview = if hits.is_empty() { String::new() } else {
                format!(" ({})", hits.iter().take(5).map(|e| e.label.clone()).collect::<Vec<_>>().join(", "))
            };
            self.b.lines.push(format!("find -> {name}: {} match(es){preview}", hits.len()));
            self.vars.insert(name, items);
        } else if let Some(var) = st.get("for_each") {
            let var = pystr(Some(var));
            let Some(items) = self.vars.get(&var).cloned() else {
                self.b.stopped = Some(format!("for_each: no list named \"{var}\" (use find ... as first)"));
                return Ok(());
            };
            let body: Vec<Value> = st.get("do").and_then(|d| d.as_array()).cloned().unwrap_or_default();
            for it in items {
                self.run(inn, &body, depth + 1, Some(&it), false);
                if self.b.stopped.is_some() {
                    return Ok(());
                }
            }
        } else if let Some(cond) = st.get("if") {
            let key = if cond.get("exists").is_some() { "exists" } else if cond.get("not_exists").is_some() { "not_exists" } else {
                self.b.stopped = Some("if needs {exists: selector} or {not_exists: selector}".into());
                return Ok(());
            };
            let (_, hits) = self.sel(inn, &cond[key], item)?;
            let ok = if key == "exists" { !hits.is_empty() } else { hits.is_empty() };
            self.b.lines.push(format!("if {key} -> {}", if ok { "then" } else { "else" }));
            let branch: Vec<Value> = st.get(if ok { "then" } else { "else" }).and_then(|d| d.as_array()).cloned().unwrap_or_default();
            self.run(inn, &branch, depth + 1, item, false);
        } else if let Some(sel) = st.get("read") {
            let (o, hits) = self.sel(inn, sel, item)?;
            let text = o.map(|o| inn.state.get(o).text.clone()).unwrap_or_default();
            let vals: Vec<String> = hits.iter().take(20).map(|e| {
                let l = line_of(&text, e.index);
                if l.is_empty() { format!("{} \"{}\"", e.role, e.label) } else { l }
            }).collect();
            let name = { let n = pystr(st.get("as")); if n.is_empty() { "read".to_string() } else { n } };
            self.out.push(format!("{name}: {}", if vals.is_empty() { "(nothing matched)".into() } else { vals.join(" | ") }));
        } else {
            let (tool, sargs) = match st.get("tool") {
                Some(t) if truthy(Some(t)) => (pystr(Some(t)), st.get("args").cloned().filter(|a| a.is_object()).unwrap_or(json!({}))),
                _ => {
                    // also accept {"click": {...}}
                    let k = st.as_object().unwrap().keys().find(|k| *k != "expect").cloned().unwrap_or_default();
                    let a = st.get(&k).cloned().filter(|a| a.is_object()).unwrap_or(json!({}));
                    (k, a)
                }
            };
            self.b.step(inn, &tool, &subst(&sargs, item), st.get("expect"), last);
        }
        Ok(())
    }
}

/// 0.9.7 GUI_GATE_PICK (GTA1 step-level test-time scaling, Agent S3 bBoN at step level): the steps are alternative
/// candidates for ONE next action. The gate ranks them by the predictor's P(direct succeeds), minus risk and
/// loop-breaker penalties; the judge role (when configured) picks among the safe ones; the chosen candidate then runs
/// through the normal gate (risk path, reviewer, ask-human unchanged). Trajectory-level best-of-N is not done: a real
/// desktop cannot be forked or reset, and every rollout's side effects would really happen.
pub fn pick_one(g: &Gate, inn: &mut Inner, args: &Value, steps: &[Value]) -> Value {
    let pid = as_i64(args.get("pid"));
    let wid = as_i64(args.get("window_id"));
    let mut cands: Vec<(usize, String, Value, f64, f64, String)> = vec![]; // (n, tool, args, p, score, risk)
    for (i, st) in steps.iter().take(6).enumerate() {
        let tool = s(st, "tool").to_string();
        if !is_gated(&tool) || NOT_IN_BATCH.contains(&tool.as_str()) {
            return text_result(format!("[gate] pick_one: candidate {} must be a single action step {{tool, args}} (got {:?})", i + 1, tool), true);
        }
        let mut a = st.get("args").cloned().filter(|a| a.is_object()).unwrap_or(json!({}));
        if a.get("pid").is_none() { if let Some(p) = pid { a["pid"] = json!(p); } }
        if a.get("window_id").is_none() { if let Some(w) = wid { a["window_id"] = json!(w); } }
        let ctx = crate::features::build_ctx(&tool, &a, &inn.state);
        let f = crate::features::features(&ctx, &inn.state, None, None, None, None);
        let p = crate::decide::p_direct(&f, &inn.cfg);
        let risk = ctx.risk_level.clone().unwrap_or_default();
        let mut sc = p - match risk.as_str() { "high" => 1.0, "medium" => 0.3, _ => 0.0 };
        if inn.lb.blocks(&ctx.signature) { sc -= 1.0; }
        if inn.lb.level >= 1 && crate::loopbreak::mode_of(&tool, &ctx.target.kind) == inn.lb.dominant_mode() { sc -= 0.15; }
        cands.push((i + 1, tool, a, p, sc, risk));
    }
    if cands.len() < 2 {
        return text_result("[gate] pick_one needs 2-6 candidate steps", true);
    }
    let mut order: Vec<usize> = (0..cands.len()).collect();
    order.sort_by(|a, b| cands[*b].4.partial_cmp(&cands[*a].4).unwrap().then(a.cmp(b)));
    let mut chosen = order[0];
    let mut why = "highest estimate".to_string();
    if let Some(role) = g.roles.get("judge") {
        let window = inn.state.latest_for(pid, wid).map(|o| trunc(&inn.state.get(o).text, 3000)).unwrap_or_default();
        let list: Vec<String> = cands.iter().map(|c| format!("{}. {} {} (estimated chance to work {:.0}%{})", c.0, c.1, trunc(&c.2.to_string(), 200), c.3 * 100.0, if c.5.is_empty() { String::new() } else { format!(", {} risk", c.5) })).collect();
        let prompt = format!("TASK: {}\n\nWINDOW:\n{window}\n\nRECENT STEPS:\n{}\n\nCANDIDATES:\n{}", crate::server::Gate::ctx_task(inn),
                             inn.recent.iter().rev().take(6).rev().map(|r| r.to_string()).collect::<Vec<_>>().join("\n"), list.join("\n"));
        let (fin, _, calls, err) = crate::roles::probe_agent(role, crate::roles::JUDGE_PICK_SYSTEM, &prompt, &|| String::new(), false);
        g.log.write(json!({"ev": "role", "role": "judge", "use": "pick", "calls": calls, "pick": fin.as_ref().and_then(|f| f.get("pick")).cloned(), "error": err}));
        if let Some(n) = fin.as_ref().and_then(|f| as_i64(f.get("pick"))) {
            if let Some(k) = cands.iter().position(|c| c.0 as i64 == n) {
                // the judge cannot prefer a risky candidate over a safe one
                if cands[k].5.is_empty() || cands.iter().all(|c| !c.5.is_empty()) {
                    chosen = k;
                    why = format!("judge ({}): {}", role.backend(), pystr(fin.as_ref().unwrap().get("reason")));
                }
            }
        }
    }
    inn.inc("picks");
    let ranking: Vec<String> = order.iter().map(|k| { let c = &cands[*k]; format!("{}. {} P={:.2}{}", c.0, c.1, c.3, if c.5.is_empty() { String::new() } else { format!(" ({} risk)", c.5) }) }).collect();
    g.log.write(json!({"ev": "pick", "n": cands.len(), "chosen": cands[chosen].0, "why": why}));
    let (n, tool, a) = (cands[chosen].0, cands[chosen].1.clone(), cands[chosen].2.clone());
    let mut r = g.gated(inn, &tool, a);
    push_text(&mut r, format!("[gate] pick_one: ran candidate {n} ({why}); ranking: {}.", ranking.join("; ")));
    r
}

pub fn run_actions(g: &Gate, inn: &mut Inner, args: &Value) -> Value {
    let steps: Vec<Value> = match args.get("steps").and_then(|s| s.as_array()) {
        Some(s) if !s.is_empty() => s.clone(),
        _ => return text_result("run_actions needs a non-empty `steps` list", true),
    };
    if truthy(args.get("pick_one")) && crate::v097::on("PICK") {
        return pick_one(g, inn, args, &steps);
    }
    let pid = as_i64(args.get("pid"));
    let wid = as_i64(args.get("window_id"));
    let b = Batch {
        g, pid, wid, stop_on_new_window: args.get("stop_on_new_window") != Some(&Value::Bool(false)), executed: 0, lines: vec![],
        stopped: None, last_pid: pid, last_wid: wid, seen: None, bid: format!("b{}", (now() * 1000.0) as u64 % 10_000_000), menu_open: false, unknown_step: None, known: None,
    };
    let mut prog = Program { b, vars: Default::default(), nodes: 0, out: vec![] };
    inn.in_batch = true;
    crate::human::set_in_batch(true);
    prog.run(inn, &steps, 0, None, true);
    inn.in_batch = false;
    crate::human::set_in_batch(false);
    let b = &prog.b;
    let head = format!("[gate] run_actions: {} GUI action(s) ran.{}", b.executed,
        match &b.stopped { Some(s) => format!(" STOPPED: {s}. Steps after it did NOT run."), None => " All done.".into() });
    let mut body = b.lines.clone();
    body.extend(prog.out.iter().cloned());
    let mut out = json!({"content": [{"type": "text", "text": format!("{head}\n{}", body.join("\n"))}],
                         "structuredContent": {"batch": {"executed": b.executed, "stopped": b.stopped, "read": prog.out, "unknown_step": b.unknown_step}}});
    if b.stopped.is_some() {
        out["isError"] = json!(true);
    }
    // one observation at the end
    if b.executed > 0 {
        if let Some(p) = b.last_pid {
            g.auto_observe(inn, p, b.last_wid, &mut out, true);
        }
    }
    inn.inc("batches");
    inn.add("batch_actions", b.executed as f64);
    let tail: Vec<String> = b.lines.iter().rev().take(MAX_STEPS).rev().cloned().collect();
    g.log.write(json!({"ev": "batch", "bid": b.bid, "executed": b.executed, "stopped": b.stopped, "lines": tail}));
    out
}
