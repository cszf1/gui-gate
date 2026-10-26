//! v0.10.3 (review 2026-10-26 v2, sections 3 and 4): the gate's records of what it cannot take for granted.
//!
//!  * `UnknownEntry`: an action whose outcome the gate could not learn (the driver died / timed out / answered garbage
//!    while delivering it), or a step a person took over. It stays until the gate has evidence: a SUCCESSFUL read of
//!    the same window after it (a failed read clears nothing); for a non-idempotent effect (send, delete, pay, submit,
//!    a hand-off target) even that is not enough - only a person confirming it did not happen unlocks it (the driver
//!    has no idempotency keys and the gate has no way to prove "not executed").
//!  * `Premise`: the business state an approval was given on - the value rows of the target window as last read
//!    (recipient, amount, body, form fields), that read's snapshot id (object version) and the version of every file
//!    the effect names. Re-read before the send; a different digest voids the approval. When nothing of that can be
//!    read the premise is "unverifiable" and the gate says so and takes the stricter path.
//!  * `ReqRec`: one record per person request (id, kind, task version, effect, deadline) with exactly one end.
use crate::state::{GateState, Observation};
use crate::util::*;
use serde_json::{json, Value};

#[derive(Clone, Debug)]
pub struct UnknownEntry {
    pub why: String,
    pub tool: String,
    /// when the outcome became unknown: only a read AFTER this counts as looking again
    pub at: f64,
    pub pid: Option<i64>,
    pub window: Option<i64>,
    /// send / delete / pay / submit / a hand-off target: a read alone never unlocks it
    pub non_idempotent: bool,
    /// "driver" (outcome unknown) | "person" (a person did this step themselves)
    pub source: String,
    /// an unmodelled action (launch_app ...): the window list was read successfully after it
    pub listed: bool,
}

