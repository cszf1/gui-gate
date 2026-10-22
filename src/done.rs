//! check_done: tool-side evidence before the agent declares a task finished. No model: the gate collects what it
//! can see (files the task names and their mtimes, files changed this session, unsaved-document markers, Calc cells,
//! terminal tail, the active window, its own refusals, the agent's plan) and flags gaps; the agent decides.
use crate::sidefx;
use regex::Regex;
use serde_json::{json, Value};
use std::sync::OnceLock;
use std::time::SystemTime;

macro_rules! re {
    ($name:ident, $pat:expr) => {
        fn $name() -> &'static Regex {
            static R: OnceLock<Regex> = OnceLock::new();
            R.get_or_init(|| Regex::new($pat).unwrap())
        }
    };
}
const EXTS: &str = "pptx|ppt|odp|docx|doc|odt|rtf|xlsx|xls|ods|csv|tsv|pdf|png|jpe?g|gif|bmp|svg|webp|xcf|txt|md|json|xml|html?|py|sh|ipynb|zip|tar|gz|mp3|mp4|wav|ogg|epub";
re!(quoted, r#"["'“‘`]([^"'“”‘’`\n]{1,120})["'”’`]"#);
re!(bare_file, &format!(r"(?i)(?:^|[\s(])((?:~|/)?[\w\-~][\w.\-/~]*\.(?:{EXTS}))\b"));
re!(abs_path, r"(?:^|\s)((?:/|~/)[\w.\-/]+)");
re!(git_url, r"(?i)https?://(?:github\.com|gitlab\.com|bitbucket\.org)/[\w.\-]+/([\w.\-]+?)(?:\.git)?(?:[\s/\x22'”]|$)");
re!(produce_words, r"(?i)\b(save|saved|export|exported|create|created|write|copy|copies|copied|rename|move|clone|convert|download|generate|produce|backup|compress|extract)\b|保存|导出|创建|复制|重命名|移动|克隆|转换|下载");
re!(on_desktop, r"(?i)\bon (the|my) desktop\b|桌面");

/// Paths and names the task text mentions: quoted names (files or folders, no spaces), bare names with a known
/// document extension, absolute / ~ paths, and the folder a git clone creates. Deduplicated, in order.
pub fn task_paths(task: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut add = |s: &str| {
        let s = s.trim().trim_end_matches(['.', ',', ';', ':', ')', '?', '!']).to_string();
        if s.len() >= 2 && !s.contains("://") && !out.contains(&s) {
            out.push(s);
        }
    };
    for c in quoted().captures_iter(task) {
        let q = c[1].trim();
        if !q.contains(' ') && !q.contains('*') && (q.contains('.') || q.contains('/') || q.contains('_') || q.contains('-')) && !q.contains("://") {
            add(q);
        }
    }
    for c in bare_file().captures_iter(task) {
        if !c[1].contains('*') {
            add(&c[1]);
        }
    }
    for c in abs_path().captures_iter(task) {
        add(&c[1]);
    }
    for c in git_url().captures_iter(task) {
        add(&c[1]);
    }
    out
}

pub fn asks_to_produce(task: &str) -> bool {
    produce_words().is_match(task)
}

/// True when a window title carries an unsaved-changes marker (gedit/mousepad/GIMP "*name", "name [Modified]", "•").
pub fn title_unsaved(title: &str) -> bool {
    let t = title.trim();
    t.starts_with('*') || t.contains(" *") && !t.contains(" * ") || t.contains('•') || t.to_lowercase().contains("[modified]")
        || t.to_lowercase().contains("(modified)")
}

/// LibreOffice's status bar "document modified" indicator text says the document has unsaved changes.
pub fn status_says_modified(texts: &[String]) -> Option<bool> {
    let mut seen = None;
    for t in texts {
        let l = t.to_lowercase();
        if l.contains("not been modified") || l.contains("no unsaved") {
            seen = Some(false);
        } else if l.contains("has been modified") || l.contains("document modified") || l.contains("unsaved changes") || l.contains("已修改") {
            return Some(true);
        }
    }
    seen
}

fn fmt_time(t: SystemTime) -> String {
    let secs = t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    // local wall clock of the controlled machine is not known without tz data; give the age instead
    let age = SystemTime::now().duration_since(t).map(|d| d.as_secs() as i64).unwrap_or(0);
    let _ = secs;
    if age < 120 { format!("{age} s ago") } else if age < 7200 { format!("{} min ago", age / 60) } else { format!("{} h ago", age / 3600) }
}

fn changed_at(m: &std::fs::Metadata) -> Option<SystemTime> {
    crate::proc::changed_at(m)
}

/// Newest change (mtime/ctime) inside a directory (depth 3, bounded).
fn newest_inside(dir: &str, depth: usize, budget: &mut usize) -> Option<SystemTime> {
    let mut best = std::fs::metadata(dir).ok().and_then(|m| changed_at(&m));
    if depth == 0 {
        return best;
    }
    let rd = std::fs::read_dir(dir).ok()?;
    for e in rd.flatten() {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let Ok(m) = e.metadata() else { continue };
        let t = if m.is_dir() { newest_inside(&e.path().to_string_lossy(), depth - 1, budget) } else { changed_at(&m) };
        if let Some(t) = t {
            if best.map(|b| t > b).unwrap_or(true) {
                best = Some(t);
            }
        }
    }
    best
}

/// Where a bare name may live: the given roots, then a bounded walk under home (depth 4).
pub fn resolve(name: &str, roots: &[String]) -> Vec<String> {
    let home = crate::util::home();
    if name.starts_with('/') || name.starts_with('~') {
        let p = sidefx::p(name, &home);
        return if sidefx::exists(&p) { vec![p] } else { vec![] };
    }
    let mut hits = vec![];
    for r in roots {
        let p = sidefx::p(name, r);
        if sidefx::exists(&p) && !hits.contains(&p) {
            hits.push(p);
        }
    }
    if !hits.is_empty() || name.contains('/') {
        return hits;
    }
    let base = name.to_string();
    let mut budget = 20000usize;
    fn walk(dir: &str, base: &str, depth: usize, budget: &mut usize, hits: &mut Vec<String>) {
        if depth == 0 || *budget == 0 || hits.len() >= 5 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            if *budget == 0 {
                return;
            }
            *budget -= 1;
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with('.') && n != base {
                continue;
            }
            let p = e.path().to_string_lossy().to_string();
            if n == base {
                hits.push(p.clone());
            }
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) && n != "node_modules" && n != "__pycache__" {
                walk(&p, base, depth - 1, budget, hits);
            }
        }
    }
    walk(&home, &base, 4, &mut budget, &mut hits);
    hits
}

