//! Structured effect keys: what a risky action would do, independent of the words that describe it.
//!
//! A key names the hazard (delete, overwrite, move, send ...), the resources it acts on, the app/window it happens
//! in, the tool that would cause it and a digest of its payload (typed text, keys, command line).
//!  * resources: `path:<canonical absolute path>` for files the disk checks resolved, `name:<basename>` when only a
//!    dialog's wording names the file, `elem:<role>|<label>` for a GUI control without a file behind it,
//!    `cmd:<digest>` for a command line with no resolvable target.
//!  * two uses with different matching:
//!      - a DENIAL record covers a stable range: same hazard and an overlapping resource (equal path, a path inside a
//!        denied folder or a folder holding a denied path, a bare name equal to a path's basename, the same control in
//!        the same app, the same command digest). The tool does not matter: `rm x` in run_command and the same `rm x`
//!        typed into a terminal are one effect; so are a dialog's "a.pdf already exists" and `/home/u/a.pdf`.
//!      - an APPROVAL (a dispatch permit) is exact: every field equal, window and payload included, one use, short
//!        expiry. A changed window, target or payload needs a fresh decision.
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct EffectKey {
    pub tool: String,
    pub hazard: String,
    pub resources: Vec<String>,
    pub app: Option<i64>,
    pub window: Option<i64>,
    pub payload: String,
}

/// FNV-1a 64-bit (equality of payloads only; not a security hash, nothing secret is derived from it).
pub fn digest(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// `a` equals `b` or is a folder that contains it.
pub fn path_covers(a: &str, b: &str) -> bool {
    let a = a.trim_end_matches('/');
    let b = b.trim_end_matches('/');
    a == b || (!a.is_empty() && b.starts_with(a) && b.as_bytes().get(a.len()) == Some(&b'/')) || (a.is_empty() && b.starts_with('/'))
}

fn base_of(p: &str) -> String {
    p.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_lowercase()
}

fn res_overlap(a: &str, b: &str) -> bool {
    let (ka, va) = a.split_once(':').unwrap_or(("", a));
    let (kb, vb) = b.split_once(':').unwrap_or(("", b));
    match (ka, kb) {
        ("path", "path") => path_covers(va, vb) || path_covers(vb, va),
        ("name", "path") => base_of(vb) == va,
        ("path", "name") => base_of(va) == vb,
        _ => ka == kb && va == vb,
    }
}

impl EffectKey {
    /// Does this denial record cover `other`? (see the module doc)
    pub fn denial_covers(&self, other: &EffectKey) -> bool {
        if self.hazard != other.hazard {
            return false;
        }
        let gui_only = |k: &EffectKey| k.resources.iter().all(|r| r.starts_with("elem:"));
        if gui_only(self) && gui_only(other) && self.app != other.app {
            return false; // the same button label in another app is another effect
        }
        self.resources.iter().any(|a| other.resources.iter().any(|b| res_overlap(a, b)))
    }

    pub fn files(&self) -> Vec<&str> {
        self.resources.iter().filter(|r| r.starts_with("path:") || r.starts_with("name:")).map(|r| r.as_str()).collect()
    }

    pub fn to_json(&self) -> Value {
        json!({"tool": self.tool, "hazard": self.hazard, "resources": self.resources, "app": self.app, "window": self.window,
               "payload": if self.payload.is_empty() { Value::Null } else { json!(self.payload) }})
    }

    /// One line for messages and logs.
    pub fn short(&self) -> String {
        format!("{}:{}", self.hazard, self.resources.iter().take(3).cloned().collect::<Vec<_>>().join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(hz: &str, res: &[&str], tool: &str, app: Option<i64>, win: Option<i64>) -> EffectKey {
        EffectKey { tool: tool.into(), hazard: hz.into(), resources: res.iter().map(|s| s.to_string()).collect(), app, window: win, payload: String::new() }
    }

    #[test]
    fn same_name_in_another_folder_is_another_effect() {
        let a = k("delete", &["path:/home/u/a/report.txt"], "run_command", None, None);
        let b = k("delete", &["path:/home/u/b/report.txt"], "run_command", None, None);
        assert!(!a.denial_covers(&b) && !b.denial_covers(&a));
        assert_ne!(a, b);
    }

    #[test]
    fn same_effect_through_another_tool_or_route() {
        let cmd = k("delete", &["path:/home/u/a/report.txt"], "run_command", None, None);
        let term = k("delete", &["path:/home/u/a/report.txt"], "press_key", Some(77), Some(5));
        assert!(cmd.denial_covers(&term) && term.denial_covers(&cmd));
        let dir = k("delete", &["path:/home/u/a"], "run_command", None, None);
        assert!(dir.denial_covers(&cmd), "a refused folder covers what is inside it");
        assert!(cmd.denial_covers(&dir), "deleting the folder deletes the refused file too");
        assert!(!k("delete", &["path:/home/u/ab"], "x", None, None).denial_covers(&cmd));
        let dialog = k("overwrite", &["name:a.pdf"], "click", Some(1), Some(2));
        let save = k("overwrite", &["path:/home/u/a.pdf"], "run_command", None, None);
        assert!(dialog.denial_covers(&save) && save.denial_covers(&dialog));
        assert!(!k("move", &["path:/home/u/a.pdf"], "run_command", None, None).denial_covers(&save), "another hazard");
    }

    #[test]
    fn gui_controls_are_per_app() {
        let a = k("send", &["elem:button|send"], "click", Some(10), Some(1));
        let b = k("send", &["elem:button|send"], "click", Some(10), Some(2));
        let c = k("send", &["elem:button|send"], "click", Some(11), Some(1));
        assert!(a.denial_covers(&b));
        assert!(!a.denial_covers(&c));
        assert_ne!(a, b, "an approval is bound to the window");
    }

    #[test]
    fn digests_differ() {
        assert_ne!(digest("rm -rf /a"), digest("rm -rf /b"));
        assert_eq!(digest("x").len(), 16);
    }
}