impl UnknownEntry {
    /// Was the window (or, without a window, the app; without either, anything) read successfully after `at`?
    pub fn observed_since(&self, st: &GateState) -> bool {
        match (self.pid, self.window) {
            (None, None) => st.obs.last().map(|o| o.t > self.at).unwrap_or(false),
            (p, w) => st.latest_for(p, w).map(|o| st.get(o).t > self.at).unwrap_or(false),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({"why": self.why, "tool": self.tool, "pid": self.pid, "window": self.window, "non_idempotent": self.non_idempotent, "source": self.source})
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Premise {
    /// something business-relevant could be read (a tree of the target window, or the files the effect names)
    pub readable: bool,
    /// SHA-256 over the rows and file versions (empty when not readable)
    pub digest: String,
    /// a short human summary of the rows (shown to the person)
    pub rows: Vec<String>,
    /// snapshot id of the read the rows came from (object version)
    pub snapshot: String,
    /// (path, Some((mtime_ns, size)) | None = missing)
    pub files: Vec<(String, Option<(u128, u64)>)>,
}

/// Roles whose rows carry business content (what would be sent / paid / submitted).
const VALUE_ROLES: &[&str] = &["text", "entry", "edit", "field", "combo", "cell", "label", "document", "paragraph", "heading",
    "static", "spin", "slider", "check", "radio", "list item", "row", "status", "tooltip", "link", "image"];

fn row_of(e: &crate::state::Element) -> Option<String> {
    let lab = e.label.trim();
    let val = e.value.as_deref().unwrap_or("").trim();
    if lab.is_empty() && val.is_empty() {
        return None;
    }
    Some(if val.is_empty() { format!("{}: {lab}", e.role) } else { format!("{}: {lab} = {val}", e.role) })
}

fn file_version(p: &str) -> Option<(u128, u64)> {
    let m = std::fs::metadata(p).ok()?;
    let t = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos()).unwrap_or(0);
    Some((t, m.len()))
}

impl Premise {
    /// The premise from an observation of the target window (None = never read) and the effect's file resources.
    pub fn of(obs: Option<&Observation>, files: &[String]) -> Premise {
        let mut all: Vec<String> = vec![];
        let mut summary: Vec<String> = vec![];
        let mut snapshot = String::new();
        if let Some(o) = obs.filter(|o| !o.elements.is_empty()) {
            snapshot = o.snapshot_id.clone();
            for e in o.elements.values() {
                if let Some(r) = row_of(e) {
                    let role = e.role.to_lowercase();
                    if VALUE_ROLES.iter().any(|v| role.contains(v)) && summary.len() < 12 {
                        summary.push(trunc(&r, 120).to_string());
                    }
                    all.push(r);
                }
            }
        }
        let files: Vec<(String, Option<(u128, u64)>)> = files.iter().map(|p| (p.clone(), file_version(p))).collect();
        for (p, v) in &files {
            summary.push(match v { Some((_, n)) => format!("file {p} ({n} bytes)"), None => format!("file {p} (does not exist)") });
        }
        let readable = !all.is_empty() || !files.is_empty();
        let digest = if readable {
            let mut src = all.join("\n");
            for (p, v) in &files {
                src.push_str(&format!("\nfile:{p}:{v:?}"));
            }
            hex(&sha256(src.as_bytes()))
        } else {
            String::new()
        };
        Premise { readable, digest, rows: summary, snapshot, files }
    }

    pub fn to_json(&self) -> Value {
        if !self.readable {
            return json!({"readable": false, "note": "unverifiable: the gate cannot read what this action would act on (no tree of the target window, no file); the approval is short-lived and nothing confirms the content before it is sent"});
        }
        json!({"readable": true, "digest": self.digest, "snapshot": self.snapshot, "rows": self.rows})
    }
}

/// What changed between two premises, for the refusal text.
pub fn premise_diff(old: &Premise, new: &Premise) -> String {
    let a: std::collections::HashSet<&String> = old.rows.iter().collect();
    let b: std::collections::HashSet<&String> = new.rows.iter().collect();
    let gone: Vec<&str> = old.rows.iter().filter(|r| !b.contains(r)).map(|r| r.as_str()).take(3).collect();
    let came: Vec<&str> = new.rows.iter().filter(|r| !a.contains(r)).map(|r| r.as_str()).take(3).collect();
    if gone.is_empty() && came.is_empty() {
        return "the window's content changed".into();
    }
    format!("was: {}; now: {}", if gone.is_empty() { "-".into() } else { gone.join(" | ") }, if came.is_empty() { "-".into() } else { came.join(" | ") })
}

/// One person request, from the moment it is put to the person until its single end.
#[derive(Clone, Debug)]
pub struct ReqRec {
    pub req: String,
    pub kind: String,
    pub task_version: u64,
    pub effect: Value,
    pub ask_key: String,
    pub asked: f64,
    pub deadline: f64,
}

/// Terminal states of a person request (exactly one per request).
pub const TERMINALS: &[&str] = &["answered", "no_answer", "expired_no_retry", "late", "void", "disconnected", "unused"];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Element;
    use indexmap::IndexMap;

    fn obs(rows: &[(&str, &str)]) -> Observation {
        let mut els = IndexMap::new();
        for (i, (r, l)) in rows.iter().enumerate() {
            els.insert(i as i64, Element::new(i as i64, r, l));
        }
        Observation { snapshot_id: "s1".into(), elements: els, t: 1.0, ..Default::default() }
    }

    #[test]
    fn premise_changes_with_the_recipient() {
        let a = Premise::of(Some(&obs(&[("button", "Send"), ("text", "To: bob@example.com")])), &[]);
        let b = Premise::of(Some(&obs(&[("button", "Send"), ("text", "To: eve@example.net")])), &[]);
        assert!(a.readable && b.readable);
        assert_ne!(a.digest, b.digest);
        assert!(premise_diff(&a, &b).contains("eve@example.net"));
        assert_eq!(a, Premise::of(Some(&obs(&[("button", "Send"), ("text", "To: bob@example.com")])), &[]));
    }

    #[test]
    fn nothing_readable_is_unverifiable() {
        let p = Premise::of(None, &[]);
        assert!(!p.readable && p.digest.is_empty());
        assert_eq!(p.to_json()["readable"], json!(false));
    }

    #[test]
    fn file_versions_are_part_of_it() {
        let d = std::env::temp_dir().join(format!("gg-prem-{}", std::process::id()));
        std::fs::write(&d, "a").unwrap();
        let f = vec![d.to_string_lossy().to_string()];
        let a = Premise::of(None, &f);
        std::fs::write(&d, "ab").unwrap();
        let b = Premise::of(None, &f);
        assert!(a.readable);
        assert_ne!(a.digest, b.digest);
        let _ = std::fs::remove_file(&d);
    }
}