/// One evidence line per named path, plus whether it is a gap (missing, or present but untouched this session).
pub fn path_evidence(name: &str, roots: &[String], start: Option<SystemTime>) -> (String, Option<String>) {
    let hits = resolve(name, roots);
    if hits.is_empty() {
        return (format!("- {name}: not found (looked in {} and under the home folder)", roots.join(", ")),
                Some(format!("{name} named in the task does not exist")));
    }
    let mut lines = vec![];
    let mut any_changed = false;
    for h in hits.iter().take(3) {
        let Ok(m) = std::fs::metadata(h) else { continue };
        let mut budget = 3000usize;
        let t = if m.is_dir() { newest_inside(h, 3, &mut budget) } else { changed_at(&m) };
        let changed = match (t, start) { (Some(t), Some(s)) => t >= s, _ => false };
        any_changed |= changed;
        let kind = if m.is_dir() { "folder".to_string() } else { format!("{} B", m.len()) };
        let when = t.map(fmt_time).unwrap_or_else(|| "?".into());
        lines.push(format!("- {h}: exists ({kind}), last modified {when}{}", if changed { " - changed during this session" } else { " - unchanged since before this session" }));
    }
    let gap = if any_changed { None } else { Some(format!("{name} exists but nothing in it changed during this session")) };
    (lines.join("\n"), gap)
}

