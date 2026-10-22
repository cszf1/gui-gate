//! Authorization check: does the user's own request cover this risky action?
//!
//! Only what the user explicitly asked for is authorized; anything the agent chose on its own is not.
//! Deterministic, no model. Two ways an action becomes authorized:
//!  * working-document exception: replacing the very file that is open in this app;
//!  * explicit wording: the task names the hazard and, when the action has a concrete file target, that file.
//! Also: a hazard class per risk reason, to pick a safe alternative when the action is denied
//! (deny-and-continue) and to remember denied effects.
use crate::sidefx::{basename, splitext_ext};
use regex::Regex;
use std::sync::OnceLock;

fn auth_words(hz: &str) -> Option<&'static [&'static str]> {
    Some(match hz {
        "overwrite" => &["overwrite", "replace", "replacing", "overwriting", "覆盖", "替换", "替代"],
        "delete" => &["delete", "remove", "erase", "wipe", "clear", "empty", "删除", "移除", "清空", "清除"],
        "send" => &["send", "submit", "publish", "post", "share", "发送", "提交", "发布", "分享"],
        "logout" => &["log out", "sign out", "logout", "退出登录", "注销"],
        "purchase" => &["buy", "purchase", "pay", "order", "checkout", "购买", "支付", "下单", "付款"],
        "power" => &["shut down", "shutdown", "restart", "reboot", "关机", "重启"],
        "install" => &["install", "uninstall", "安装", "卸载"],
        "move" => &["move", "moving", "rename", "renaming", "change its name", "change the name", "mv", "relocate",
                     "移动", "移到", "重命名", "改名"],
        "permissions" => &["chmod", "permission", "permissions", "权限"],
        _ => return None,
    })
}

/// Hazards a request typed into gate_task by the agent itself cannot authorize (only --task / GUI_GATE_TASK can).
pub const OPERATOR_ONLY: &[&str] = &["send", "purchase", "permissions", "power", "install", "logout", "command"];

fn file_in_reason() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?:replace the existing file|overwrite existing file\(s\):)\s*([^\s,()]+)").unwrap())
}
fn quoted_file() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"["'“‘]([^"'“”‘’\n]{1,200}\.[A-Za-z0-9]{1,6})["'”’]"#).unwrap())
}
fn name_in_dialog() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"(?i)(?:file named|named|文件)\s*["'“‘]?([^"'“”‘’\n]{1,200}?\.[A-Za-z0-9]{1,6})"#).unwrap())
}

pub fn hazard_of(reason: &str) -> &'static str {
    let r = reason.to_lowercase();
    let any = |ks: &[&str]| ks.iter().any(|k| r.contains(k));
    if any(&["replac", "overwrite", "覆盖", "clobber"]) {
        "overwrite"
    } else if r.contains("would move the existing") {
        "move"
    } else if r.contains("chmod") {
        "permissions"
    } else if any(&["delete", "rm ", "erase", "wipe", "remove", "trash", "删除", "清空", "find -exec rm"]) {
        "delete"
    } else if any(&["log out", "sign out", "logout", "退出登录"]) {
        "logout"
    } else if any(&["pay", "purchase", "buy", "order", "checkout", "支付", "购买", "下单"]) {
        "purchase"
    } else if any(&["send", "submit", "publish", "post", "share", "发送", "提交", "发布"]) {
        "send"
    } else if any(&["shut", "reboot", "restart", "关机", "重启"]) {
        "power"
    } else if r.contains("install") {
        "install"
    } else if r.contains("typed a dangerous command") || r.contains("enter would run") {
        "command"
    } else {
        "other"
    }
}

/// The file a risky action would replace, from the risk reason or the dialog's own words.
pub fn target_file(reason: &str, dialog_text: &str) -> Option<String> {
    if let Some(m) = file_in_reason().captures(reason) {
        return Some(m[1].trim_end_matches('.').to_string());
    }
    for rx in [name_in_dialog(), quoted_file()] {
        if let Some(m) = rx.captures(dialog_text) {
            return Some(m[1].trim().to_string());
        }
    }
    None
}

