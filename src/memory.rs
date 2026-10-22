//! 0.9.7 experience memory (HIPPO / muscle-mem cross-run reuse; UiPath / OS-Symphony memories are per task and live
//! in set_plan facts). GUI_GATE_MEMORY=FILE (JSONL): one entry per successful earlier run {task, app, steps:[...],
//! source}. Built offline from gate logs of runs the bench scored as successful:
//!     gui-gate memory add --task-file T.txt --log RUN.jsonl [--out FILE] [--source NAME]
//!     gui-gate memory show [--file FILE] QUERY
//! At run time the briefing and `inspect kind=memory` offer the closest entries (word overlap) as hints, never as
//! orders. For a benchmark, build the memory from a disjoint task set: entries from the same task are an answer key.
use crate::util::*;
use serde_json::{json, Value};

pub fn path() -> Option<String> {
    env("GUI_GATE_MEMORY").filter(|p| !p.trim().is_empty() && p != "0")
}

fn words(s: &str) -> std::collections::HashSet<String> {
    const STOP: &[&str] = &["the", "and", "for", "with", "this", "that", "from", "into", "you", "can", "please", "help", "want", "need", "file", "make", "all", "are", "then", "have", "will", "my", "me", "it", "in", "on", "of", "to", "a", "an", "is", "be", "as", "at", "by", "or"];
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() > 2 && !STOP.contains(w)).map(String::from).collect()
}

pub fn load(p: &str) -> Vec<Value> {
    std::fs::read_to_string(p).unwrap_or_default().lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(|v| v.get("task").is_some()).collect()
}

/// (score, entry) best first; score = Jaccard overlap of content words.
pub fn lookup(entries: &[Value], query: &str, k: usize) -> Vec<(f64, Value)> {
    let q = words(query);
    let mut out: Vec<(f64, Value)> = entries.iter().map(|e| {
        let w = words(&pystr(e.get("task")));
        let inter = q.intersection(&w).count() as f64;
        let uni = q.union(&w).count().max(1) as f64;
        (inter / uni, e.clone())
    }).filter(|(sc, _)| *sc >= 0.15).collect();
    out.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    out.truncate(k);
    out
}

pub fn format_entry(sc: f64, e: &Value) -> String {
    let steps: Vec<String> = e.get("steps").and_then(|s| s.as_array()).map(|a| a.iter().take(25).map(|x| pystr(Some(x))).collect()).unwrap_or_default();
    format!("- similar task ({:.0}% word overlap): \"{}\"\n  steps that worked: {}", sc * 100.0, trunc(&pystr(e.get("task")), 160), steps.join(" ; "))
}

pub fn lookup_text(query: &str, k: usize) -> String {
    let Some(p) = path() else { return "[inspect memory] no experience memory is configured (GUI_GATE_MEMORY).".into() };
    let hits = lookup(&load(&p), query, k);
    if hits.is_empty() {
        return "[inspect memory] no earlier successful run of a similar task.".into();
    }
    let mut s = String::from("[inspect memory] earlier successful runs of similar tasks (hints: the current screen may differ, check before you repeat a step):\n");
    for (sc, e) in &hits {
        s.push_str(&format_entry(*sc, e));
        s.push('\n');
    }
    s
}

/// Compact step list from a gate log: executed actions that did not fail, in order, commands and keys included.
pub fn steps_from_log(path: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for l in std::fs::read_to_string(path).unwrap_or_default().lines() {
        let Ok(e) = serde_json::from_str::<Value>(l) else { continue };
        match s(&e, "ev") {
            "action" if truthy(e.get("executed")) && !truthy(e.get("is_error")) => {
                let tool = s(&e, "tool");
                let tg = e.get("target").cloned().unwrap_or(json!({}));
                let label = s(&tg, "label");
                let keys = s(&e, "sig").rsplit('|').next().unwrap_or("").to_string();
                let step = match tool {
                    "click" | "double_click" | "right_click" if !label.is_empty() => format!("{tool} {} \"{}\"", s(&tg, "role"), trunc(label, 40)),
                    "press_key" | "hotkey" => format!("{tool} {}", trunc(&keys, 40)),
                    "type_text" => format!("type_text {}", trunc(&keys, 50)),
                    "run_command" => "run_command".to_string(),
                    t => t.to_string(),
                };
                if out.last() != Some(&step) {
                    out.push(step);
                }
            }
            "command" => {
                if let Some(last) = out.last_mut().filter(|l| *l == "run_command") {
                    *last = format!("run_command (cwd {})", s(&e, "cwd"));
                }
            }
            _ => {}
        }
    }
    out.truncate(40);
    out
}