/// Files under `root` (depth 3, no dot folders) changed since `start`, newest first.
pub fn changed_since(root: &str, start: SystemTime, max: usize) -> Vec<(String, u64, SystemTime)> {
    let mut out = vec![];
    let mut budget = 20000usize;
    fn walk(dir: &str, start: SystemTime, depth: usize, budget: &mut usize, out: &mut Vec<(String, u64, SystemTime)>) {
        if depth == 0 || *budget == 0 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            if *budget == 0 {
                return;
            }
            *budget -= 1;
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with('.') || n == "node_modules" || n == "__pycache__" {
                continue;
            }
            let Ok(m) = e.metadata() else { continue };
            let p = e.path().to_string_lossy().to_string();
            if m.is_dir() {
                walk(&p, start, depth - 1, budget, out);
            } else if let Ok(t) = m.modified() {
                if t >= start {
                    out.push((p, m.len(), t));
                }
            }
        }
    }
    walk(root, start, 3, &mut budget, &mut out);
    out.sort_by(|a, b| b.2.cmp(&a.2));
    out.truncate(max);
    out
}

pub fn tool_schema() -> Value {
    json!({"name": "check_done",
        "description": "Before you reply DONE, call this once: the gate gathers evidence about the end state (files the task names: exist? changed during this session?; files changed this session; unsaved-document markers; visible Calc cells; terminal tail; open windows; steps the gate refused; your plan's open milestones) and lists possible gaps. It does not judge the task; you decide whether to finish or fix something first.",
        "inputSchema": {"type": "object", "properties": {
            "files": {"type": "array", "items": {"type": "string"}, "description": "extra paths you produced or changed (optional)"}},
            "required": []}, "annotations": {"readOnlyHint": true}})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_from_task_text() {
        assert_eq!(task_paths("Could you help me save my slides as pre.pptx on the Desktop?"), vec!["pre.pptx"]);
        assert_eq!(task_paths("Copies file 'file1' to each of directories 'dir1', 'dir2', 'dir3'."), Vec::<String>::new());
        assert_eq!(task_paths("I have a directory named \"todo_list_Jan_1\". Can you help me change its name into \"todo_list_Jan_2\"?"),
                   vec!["todo_list_Jan_1", "todo_list_Jan_2"]);
        assert_eq!(task_paths("Please help me clone the repo \"https://github.com/xlang-ai/instructor-embedding\" to /home/user."),
                   vec!["/home/user", "instructor-embedding"]);
        assert_eq!(task_paths("export an Impress file to a .png image file and save it as res.png on the Desktop"), vec!["res.png"]);
        assert_eq!(task_paths("Copy all files matching \"*failed.ipynb\" in the current directory tree to \"./fails\""), vec!["./fails"]);
        assert!(asks_to_produce("save my slides as pre.pptx"));
        assert!(!asks_to_produce("Make the line spacing of first two paragraph into double line spacing"));
    }

    #[test]
    fn unsaved_markers() {
        assert!(title_unsaved("*notes.txt - Mousepad"));
        assert!(title_unsaved("*[Untitled]-1.0 (RGB color 8-bit gamma integer, GIMP built-in sRGB, 1 layer) 800x600 – GIMP"));
        assert!(!title_unsaved("pre.pptx — LibreOffice Impress"));
        assert!(!title_unsaved("Terminal - sandbox@box: /home/user"));
        assert_eq!(status_says_modified(&["The document has been modified. Click to save the document.".into()]), Some(true));
        assert_eq!(status_says_modified(&["The document has not been modified since the last save.".into()]), Some(false));
        assert_eq!(status_says_modified(&["Page 1 of 2".into()]), None);
    }

    #[test]
    fn evidence_for_paths() {
        let d = format!("/tmp/gg-done-{}", crate::util::rand_hex(3));
        std::fs::create_dir_all(format!("{d}/repo")).unwrap();
        std::fs::write(format!("{d}/repo/a"), "x").unwrap();
        let start = SystemTime::now() + std::time::Duration::from_secs(5);
        let (l, gap) = path_evidence("repo", &[d.clone()], Some(start));
        assert!(l.contains("unchanged since before this session") && gap.is_some(), "{l}");
        let (_, gap) = path_evidence("missing.pptx", &[d.clone()], Some(start));
        assert!(gap.unwrap().contains("does not exist"));
        let past = SystemTime::now() - std::time::Duration::from_secs(5);
        let (l, gap) = path_evidence("repo", &[d.clone()], Some(past));
        assert!(l.contains("changed during this session") && gap.is_none(), "{l}");
        assert_eq!(changed_since(&d, past, 5).len(), 1);
    }
}
