//! Authorization check: does the user's own request cover this risky action?
//!
//! Only what the user explicitly asked for is authorized; anything the agent chose on its own is not.
//! Deterministic, no model. Two ways an action becomes authorized:
//!  * working-document exception: replacing the very file that is open in this app;
//!  * explicit wording: the task names the hazard and, when the action has a concrete file target, that file.
//! Also: a hazard class per risk reason, to pick a safe alternative when the action is denied
//! (deny-and-continue) and to remember denied effects.
use crate::effect::{digest, EffectKey};
use crate::features::ActionCtx;
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
    } else if r.contains("typed a dangerous command") || r.contains("enter would run") || r.contains("would run:") || r.contains("could not parse") {
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

/// Path-like words of the request ("~/a/report.txt", "/tmp/x/a", "Desktop/report.pdf"), quotes and trailing
/// punctuation stripped, URLs skipped.
fn path_words(task: &str) -> Vec<String> {
    task.split(|c: char| c.is_whitespace() || "\"'“”‘’`,;()<>[]{}".contains(c))
        .map(|w| w.trim_end_matches(|c: char| ".:!?。，；：".contains(c)).to_string())
        .filter(|w| w.contains('/') && !w.contains("://") && w.len() > 1)
        .collect()
}

/// Is this file resource named by the request? (covered?, why-not)
fn file_named(res: &str, task_l: &str, cwd: Option<&str>) -> Result<(), String> {
    let (kind, val) = res.split_once(':').unwrap_or(("", res));
    let base = val.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_lowercase();
    if base.is_empty() {
        return Err(format!("cannot tell which file {val} is"));
    }
    let stem_ok = |t: &str| {
        let ext = splitext_ext(&base);
        let stem = &base[..base.len() - ext.len()];
        t.contains(&base) || (stem.chars().count() >= 3 && !ext.is_empty() && t.contains(stem))
    };
    if kind == "path" {
        // a path in the request with this basename must be this very path (the same name elsewhere is another file)
        let anchor = cwd.map(String::from).unwrap_or_else(crate::util::home);
        let mut same_name = vec![];
        for w in path_words(task_l) {
            if w.trim_end_matches('/').rsplit('/').next().unwrap_or("") != base {
                continue;
            }
            let abs = crate::sidefx::canon(&crate::sidefx::p(&w, &anchor)).to_lowercase();
            let rel_tail = format!("/{}", w.trim_start_matches("./").trim_end_matches('/'));
            if abs == val.to_lowercase() || (!crate::sidefx::is_abs(&w) && !w.starts_with('~') && val.to_lowercase().ends_with(&rel_tail)) {
                return Ok(());
            }
            same_name.push(w);
        }
        if let Some(w) = same_name.first() {
            return Err(format!("the request names {w}, which is not {val} (same name, another folder)"));
        }
    }
    if !stem_ok(task_l) {
        return Err(format!("the request does not name {base}"));
    }
    // 0.10.1: the request names the file only by its name. That covers exactly one candidate: the file of that name
    // in the working directory (the command's cwd / the terminal's current directory). The same name elsewhere is
    // another file the user may not have meant, so it goes to the reviewer / a person instead of being authorized.
    if kind == "path" {
        if let Some(c) = cwd {
            let dir = crate::sidefx::canon(c).trim_end_matches('/').to_lowercase();
            let parent = val.trim_end_matches('/').rsplit_once('/').map(|x| x.0).unwrap_or("").to_lowercase();
            let parent = if parent.is_empty() { "/".to_string() } else { parent };
            if parent != dir && !(dir.is_empty() && parent == "/") {
                return Err(format!("the request names only the file name {base}, and the file is in {parent}, not in the working directory {dir} (the same name may mean another file)"));
            }
        }
    }
    Ok(())
}

/// Two resources of one effect with the same file name in different folders: a name-only request is ambiguous.
fn same_name_elsewhere(files: &[&str]) -> Option<String> {
    let mut seen: std::collections::HashMap<String, String> = Default::default();
    for f in files {
        let (kind, v) = f.split_once(':').unwrap_or(("", *f));
        if kind != "path" {
            continue;
        }
        let v = v.trim_end_matches('/');
        let base = v.rsplit('/').next().unwrap_or("").to_lowercase();
        if let Some(other) = seen.get(&base) {
            if other != v {
                return Some(base);
            }
        }
        seen.insert(base, v.to_string());
    }
    None
}

/// (authorized?, why). Only the operator's request (--task / GUI_GATE_TASK) is passed here; what the agent stated
/// itself never is. Authorized only when the request affirms the hazard AND names every file the structured effect
/// acts on (a path the request spells out must be this very path). An effect without a file the gate can match
/// (a "Send" button, a "Delete" control, a command without a resolvable target) is never covered by wording alone:
/// the reviewer (medium risk) or a person (high risk) decides. The working-document exception (replacing the file that
/// is open in this very app) needs no wording.
pub fn authorized(key: &EffectKey, task: &str, titles: &[String], cwd: Option<&str>) -> (bool, String) {
    let hz = key.hazard.as_str();
    if hz == "overwrite" {
        for r in key.files() {
            let v = r.split_once(':').map(|x| x.1).unwrap_or(r);
            if is_open_document(Some(v), titles) {
                return (true, format!("replaces the document that is open in this app ({})", basename(v)));
            }
        }
    }
    let t = norm(task);
    if t.is_empty() {
        return (false, "no user request from the operator (--task) is known to the gate".into());
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
    let files = key.files();
    if files.is_empty() {
        return (false, format!("the request says \"{hit}\" but the gate cannot match what this action acts on to the request (no file it can resolve), so the words alone do not cover it"));
    }
    if let Some(b) = same_name_elsewhere(&files) {
        // only refuse the wording when the request does not spell out each path
        let spelled = files.iter().all(|f| path_words(&t).iter().any(|w| { let v = f.split_once(':').map(|x| x.1).unwrap_or(f); v.to_lowercase().ends_with(&format!("/{}", w.trim_start_matches("./").trim_start_matches('~').trim_start_matches('/'))) }));
        if !spelled {
            return (false, format!("the request says \"{hit}\" but {b} names files in more than one folder here; the name alone does not say which"));
        }
    }
    for f in &files {
        if let Err(why) = file_named(f, &t, cwd) {
            return (false, format!("the request says \"{hit}\" but {why}"));
        }
    }
    let names: Vec<String> = files.iter().take(3).map(|f| basename(f.split_once(':').map(|x| x.1).unwrap_or(f))).collect();
    (true, format!("the request explicitly says \"{hit}\" and names {}", names.join(", ")))
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
    if reason.starts_with("`soffice") {
        return "do not overwrite it: convert into a new folder (`--outdir` a directory that has no file of that name) or copy the source to a new name first, finish the task with that file, and mention both paths in your final answer";
    }
    if (reason.starts_with("`cp`") || reason.starts_with("`mv`") || reason.contains("would overwrite existing file")) && !reason.contains("would move the existing") {
        return ALTERNATIVES_CP;
    }
    alternative_for(hazard_of(reason))
}

/// The structured effect of a risky (or hand-off) action: hazard from the rule that fired, resources from the disk
/// checks (`targets`, canonical paths), else the file the dialog's own words name, else the control or command.
/// `line`: in a terminal, the command line Enter would submit (part of the payload).
pub fn effect_of(ctx: &ActionCtx, targets: &[String], dialog_text: &str, line: Option<&str>) -> EffectKey {
    let reason = ctx.risk.clone().or(ctx.handoff.clone()).unwrap_or_default();
    let hazard = if ctx.risk.is_none() && ctx.handoff.is_some() { "secret" } else { hazard_of(&reason) };
    let mut resources: Vec<String> = targets.iter().map(|t| format!("path:{t}")).collect();
    if resources.is_empty() {
        if let Some(tf) = target_file(&reason, dialog_text) {
            if crate::sidefx::is_abs(&tf) {
                resources.push(format!("path:{}", crate::sidefx::canon(&tf)));
            } else {
                resources.push(format!("name:{}", basename(&tf).to_lowercase()));
            }
        }
    }
    let cmd = if ctx.tool == "run_command" { crate::util::pystr(ctx.args.get("command")) } else { String::new() };
    if resources.is_empty() {
        let t = &ctx.target;
        if ctx.tool == "run_command" {
            resources.push(format!("cmd:{}", digest(&norm(&cmd))));
        } else if let Some(l) = line.filter(|l| !l.trim().is_empty()) {
            resources.push(format!("cmd:{}", digest(&norm(l))));
        } else if !t.the_label().is_empty() || !t.the_role().is_empty() {
            resources.push(format!("elem:{}|{}", norm(t.the_role()), norm(t.the_label())));
        } else {
            resources.push(format!("none:{}|{}", ctx.tool, norm(&reason)));
        }
    }
    resources.sort();
    resources.dedup();
    let a = &ctx.args;
    let typed = if crate::util::truthy(a.get("text")) { crate::util::pystr(a.get("text")) } else { crate::util::pystr(a.get("value")) };
    let keys = a.get("keys").map(|k| k.to_string()).unwrap_or_default() + &crate::util::pystr(a.get("key")) + &a.get("modifiers").map(|k| k.to_string()).unwrap_or_default();
    // a pass-through action (launch_app, a new driver tool): its whole argument object is the payload
    let extra = if crate::features::is_passthrough_action(&ctx.tool) { a.to_string() } else { String::new() };
    let payload_src = format!("{cmd}\u{1}{typed}\u{1}{keys}\u{1}{}\u{1}{}{extra}", line.unwrap_or(""), crate::util::pystr(a.get("cwd")));
    let payload = if payload_src.chars().all(|c| c == '\u{1}') { String::new() } else { digest(&payload_src) };
    EffectKey {
        tool: ctx.tool.clone(), hazard: hazard.to_string(), resources,
        app: if ctx.tool == "run_command" { None } else { ctx.target.pid },
        window: if ctx.tool == "run_command" { None } else { ctx.target.window_id },
        payload,
    }
}

/// The key of a command-line effect (the offline audits and tests use it): (level, reason, targets) from sidefx.
pub fn command_key(eff: &crate::sidefx::Effect, cmd: &str) -> EffectKey {
    let ctx = ActionCtx { tool: "run_command".into(), args: serde_json::json!({"command": cmd}), risk: Some(eff.1.clone()), risk_level: Some(eff.0.clone()), ..Default::default() };
    effect_of(&ctx, &eff.2, "", None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::Target;

    fn ctx(reason: &str, label: &str) -> ActionCtx {
        ActionCtx { tool: "click".into(), args: serde_json::json!({}), risk: Some(reason.into()), risk_level: Some("high".into()),
                    target: Target { kind: "token".into(), pid: Some(7), window_id: Some(3), label: label.into(), role: "button".into(), ..Default::default() },
                    ..Default::default() }
    }
    fn key(reason: &str, dialog: &str, targets: &[&str]) -> EffectKey {
        effect_of(&ctx(reason, ""), &targets.iter().map(|s| s.to_string()).collect::<Vec<_>>(), dialog, None)
    }
    fn auth(reason: &str, task: &str, targets: &[&str]) -> bool {
        authorized(&key(reason, "", targets), task, &[], None).0
    }

    #[test]
    fn working_document_and_wording() {
        let titles = vec!["report.pdf - Viewer".to_string()];
        let (ok, why) = authorized(&key("\"Yes\" confirms replacing an existing file", "A file named \"report.pdf\" already exists.", &[]), "", &titles, None);
        assert!(ok, "{why}");
        let r = "saving here would replace the existing file /home/u/Desktop/report.pdf";
        let t = &["/home/u/Desktop/report.pdf"];
        assert!(!auth(r, "Export the current document into PDF, keep the file name", t));
        assert!(auth(r, "Export it as PDF and overwrite report.pdf", t));
        assert!(auth(r, "Export it as PDF and overwrite ~/Desktop/report.pdf", t) || crate::util::home() != "/home/u");
        assert!(auth(r, "Export it as PDF and overwrite Desktop/report.pdf", t));
        assert!(!auth(r, "Export it as PDF and overwrite /home/u/Documents/report.pdf", t), "same name, another folder");
        assert!(!auth(r, "Export it as PDF to the Desktop", t));
        let cp = "`cp` would overwrite existing file(s): dir2/file1";
        assert!(!auth(cp, "Copies file 'file1' to each of directories 'dir1', 'dir2', 'dir3'.", &["/tmp/x/dir2/file1"]));
        assert!(alternative(cp).starts_with("do not overwrite it: copy"));
        assert!(!auth("\"Delete file\" (delete)", "", &[]));
    }

    #[test]
    fn wording_without_a_matchable_target_is_not_enough() {
        // a "Send"/"Delete" control: the gate cannot match its object to the request, so words alone never cover it
        let r = "\"Send\" (send)";
        assert!(!auth(r, "reply to Bob and send it", &[]));
        let (ok, why) = authorized(&key("\"Delete file\" (delete)", "", &[]), "Please delete the file in this window", &[], None);
        assert!(!ok);
        assert!(why.contains("cannot match"), "{why}");
        // a named file is covered
        assert!(auth("`rm` deletes report.txt", "delete report.txt", &["/tmp/w/a/report.txt"]));
        assert!(auth("`rm` deletes report.txt", "delete /tmp/w/a/report.txt", &["/tmp/w/a/report.txt"]));
        assert!(!auth("`rm` deletes report.txt", "delete /tmp/w/b/report.txt", &["/tmp/w/a/report.txt"]));
        // every file the effect touches must be named
        assert!(!auth("`rm` deletes a.txt b.txt", "delete a.txt", &["/tmp/w/a.txt", "/tmp/w/b.txt"]));
    }

    #[test]
    fn a_file_name_alone_covers_only_the_file_in_the_working_directory() {
        let k = key("`rm` deletes report.txt", "", &["/tmp/w/a/report.txt"]);
        assert!(authorized(&k, "delete report.txt", &[], Some("/tmp/w/a")).0, "the one in the working directory");
        let (ok, why) = authorized(&k, "delete report.txt", &[], Some("/tmp/w"));
        assert!(!ok, "same name in a subfolder is not what a name-only request covers");
        assert!(why.contains("working directory"), "{why}");
        assert!(authorized(&k, "delete a/report.txt", &[], Some("/tmp/w")).0, "a spelled-out relative path is exact");
        // the same name in two folders in one effect: ambiguous without paths
        let k2 = key("`rm` deletes report.txt", "", &["/tmp/w/a/report.txt", "/tmp/w/b/report.txt"]);
        assert!(!authorized(&k2, "delete report.txt", &[], None).0);
        assert!(!authorized(&k2, "delete report.txt", &[], Some("/tmp/w/a")).0);
        assert!(authorized(&k2, "delete /tmp/w/a/report.txt and /tmp/w/b/report.txt", &[], None).0);
        // a denial still covers by name (conservative)
        let d = key("\"Yes\" confirms replacing an existing file", "A file named \"report.txt\" already exists.", &[]);
        let k3 = key("saving here would replace the existing file /tmp/w/b/report.txt", "", &["/tmp/w/b/report.txt"]);
        assert!(d.denial_covers(&k3), "a name-only denial covers the same name in any folder");
    }

    #[test]
    fn negated_wording_does_not_authorize() {
        let t = &["/tmp/w/notes.txt"];
        let r = "`rm` deletes notes.txt";
        assert!(!auth(r, "tidy notes.txt, do not delete it", t));
        assert!(!auth(r, "Edit notes.txt but don't remove it", t));
        assert!(!auth(r, "整理 notes.txt，先不要删除", t));
        assert!(auth(r, "delete notes.txt", t));
        assert!(auth(r, "do not wait, delete notes.txt now", t));
        assert!(auth(r, "把 notes.txt 删除", t));
        assert!(!auth(r, "Rename notes.txt; never delete anything", t));
        // the clause logic itself
        assert!(affirmed_hit("send", "reply to bob and send it"));
        assert!(!affirmed_hit("send", "draft a reply to bob, do not send it yet"));
        assert!(!affirmed_hit("发送", "回复 bob，先不要发送"));
    }

    #[test]
    fn same_effect_across_routes() {
        let a = key("saving here would replace the existing file /home/u/a.pdf", "", &["/home/u/a.pdf"]);
        let b = key("\"Yes\" confirms replacing an existing file", "A file named \"a.pdf\" already exists.", &[]);
        assert!(a.denial_covers(&b) && b.denial_covers(&a));
        let c = key("saving here would replace the existing file /home/v/a.pdf", "", &["/home/v/a.pdf"]);
        assert!(!a.denial_covers(&c), "same basename in another folder");
    }

    #[test]
    fn moving_existing_data_needs_the_words() {
        let r = "`mv` would move the existing instructor-embedding away (to instructor-embedding.KEEP.bak); it is no longer where it was";
        assert_eq!(hazard_of(r), "move");
        let t = &["/home/user/instructor-embedding"];
        assert!(!auth(r, "Please help me clone the repo \"https://github.com/xlang-ai/instructor-embedding\" to /home/user.", t));
        let t2 = &["/tmp/w/todo_list_Jan_1"];
        assert!(auth(r, "I have a directory named \"todo_list_Jan_1\". Can you help me change its name into \"todo_list_Jan_2\"?", t2));
        assert!(auth(r, "Rename the folder todo_list_Jan_1 to b", t2));
        assert!(auth(r, "把 todo_list_Jan_1 文件夹重命名为 b", t2));
        assert!(!auth(r, "clone it but do not move anything", t2));
        assert!(alternative(r).starts_with("do not move"));
        assert!(alternative("`mv` would overwrite existing file(s): a").starts_with("do not overwrite"));
    }
}