pub fn norm(s: &str) -> String {
    s.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// True when `path` (or its basename) is the document shown in one of the app's window titles.
pub fn is_open_document(path: Option<&str>, titles: &[String]) -> bool {
    let Some(path) = path else { return false };
    let base = basename(path).to_lowercase();
    if base.is_empty() || !base.contains('.') {
        return false;
    }
    let sep = {
        static R: OnceLock<Regex> = OnceLock::new();
        R.get_or_init(|| Regex::new(r"\s+[-–—]\s+").unwrap())
    };
    titles.iter().any(|t| {
        let tl = t.to_lowercase();
        let head = sep.splitn(&tl, 2).next().unwrap_or("").trim().to_string();
        head == base || head.ends_with(&format!("/{base}"))
    })
}

pub fn word_hit(word: &str, text: &str) -> bool {
    if word.is_ascii() {
        Regex::new(&format!(r"\b{}\b", regex::escape(word))).map(|r| r.is_match(text)).unwrap_or(false)
    } else {
        text.contains(word)
    }
}

const NEG_EN: &[&str] = &["not", "don't", "dont", "never", "without", "no", "avoid", "nor", "shouldn't", "mustn't", "can't", "cannot", "won't"];
const NEG_ZH: &[&str] = &["不要", "别", "勿", "不", "禁止", "无需", "不用", "先不", "避免"];

/// The word occurs in the request in a clause that is not negated ("do not send it yet" does not ask to send).
pub fn affirmed_hit(word: &str, text: &str) -> bool {
    let clause_break = |c: char| ",.;:!?，。；：！？\n".contains(c);
    if word.is_ascii() {
        let Ok(r) = Regex::new(&format!(r"\b{}\b", regex::escape(word))) else { return false };
        let hit = r.find_iter(text).any(|m| {
            let before = &text[..m.start()];
            let clause = before.rsplit(clause_break).next().unwrap_or("");
            let clause = clause.rsplit(" but ").next().unwrap_or(clause);
            let prev: Vec<&str> = clause.split_whitespace().rev().take(4).collect();
            !prev.iter().any(|w| NEG_EN.contains(&w.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')))
        });
        hit
    } else {
        text.match_indices(word).any(|(i, _)| {
            let clause = text[..i].rsplit(clause_break).next().unwrap_or("");
            let prev: String = clause.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
            !NEG_ZH.iter().any(|n| prev.contains(n))
        })
    }
}

/// (authorized?, why). Never authorizes when there is no task text, except the working document.
pub fn authorized(reason: &str, task: &str, _label: &str, dialog_text: &str, titles: &[String]) -> (bool, String) {
    let hz = hazard_of(reason);
    let tf = target_file(reason, dialog_text);
    if hz == "overwrite" && is_open_document(tf.as_deref(), titles) {
        return (true, format!("replaces the document that is open in this app ({})", basename(tf.as_deref().unwrap())));
    }
    let t = norm(task);
    if t.is_empty() {
        return (false, "no user request known to the gate".into());
    }
    let Some(words) = auth_words(hz) else {
        return (false, "this kind of action is never authorized by wording alone".into());
    };
    let Some(hit) = words.iter().find(|w| affirmed_hit(w, &t)) else {
        if words.iter().any(|w| word_hit(w, &t)) {
            return (false, format!("the request mentions it only in a negated way (does not ask to {hz})"));
        }
        return (false, format!("the request does not ask to {hz}"));
    };
    if let Some(tf) = tf {
        let base = basename(&tf).to_lowercase();
        let ext = splitext_ext(&base);
        let stem = base[..base.len() - ext.len()].to_string();
        if !t.contains(&base) && !(stem.chars().count() >= 3 && t.contains(&stem)) {
            return (false, format!("the request says \"{hit}\" but does not name {}", basename(&tf)));
        }
    }
    (true, format!("the request explicitly says \"{hit}\""))
}

pub fn alternative_for(hz: &str) -> &'static str {
    match hz {
        "overwrite" => "do not replace it: answer No/Cancel in the dialog, then save or export under a new name that does not exist yet (for example add _1 before the extension) in the same folder, finish the task with that file, and mention both paths in your final answer",
        "move" => "do not move or rename the existing path: leave it where it is and put the new result under a name that does not exist yet (for example clone or copy into <name>-1 or a new subfolder), finish the task with that, and mention both paths in your final answer",
        "delete" => "do not delete it: leave it in place (or move it to a clearly named backup folder only if the task needs the space) and continue with the rest of the task",
        "permissions" => "narrow the command so it touches only what the task names (for example `find ... -type f` instead of a recursive change on directories), list the matches first, then run it",
        "command" => "run a read-only version first (list what it would touch, e.g. `ls` / `find ... -print`), then a narrower command that touches only what the task names",
        "send" => "stop before the final send/submit: leave it as a draft and say what is ready in your final answer",
        "purchase" => "stop before paying or ordering and say what is ready in your final answer",
        "logout" => "do not log out; finish the task in the current session",
        "power" => "do not shut down or restart; finish the rest of the task",
        "install" => "do not install or uninstall anything; finish the task with what is available",
        _ => "find a way to finish the task that avoids this action",
    }
}

const ALTERNATIVES_CP: &str = "do not overwrite it: copy to the other destinations, and for the conflicting one use a name that does not exist yet (e.g. `cp -n`, or `cp file dir/file.copy`), then report it";

pub fn alternative(reason: &str) -> &'static str {
    if (reason.starts_with("`cp`") || reason.starts_with("`mv`") || reason.contains("would overwrite existing file")) && !reason.contains("would move the existing") {
        return ALTERNATIVES_CP;
    }
    alternative_for(hazard_of(reason))
}

/// What the action would do, independent of the route (dialog button, Enter key, terminal).
pub fn effect_key(reason: &str, dialog_text: &str) -> String {
    let hz = hazard_of(reason);
    match target_file(reason, dialog_text) {
        Some(tf) => format!("{hz}:{}", basename(&tf).to_lowercase()),
        None => format!("{hz}:{}", crate::util::trunc(&norm(reason), 80)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn working_document_and_wording() {
        let titles = vec!["report.pdf - Viewer".to_string()];
        let (ok, why) = authorized("\"Yes\" confirms replacing an existing file", "", "Yes",
                                   "A file named \"report.pdf\" already exists.", &titles);
        assert!(ok, "{why}");
        let r = "saving here would replace the existing file /home/u/Desktop/report.pdf";
        assert!(!authorized(r, "Export the current document into PDF, keep the file name", "", "", &[]).0);
        assert!(authorized(r, "Export it as PDF and overwrite report.pdf", "", "", &[]).0);
        assert!(!authorized(r, "Export it as PDF to the Desktop", "", "", &[]).0);
        let cp = "`cp` would overwrite existing file(s): dir2/file1";
        assert!(!authorized(cp, "Copies file 'file1' to each of directories 'dir1', 'dir2', 'dir3'.", "", "", &[]).0);
        assert!(alternative(cp).starts_with("do not overwrite it: copy"));
        assert!(!authorized("\"Delete file\" (delete)", "", "", "", &[]).0);
    }

    #[test]
    fn negated_wording_does_not_authorize() {
        let r = "\"Send\" (send)";
        assert!(!authorized(r, "draft a reply to Bob, do not send it yet", "", "", &[]).0);
        assert!(!authorized(r, "Write the email but don't send it", "", "", &[]).0);
        assert!(!authorized(r, "回复 Bob，先不要发送", "", "", &[]).0);
        assert!(authorized(r, "reply to Bob and send it", "", "", &[]).0);
        assert!(authorized(r, "do not wait, send it now", "", "", &[]).0);
        assert!(authorized(r, "写好后发送给 Bob", "", "", &[]).0);
        assert!(authorized(r, "send it before 5 pm", "", "", &[]).0);
        assert!(!authorized("\"Delete file\" (delete)", "Rename the file; never delete anything", "", "", &[]).0);
    }

    #[test]
    fn same_effect_same_key() {
        let a = effect_key("saving here would replace the existing file /home/u/a.pdf", "");
        let b = effect_key("\"Yes\" confirms replacing an existing file", "A file named \"a.pdf\" already exists.");
        assert_eq!(a, b);
    }

    #[test]
    fn moving_existing_data_needs_the_words() {
        let r = "`mv` would move the existing instructor-embedding away (to instructor-embedding.KEEP.bak); it is no longer where it was";
        assert_eq!(hazard_of(r), "move");
        assert!(!authorized(r, "Please help me clone the repo \"https://github.com/xlang-ai/instructor-embedding\" to /home/user.", "", "", &[]).0);
        assert!(authorized(r, "I have a directory named \"todo_list_Jan_1\". Can you help me change its name into \"todo_list_Jan_2\"?", "", "", &[]).0);
        assert!(authorized(r, "Rename the folder a to b", "", "", &[]).0);
        assert!(authorized(r, "把 a 文件夹重命名为 b", "", "", &[]).0);
        assert!(!authorized(r, "clone it but do not move anything", "", "", &[]).0);
        assert!(alternative(r).starts_with("do not move"));
        assert!(alternative("`mv` would overwrite existing file(s): a").starts_with("do not overwrite"));
    }
}
