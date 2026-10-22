//! 0.9.7 milestones as checkable END STATES (Pointer planner, VLAA "UI-observable success criteria", HIPPO plan
//! check). A milestone may carry a predicate; check_done / set_plan evaluate it against the live machine without a
//! model. GUI_GATE_STATE_PLAN.
//!   {"file_exists": P}   {"file_changed": P} (modified during this gate session)   {"file_contains": [P, TEXT]}
//!   {"title_contains": T}  {"not_title_contains": T}  {"window_open": T}  {"app_running": NAME}
//!   {"terminal_contains": T}  {"cell": ["B3", "VALUE"]}
//! Several keys in one object must all hold.
use crate::util::*;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;
use std::time::SystemTime;

pub const KEYS: &[&str] = &["file_exists", "file_changed", "file_contains", "title_contains", "not_title_contains", "window_open", "app_running", "terminal_contains", "cell"];

/// What the gate can see right now (filled by the server; tests fill it by hand).
#[derive(Default)]
pub struct Ctx {
    pub titles: Vec<String>,
    pub terminal: String,
    pub cells: String,
    pub roots: Vec<String>,
    pub start: Option<SystemTime>,
}

fn resolve(p: &str, roots: &[String]) -> Option<String> {
    let e = crate::sidefx::expanduser(p.trim());
    if e.starts_with('/') {
        return Some(e);
    }
    roots.iter().map(|r| format!("{r}/{e}")).find(|c| std::path::Path::new(c).exists())
}

fn changed_since(path: &str, start: Option<SystemTime>) -> bool {
    let Some(st) = start else { return false };
    let Ok(md) = std::fs::metadata(path) else { return false };
    let t = md.modified().ok();
    let c = {
        use std::os::unix::fs::MetadataExt;
        SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(md.ctime().max(0) as u64))
    };
    t.map(|t| t >= st).unwrap_or(false) || c.map(|c| c >= st).unwrap_or(false)
}

fn proc_running(name: &str) -> bool {
    let n = name.to_lowercase();
    std::fs::read_dir("/proc").map(|rd| rd.flatten().any(|e| {
        let f = e.file_name().to_string_lossy().to_string();
        f.chars().all(|c| c.is_ascii_digit()) && std::fs::read(e.path().join("cmdline")).map(|b| {
            let cl = String::from_utf8_lossy(&b).replace('\0', " ").to_lowercase();
            cl.split_whitespace().next().map(|a| crate::sidefx::basename(a).contains(&n)).unwrap_or(false)
        }).unwrap_or(false)
    })).unwrap_or(false)
}

fn cell_rx() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\b([A-Z]{1,3}\d{1,7})=([^|{\n]*)").unwrap())
}

/// Text a file holds for file_contains (office documents via their saved text).
fn file_text(p: &str) -> String {
    crate::inspect::read_file_text(p, 200_000)
}

