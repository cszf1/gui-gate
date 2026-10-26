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
//!        expiry. A changed window, target or payload needs a fresh decision; on top of the key, the approval is bound
//!        to its business premise (server.rs `Premise`: the window's value rows, file versions) re-read before the send.
//!  * conservative approximations, used ONLY for denial coverage (they can refuse more, never approve more): a bare
//!    `name:<basename>` from a dialog's wording matches any path with that basename, and a GUI control is matched by
//!    app + role|label (the same "Send" label in the same app is one effect). Approvals never use either.
//!  * payload / command digests are SHA-256 (v0.10.3; FNV-1a 64 before). Logs written before v0.10.3 carry 16-hex FNV
//!    digests: `EffectKey::from_json` reads them and marks them legacy (`digest_kind`); a legacy digest never equals a
//!    new one, so an old record can never approve anything now.
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

/// SHA-256 of a payload / command line, 64 hex chars. It takes part in approval equality, so it is a cryptographic
/// digest (v0.10.3; nothing secret is derived from it). Pure Rust (util::sha256, checked against the NIST vectors).
pub fn digest(s: &str) -> String {
    crate::util::hex(&crate::util::sha256(s.as_bytes()))
}

/// The pre-v0.10.3 payload digest (FNV-1a 64), only to recognise / recompute digests in old logs.
pub fn digest_fnv_legacy(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// "sha256" | "fnv1a64" (a log written before v0.10.3) | "" (none / not a digest).
pub fn digest_kind(d: &str) -> &'static str {
    let hexy = !d.is_empty() && d.bytes().all(|b| b.is_ascii_hexdigit());
    match d.len() {
        64 if hexy => "sha256",
        16 if hexy => "fnv1a64",
        _ => "",
    }
}

/// Does a digest from a log (either generation) match this payload text?
pub fn digest_matches(d: &str, text: &str) -> bool {
    match digest_kind(d) {
        "sha256" => d.eq_ignore_ascii_case(&digest(text)),
        "fnv1a64" => d.eq_ignore_ascii_case(&digest_fnv_legacy(text)),
        _ => false,
    }
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

    /// Read a key back from a log line (`to_json`), old or new. `legacy` is true when its payload or a `cmd:` digest
    /// is a pre-v0.10.3 FNV digest: such a key is for reading / statistics only.
    pub fn from_json(v: &Value) -> Option<(EffectKey, bool)> {
        let o = v.as_object()?;
        let st = |k: &str| o.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let resources: Vec<String> = o.get("resources")?.as_array()?.iter().filter_map(|r| r.as_str().map(String::from)).collect();
        let k = EffectKey { tool: st("tool"), hazard: st("hazard"), resources, app: o.get("app").and_then(|x| x.as_i64()),
                            window: o.get("window").and_then(|x| x.as_i64()), payload: st("payload") };
        let legacy = digest_kind(&k.payload) == "fnv1a64"
            || k.resources.iter().any(|r| r.strip_prefix("cmd:").map(|d| digest_kind(d) == "fnv1a64").unwrap_or(false));
        Some((k, legacy))
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
        assert_eq!(digest("x").len(), 64);
        assert_eq!(digest("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(digest_kind(&digest("x")), "sha256");
    }

    #[test]
    fn old_logs_still_read() {
        // a v0.10.2 log line: FNV-1a 64 payload and command digests
        let old = json!({"tool": "run_command", "hazard": "delete", "resources": [format!("cmd:{}", digest_fnv_legacy("rm x"))],
                         "app": null, "window": null, "payload": digest_fnv_legacy("rm x")});
        let (k, legacy) = EffectKey::from_json(&old).unwrap();
        assert!(legacy);
        assert_eq!(digest_kind(&k.payload), "fnv1a64");
        assert!(digest_matches(&k.payload, "rm x") && !digest_matches(&k.payload, "rm y"));
        // a legacy key never equals the key the gate computes now for the same command
        let new = EffectKey { payload: digest("rm x"), resources: vec![format!("cmd:{}", digest("rm x"))], ..k.clone() };
        assert_ne!(k, new);
        let (n2, l2) = EffectKey::from_json(&new.to_json()).unwrap();
        assert!(!l2);
        assert_eq!(n2, new);
        assert!(digest_matches(&new.payload, "rm x"));
    }
}