pub fn main_cli(argv: &[String]) -> i32 {
    let get = |k: &str| argv.iter().position(|a| a == k).and_then(|i| argv.get(i + 1)).cloned();
    match argv.first().map(|s| s.as_str()) {
        Some("add") => {
            let task = get("--task").or_else(|| get("--task-file").and_then(|f| std::fs::read_to_string(f).ok())).unwrap_or_default();
            let Some(log) = get("--log") else { eprintln!("usage: gui-gate memory add --task TEXT|--task-file F --log GATE_LOG.jsonl [--out FILE] [--source NAME]"); return 2 };
            let out = get("--out").or_else(path).unwrap_or_else(|| format!("{}/.config/gui-gate/memory.jsonl", home()));
            let steps = steps_from_log(&log);
            if steps.is_empty() || task.trim().is_empty() {
                eprintln!("nothing to add (no task text or no executed steps in {log})");
                return 1;
            }
            let e = json!({"task": task.trim(), "steps": steps, "source": get("--source").unwrap_or_else(|| crate::sidefx::basename(&log))});
            if let Some(d) = std::path::Path::new(&out).parent() { let _ = std::fs::create_dir_all(d); }
            use std::io::Write;
            let ok = std::fs::OpenOptions::new().create(true).append(true).open(&out).and_then(|mut f| writeln!(f, "{e}")).is_ok();
            println!("{} {} steps -> {out}", if ok { "added" } else { "FAILED" }, e["steps"].as_array().map(|a| a.len()).unwrap_or(0));
            if ok { 0 } else { 1 }
        }
        Some("show") => {
            let file = get("--file").or_else(path).unwrap_or_else(|| format!("{}/.config/gui-gate/memory.jsonl", home()));
            let q: Vec<String> = argv[1..].iter().filter(|a| !a.starts_with("--") && Some((*a).clone()) != get("--file")).cloned().collect();
            for (sc, e) in lookup(&load(&file), &q.join(" "), 5) {
                println!("{}", format_entry(sc, &e));
            }
            0
        }
        _ => {
            println!("usage: gui-gate memory add --task TEXT|--task-file F --log GATE_LOG.jsonl [--out FILE] [--source NAME]\n       gui-gate memory show [--file FILE] QUERY");
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_ranks_similar_tasks() {
        let es = vec![json!({"task": "Change the font of the Chrome browser to Times New Roman", "steps": ["hotkey ctrl+l", "type_text chrome://settings/fonts"]}),
                      json!({"task": "Sort the spreadsheet by date in LibreOffice Calc", "steps": ["click menu \"Data\""]})];
        let h = lookup(&es, "Please set the Chrome font size to 20", 3);
        assert_eq!(h.len(), 1);
        assert!(format_entry(h[0].0, &h[0].1).contains("chrome://settings/fonts"));
        assert!(lookup(&es, "rename a folder in the terminal", 3).is_empty());
    }

    #[test]
    fn steps_from_a_gate_log() {
        let d = std::env::temp_dir().join(format!("gg-mem-{}.jsonl", std::process::id()));
        std::fs::write(&d, [
            json!({"ev": "action", "executed": true, "tool": "click", "target": {"role": "menu", "label": "Format"}, "sig": "click|x"}).to_string(),
            json!({"ev": "action", "executed": true, "is_error": true, "tool": "click", "target": {"role": "menu", "label": "Bad"}, "sig": "click|y"}).to_string(),
            json!({"ev": "action", "executed": true, "tool": "hotkey", "target": {}, "sig": "hotkey|ctrl+s"}).to_string(),
            json!({"ev": "action", "executed": false, "tool": "click", "target": {"label": "Refused"}}).to_string(),
        ].join("\n")).unwrap();
        assert_eq!(steps_from_log(d.to_str().unwrap()), vec!["click menu \"Format\"".to_string(), "hotkey ctrl+s".to_string()]);
    }
}