/// (holds?, explanation). None = cannot tell (malformed predicate or nothing to look at).
pub fn eval(check: &Value, ctx: &Ctx) -> (Option<bool>, String) {
    let Some(o) = check.as_object() else { return (None, "not a predicate object".into()) };
    if o.is_empty() {
        return (None, "empty predicate".into());
    }
    let mut parts = vec![];
    let mut all = Some(true);
    let mut and = |r: Option<bool>, why: String| {
        all = match (all, r) { (Some(false), _) | (_, Some(false)) => Some(false), (None, _) | (_, None) => None, _ => Some(true) };
        parts.push(why);
    };
    for (k, v) in o {
        let sv = pystr(Some(v));
        match k.as_str() {
            "file_exists" => match resolve(&sv, &ctx.roots) {
                Some(p) if std::path::Path::new(&p).exists() => and(Some(true), format!("{sv} exists")),
                _ => and(Some(false), format!("{sv} does not exist")),
            },
            "file_changed" => match resolve(&sv, &ctx.roots) {
                Some(p) if changed_since(&p, ctx.start) => and(Some(true), format!("{sv} changed this session")),
                Some(p) if std::path::Path::new(&p).exists() => and(Some(false), format!("{sv} exists but did not change this session")),
                _ => and(Some(false), format!("{sv} does not exist")),
            },
            "file_contains" => {
                let arr = v.as_array().cloned().unwrap_or_default();
                let (p, t) = (arr.first().map(|x| pystr(Some(x))).unwrap_or_default(), arr.get(1).map(|x| pystr(Some(x))).unwrap_or_default());
                match resolve(&p, &ctx.roots) {
                    Some(f) if std::path::Path::new(&f).is_file() && !t.is_empty() => {
                        let has = file_text(&f).to_lowercase().contains(&t.to_lowercase());
                        and(Some(has), format!("{p} {} \"{}\" (saved file)", if has { "contains" } else { "does not contain" }, trunc(&t, 40)));
                    }
                    Some(_) if t.is_empty() => and(None, "file_contains needs [path, text]".into()),
                    _ => and(Some(false), format!("{p} does not exist")),
                }
            }
            "title_contains" | "window_open" => {
                let has = ctx.titles.iter().any(|ti| ti.to_lowercase().contains(&sv.to_lowercase()));
                and(Some(has), format!("{} window title contains \"{}\"", if has { "a" } else { "no" }, trunc(&sv, 40)));
            }
            "not_title_contains" => {
                let has = ctx.titles.iter().any(|ti| ti.to_lowercase().contains(&sv.to_lowercase()));
                and(Some(!has), format!("{} window title contains \"{}\"", if has { "a" } else { "no" }, trunc(&sv, 40)));
            }
            "app_running" => {
                let r = proc_running(&sv);
                and(Some(r), format!("{sv} {} running", if r { "is" } else { "is not" }));
            }
            "terminal_contains" => {
                if ctx.terminal.is_empty() {
                    and(None, "no terminal text visible".into());
                } else {
                    let has = ctx.terminal.contains(&sv);
                    and(Some(has), format!("terminal {} \"{}\"", if has { "shows" } else { "does not show" }, trunc(&sv, 40)));
                }
            }
            "cell" => {
                let arr = v.as_array().cloned().unwrap_or_default();
                let (c, want) = (arr.first().map(|x| pystr(Some(x)).to_uppercase()).unwrap_or_default(), arr.get(1).map(|x| pystr(Some(x))).unwrap_or_default());
                if ctx.cells.is_empty() {
                    and(None, format!("no spreadsheet cells visible to check {c}"));
                } else {
                    let got = cell_rx().captures_iter(&ctx.cells).find(|m| m[1] == c).map(|m| m[2].trim().to_string());
                    match got {
                        Some(g) => and(Some(g == want.trim()), format!("{c} shows \"{}\" (want \"{}\")", trunc(&g, 30), trunc(&want, 30))),
                        None => and(None, format!("{c} is not among the visible cells")),
                    }
                }
            }
            other => and(None, format!("unknown predicate {other:?} (use {})", KEYS.join(", "))),
        }
    }
    (all, parts.join("; "))
}

/// A milestone that reads like a click recipe instead of an end state (Pointer: "describe states, not steps").
pub fn recipe_like(m: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)^\s*(click|double[- ]click|right[- ]click|press|type|tap|open the .* menu|go to the .* menu|select the .* menu|scroll|hit)\b|\bthen click\b|\bclick (on )?the\b").unwrap()).is_match(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn predicates() {
        let d = std::env::temp_dir().join(format!("gg-pc-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let before = SystemTime::now() - std::time::Duration::from_secs(1);
        std::fs::write(d.join("out.txt"), "Total: 42").unwrap();
        let ctx = Ctx { titles: vec!["report.xlsx - LibreOffice Calc".into()], terminal: "$ ls\nfile1\n$".into(), cells: "4: A4=plum | B4=7 | C4=14 {=B3*2}".into(),
                        roots: vec![d.to_string_lossy().to_string()], start: Some(before) };
        assert_eq!(eval(&json!({"file_exists": "out.txt"}), &ctx).0, Some(true));
        assert_eq!(eval(&json!({"file_changed": "out.txt"}), &ctx).0, Some(true));
        assert_eq!(eval(&json!({"file_contains": ["out.txt", "total: 42"]}), &ctx).0, Some(true));
        assert_eq!(eval(&json!({"file_contains": ["out.txt", "43"]}), &ctx).0, Some(false));
        assert_eq!(eval(&json!({"file_exists": "nope.txt"}), &ctx).0, Some(false));
        assert_eq!(eval(&json!({"title_contains": "report.xlsx"}), &ctx).0, Some(true));
        assert_eq!(eval(&json!({"not_title_contains": "*"}), &ctx).0, Some(true));
        assert_eq!(eval(&json!({"terminal_contains": "file1"}), &ctx).0, Some(true));
        assert_eq!(eval(&json!({"cell": ["c4", "14"]}), &ctx).0, Some(true));
        assert_eq!(eval(&json!({"cell": ["B4", "8"]}), &ctx).0, Some(false));
        assert_eq!(eval(&json!({"cell": ["Z9", "1"]}), &ctx).0, None);
        assert_eq!(eval(&json!({"file_exists": "out.txt", "title_contains": "Writer"}), &ctx).0, Some(false));
        assert_eq!(eval(&json!({"bogus": 1}), &ctx).0, None);
        let old = Ctx { start: Some(SystemTime::now() + std::time::Duration::from_secs(60)), ..ctx };
        assert_eq!(eval(&json!({"file_changed": "out.txt"}), &old).0, Some(false));
        assert!(recipe_like("Click the Data menu, then Sort"));
        assert!(recipe_like("press ctrl+s"));
        assert!(!recipe_like("report.xlsx is sorted by date and saved"));
        assert!(!recipe_like("The Chrome font size is 20"));
    }
}
