//! 0.9.7 optional model roles inside the gate (rule change 2026-10-22 18:56: extra roles are allowed where they are
//! the better design). Each role is OFF unless its endpoint is configured (GUI_GATE_ROLE_<NAME>_URL/_MODEL/_KEY|_KEY_FILE
//! [/_PROTOCOL/_TIMEOUT/_MAX_TOKENS/_MAX_STEPS], or GUI_GATE_ROLE_<NAME>=reviewer) AND the feature that uses it is on.
//! Roles never act on the desktop: the probing roles (feasibility, planner, verifier) may run READ-ONLY probes
//! (an allowlist of inspection commands, file reads, the window list); the others only read what the gate hands them.
//! The reviewer stays the only authority for risky steps; no role can allow a risky step, and the acting model is
//! never asked to approve its own step.
//!
//! Wire format (works on every protocol the reviewer client speaks, no native tool calling needed): each turn the
//! role answers with ONE JSON object - a probe `{"probe":"sh","command":"..."}` / `{"probe":"read","path":"..."}` /
//! `{"probe":"windows"}`, or its final answer `{"final":{...}}`.
use crate::reviewer::Reviewer;
use crate::util::*;
use regex::Regex;
use serde_json::{json, Value};
use std::sync::OnceLock;

pub const ROLES: &[&str] = &["feasibility", "planner", "verifier", "judge", "search", "compact", "grounder"];

pub struct Role {
    pub name: String,
    pub llm: Reviewer,
    pub max_steps: usize,
}

impl Role {
    pub fn from_env(name: &str) -> Option<Role> {
        let llm = Reviewer::for_role(name)?;
        let max_steps = env(&format!("GUI_GATE_ROLE_{}_MAX_STEPS", name.to_uppercase())).and_then(|v| v.trim().parse().ok()).unwrap_or(8usize).clamp(1, 30);
        Some(Role { name: name.to_string(), llm, max_steps })
    }
    pub fn backend(&self) -> String {
        self.llm.backend().unwrap_or_default()
    }
}

/// All configured roles (whether a feature uses one is decided where it is used).
pub struct Roles {
    pub map: std::collections::HashMap<String, Role>,
}

impl Roles {
    pub fn from_env() -> Roles {
        Roles { map: ROLES.iter().filter_map(|r| Role::from_env(r).map(|x| (r.to_string(), x))).collect() }
    }
    pub fn get(&self, name: &str) -> Option<&Role> {
        self.map.get(name)
    }
    pub fn configured(&self) -> Vec<String> {
        let mut v: Vec<String> = self.map.iter().map(|(k, r)| format!("{k}={}", r.backend())).collect();
        v.sort();
        v
    }
}

// ------------------------------------------------------------------ read-only probes

fn safe_redirect_rx() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\s*(?:[0-9&]?>>?\s*/dev/null|[12]>&[12])").unwrap())
}
fn py_write_rx() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"(?x)open\s*\([^)]*,\s*['"][^'"]*[wax+]|\.open\s*\(\s*['"][^'"]*[wax+]|mode\s*=\s*['"][^'"]*[wax+]|makedirs|symlink|\.link\(|truncate|os\.open|ctypes|importlib|getattr\s*\(|setattr|\.copy\w*\(|\.chown|utime|\.kill\(|signal\.|\.write\(|\.save\(|\.unlink\(|\.rename\(|\.replace\(\s*['"/]|\.mkdir\(|\.touch\(|\.rmdir\(|remove\(|rmtree|shutil\.|os\.system|subprocess|popen|exec\(|eval\(|__import__|to_csv|to_excel|\.chmod\(|socket|requests\.|urllib|http\.client|write_text|write_bytes"#).unwrap())
}

const RO_CMDS: &[&str] = &["ls", "cat", "head", "tail", "stat", "file", "which", "whereis", "type", "find", "grep", "egrep", "fgrep",
    "wc", "du", "df", "dpkg-query", "echo", "printf", "pwd", "id", "whoami", "uname", "lsb_release", "printenv", "readlink",
    "realpath", "basename", "dirname", "sort", "uniq", "cut", "tr", "lspci", "lsusb", "lsblk", "fc-list", "identify", "ffprobe",
    "exiftool", "zipinfo", "jq", "md5sum", "sha256sum", "xxd", "od", "strings", "tree", "getent", "ps", "pgrep", "xprop", "xwininfo",
    "hostname", "date", "locale", "test", "[", "true", "false", "column", "nl", "comm", "diff", "cmp", "less", "more", "xdpyinfo",
    "pdfinfo", "lsof", "free", "nproc", "uptime", "env"];

/// Positional (non-option) arguments of a command line, after the command word.
fn positionals(argv: &[String]) -> Vec<&String> {
    argv[1..].iter().filter(|a| !a.starts_with('-') || a.as_str() == "-").collect()
}

/// Commands on the read-only list that can still write a file or change the machine with some arguments.
fn ro_cmd_writes(w: &str, argv: &[String]) -> Option<&'static str> {
    let has = |f: &[&str]| argv[1..].iter().any(|a| f.iter().any(|x| a == x || (x.starts_with("--") && a.starts_with(&format!("{x}=")))));
    let starts = |p: &str| argv[1..].iter().any(|a| a.starts_with(p) && a.len() > p.len());
    match w {
        "sort" if has(&["-o", "--output"]) || starts("-o") => Some("sort -o writes a file"),
        "uniq" if positionals(argv).len() >= 2 => Some("uniq IN OUT writes a file"),
        "xxd" if has(&["-r", "-revert"]) || positionals(argv).len() >= 2 => Some("xxd -r / xxd IN OUT writes"),
        "tree" if has(&["-o"]) => Some("tree -o writes a file"),
        "file" if has(&["-C", "--compile"]) => Some("file -C writes a magic file"),
        "exiftool" if argv[1..].iter().any(|a| (a.starts_with('-') && a.contains('=')) || ["-o", "-out", "-w", "-overwrite_original", "-overwrite_original_in_place", "-delete_original", "-tagsFromFile"].contains(&a.as_str())) => Some("exiftool would write tags or files"),
        "date" if has(&["-s", "--set"]) || positionals(argv).iter().any(|a| !a.starts_with('+')) => Some("date would set the clock"),
        "hostname" if !positionals(argv).is_empty() || has(&["-F", "--file", "-b", "--boot"]) => Some("hostname NAME would rename the machine"),
        "less" | "more" => Some("pagers can run commands; use cat / head"),
        "lsblk" | "lsof" | "ps" | "find" => None,
        _ => None,
    }
}

/// None when `cmd` is a read-only inspection command line the probing roles may run; else why not.
pub fn readonly_problem(cmd: &str) -> Option<String> {
    let c = safe_redirect_rx().replace_all(cmd, " ").to_string();
    if c.contains('>') || c.contains("<<") || c.contains("$(") || c.contains('`') || c.contains("<(") {
        return Some("redirection, heredocs and command substitution are not allowed in a probe".into());
    }
    if c.contains('\n') {
        return Some("one command line per probe".into());
    }
    let toks = crate::sidefx::tokenize(&c).ok_or_else(|| "unbalanced quotes".to_string()).err();
    if let Some(e) = toks {
        return Some(e);
    }
    let toks = crate::sidefx::tokenize(&c).unwrap_or_default();
    let mut cmds: Vec<Vec<String>> = vec![vec![]];
    for t in toks {
        if ["|", "||", "&&", ";"].contains(&t.as_str()) {
            cmds.push(vec![]);
        } else if t == "&" || t == "(" || t == ")" {
            return Some("background jobs and subshells are not allowed in a probe".into());
        } else {
            cmds.last_mut().unwrap().push(t);
        }
    }
    for argv in cmds.iter().filter(|a| !a.is_empty()) {
        let w = crate::sidefx::basename(&argv[0]);
        let sub = argv.get(1).map(|s| s.as_str()).unwrap_or("");
        let ok = match w.as_str() {
            w if RO_CMDS.contains(&w) => {
                if w == "find" && argv.iter().any(|a| ["-delete", "-exec", "-execdir", "-ok", "-okdir", "-fprint", "-fprintf", "-fls", "-fprint0"].contains(&a.as_str())) {
                    return Some("find with -delete/-exec/-fprint is not read-only".into());
                }
                if w == "env" && argv.len() > 1 {
                    return Some("env only without arguments (it would run a command)".into());
                }
                if let Some(why) = ro_cmd_writes(w, argv) {
                    return Some(format!("{why}: not read-only"));
                }
                true
            }
            "dpkg" => matches!(sub, "-l" | "-L" | "-s" | "--list" | "--status" | "--listfiles" | "-S" | "--search"),
            "apt" | "apt-cache" => matches!(sub, "list" | "show" | "policy" | "search" | "depends" | "madison"),
            "snap" | "flatpak" => matches!(sub, "list" | "info"),
            "gsettings" => matches!(sub, "get" | "list-keys" | "list-schemas" | "list-recursively" | "range" | "describe" | "list-children"),
            "dconf" => matches!(sub, "read" | "list" | "dump"),
            "timedatectl" => matches!(sub, "" | "status" | "show" | "list-timezones"),
            "systemctl" => matches!(sub, "status" | "is-active" | "is-enabled" | "list-units" | "show" | "list-unit-files"),
            "bluetoothctl" => matches!(sub, "show" | "list" | "devices" | "info"),
            "rfkill" => matches!(sub, "" | "list"),
            "xrandr" => matches!(sub, "" | "-q" | "--query" | "--current" | "--listmonitors"),
            "wmctrl" => argv[1..].iter().all(|a| matches!(a.as_str(), "-l" | "-lx" | "-lp" | "-lG" | "-m" | "-d" | "-x" | "-p" | "-G")),
            "xdg-mime" => sub == "query",
            "pactl" => matches!(sub, "list" | "info"),
            "unzip" => matches!(sub, "-l" | "-p" | "-v" | "-Z" | "-lv"),
            "pdftotext" => argv.last().map(|a| a == "-").unwrap_or(false),
            "sqlite3" => argv.iter().any(|a| a == "-readonly") && !c.to_lowercase().contains(".output") && !c.to_lowercase().contains(".shell")
                && !Regex::new(r"(?i)\b(insert|update|delete|drop|create|alter|attach|replace|vacuum|pragma\s+\w+\s*=)\b").unwrap().is_match(&c),
            "python3" | "python" => {
                if sub != "-c" || argv.len() < 3 {
                    return Some("python only as `python3 -c CODE` in a probe".into());
                }
                if py_write_rx().is_match(&argv[2]) {
                    return Some("the python code may write, delete, run commands or use the network".into());
                }
                true
            }
            "soffice" | "libreoffice" | "gimp" | "vlc" | "google-chrome" | "chromium" | "chromium-browser" | "thunderbird" | "code" | "firefox" | "inkscape" | "audacity" =>
                argv[1..].iter().all(|a| matches!(a.as_str(), "--version" | "-v" | "-version")) && argv.len() == 2,
            _ => false,
        };
        if !ok {
            return Some(format!("`{}` is not on the read-only probe list", trunc(&argv.join(" "), 60)));
        }
    }
    if crate::command::command_risk(cmd, &home()).is_some() {
        return Some("the gate's risk check flags this command".into());
    }
    None
}

/// Run one probe (read-only), capped output.
pub fn run_probe(p: &Value, windows: &dyn Fn() -> String) -> String {
    match s(p, "probe") {
        "sh" | "bash" | "shell" => {
            let cmd = pystr(p.get("command"));
            if let Some(why) = readonly_problem(&cmd) {
                return format!("refused: {why}");
            }
            let r = crate::command::run(&cmd, &home(), 15.0);
            let mut out = crate::command::tail_cap(&r.stdout, 3000);
            if !r.stderr.trim().is_empty() {
                out.push_str(&format!("\n[stderr] {}", crate::command::tail_cap(&r.stderr, 800)));
            }
            format!("exit {}{}\n{}", r.code.map(|c| c.to_string()).unwrap_or_else(|| "killed".into()), if r.timed_out { " (timed out)" } else { "" }, out)
        }
        "read" | "file" => {
            let path = crate::sidefx::expanduser(&pystr(p.get("path")));
            crate::inspect::read_file_text(&path, 4000)
        }
        "windows" => windows(),
        other => format!("unknown probe {other:?}: use sh, read or windows"),
    }
}

const PROBE_PROTOCOL: &str = "\n\nHOW TO ANSWER: every reply is ONE JSON object and nothing else.\n- To probe (read-only): {\"probe\":\"sh\",\"command\":\"ls -la ~/Desktop\"} (one command line from: ls cat head tail stat file which find grep wc dpkg -l dpkg-query apt list/show snap list gsettings get dconf read timedatectl systemctl status rfkill list bluetoothctl show xrandr wmctrl -l lspci lsusb ffprobe identify pdftotext FILE - unzip -l/-p python3 -c (read-only code) APP --version; pipes allowed, no redirection), {\"probe\":\"read\",\"path\":\"/path/file\"}, or {\"probe\":\"windows\"} (open windows).\n- To finish: {\"final\": <your answer object as described above>}.\nProbe results come back in the next message. Probe only what you need.";

/// The probing loop: the role probes read-only until it gives `{"final": ...}` (or the step budget runs out, then it
/// is asked for its final answer once more). Returns (final object, probe transcript, model calls, error).
pub fn probe_agent(role: &Role, system: &str, task_prompt: &str, windows: &dyn Fn() -> String, probes_allowed: bool) -> (Option<Value>, Vec<Value>, usize, Option<String>) {
    let sys = if probes_allowed { format!("{system}{PROBE_PROTOCOL}") } else { format!("{system}\n\nAnswer with ONE JSON object {{\"final\": ...}} and nothing else.") };
    let mut log: Vec<Value> = vec![];
    let mut calls = 0;
    let steps = if probes_allowed { role.max_steps } else { 0 };
    for step in 0..=steps {
        let mut prompt = task_prompt.to_string();
        if !log.is_empty() {
            prompt.push_str("\n\nPROBES SO FAR:\n");
            let mut body = String::new();
            for (i, e) in log.iter().enumerate() {
                body.push_str(&format!("#{} {} -> {}\n", i + 1, trunc(&e["probe"].to_string(), 300), e["result"].as_str().unwrap_or("")));
            }
            prompt.push_str(&tail(&body, 14000));
        }
        if step == steps {
            prompt.push_str("\n\nNo probes left: give your final answer now as {\"final\": ...}.");
        }
        calls += 1;
        let Some(txt) = role.llm.ask(&sys, &prompt, None) else {
            return (None, log, calls, role.llm.take_error().or(Some("no answer".into())));
        };
        let Some(v) = first_json(&txt) else {
            log.push(json!({"probe": "(unparsable reply)", "result": format!("reply was not one JSON object: {}", trunc(&txt, 200))}));
            continue;
        };
        if let Some(f) = v.get("final") {
            return (Some(f.clone()), log, calls, None);
        }
        if v.get("probe").is_some() && probes_allowed && step < steps {
            let r = run_probe(&v, windows);
            log.push(json!({"probe": v, "result": r}));
            continue;
        }
        // a bare answer object without the {"final":...} wrapper is accepted as final
        if v.get("probe").is_none() {
            return (Some(v), log, calls, None);
        }
    }
    (None, log, calls, Some("no final answer within the step budget".into()))
}

/// One call with an image (grounder): returns (final, [], calls, error).
pub fn probe_agent_image(role: &Role, system: &str, prompt: &str, image_b64: &str) -> (Option<Value>, Vec<Value>, usize, Option<String>) {
    let sys = format!("{system}\n\nAnswer with ONE JSON object {{\"final\": ...}} and nothing else.");
    match role.llm.ask(&sys, prompt, Some(image_b64)) {
        None => (None, vec![], 1, role.llm.take_error().or(Some("no answer".into()))),
        Some(t) => match first_json(&t) {
            Some(v) => (Some(v.get("final").cloned().unwrap_or(v)), vec![], 1, None),
            None => (None, vec![], 1, Some(format!("unparsable reply: {}", trunc(&t, 120)))),
        },
    }
}

/// The first balanced JSON object in a reply (fenced or not).
pub fn first_json(txt: &str) -> Option<Value> {
    let b: Vec<char> = txt.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '{' {
            let (mut depth, mut instr, mut esc) = (0i32, false, false);
            for j in i..b.len() {
                let c = b[j];
                if instr {
                    if esc { esc = false; } else if c == '\\' { esc = true; } else if c == '"' { instr = false; }
                    continue;
                }
                match c {
                    '"' => instr = true,
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            let cand: String = b[i..=j].iter().collect();
                            if let Ok(v) = serde_json::from_str::<Value>(&cand) {
                                return Some(v);
                            }
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
        i += 1;
    }
    None
}

// ------------------------------------------------------------------ role prompts (after Pointer / II / VLAA / GTA1)

pub const FEASIBILITY_SYSTEM: &str = "You are a Feasibility Gate. Decide whether a task CAN be completed on this Linux desktop by a GUI agent (which can also run terminal commands and Python) before it starts. You do NOT do the task; you only probe read-only and decide.\nMANDATORY CHECKS: (1) VERIFY RESOURCES: files, folders, apps, hardware and settings the task names exist (ls, find, which, dpkg -l, /sys/class/power_supply, /sys/class/backlight, /dev/video*, rfkill list...). (2) PARSE CONSTRAINTS: \"without extensions\", \"only with APP\", \"without X\" - does the constraint remove every viable route? (3) CHECK A GUI PATH: even if the feature nominally exists, is there a real path from the current state to the requested end state?\nINFEASIBLE when ANY holds: the app lacks the feature in its standard installation (needs third-party extensions/plugins/hacks); required files/data/hardware do not exist and cannot reasonably be obtained; the task's own constraints block every route; the thing asked for does not exist (a version never released, a setting that is not there); it would need a creative workaround (creating fake files, symlinks, installing unrequested extensions). If you catch yourself inventing a workaround, it is infeasible. Otherwise FEASIBLE - do not cry wolf: an ordinary task in an installed app is feasible.\nFINAL ANSWER: {\"feasible\": true|false, \"reason\": \"<two or three sentences citing probe evidence>\", \"evidence\": [\"<short probe finding>\", ...], \"notes\": [\"<facts the executor should know: exact paths, app versions, which dialog/menu has the feature>\"]}";

pub const PLANNER_SYSTEM: &str = "You plan tasks for an executor on a Linux desktop. Output a short list of MILESTONES: each describes a STATE the world should be in (not clicks). Good: \"/home/user/Desktop/report.xlsx has the totals in column F and is saved\". Bad: \"click Data > Sort\". Each milestone names the concrete artifact / setting / app window, is verifiable on its own, and leaves the mechanism open. As many as the task needs (often 2-5). If a GUI app holds the work, end with a milestone like \"the app shows the finished work and the file on disk matches it (saved)\". A file edited in code while open in its app must be closed/reloaded - say so in the target state. You may attach a machine-checkable predicate to a milestone: {\"file_exists\":P} {\"file_changed\":P} {\"file_contains\":[P,TEXT]} {\"title_contains\":TEXT} {\"window_open\":TEXT} {\"app_running\":NAME} {\"terminal_contains\":TEXT} {\"cell\":[\"B3\",\"VALUE\"]} {\"not_title_contains\":TEXT}.\nFINAL ANSWER: {\"milestones\": [{\"state\": \"...\", \"check\": {..} or null}, ...], \"infeasible_reason\": \"\" or \"<why it looks impossible>\"}";

pub const VERIFIER_SYSTEM: &str = "You verify whether a Linux desktop's END STATE satisfies a task. You never review the agent's actions; you probe the machine read-only and decide. Graders read files on disk, settings and app state, so verify the same way: a file with the right content, a settings key with the right value, a command output. Be stringent - reject on sight: WRONG TARGET (right-looking content at another path or in a new file instead of the named one); SURFACE LOOKALIKE (plain cells instead of a real pivot/chart/style; a config stub that does not change the app); SUBSTITUTE for something that does not exist (then the verdict is infeasible, not done); WRONG KEY/SCOPE; UNUSED PRE-PROVIDED ARTIFACT (the given file untouched, a new file holds the work); PARTIAL COMPLETION (one of five items). For a GUI app check BOTH the saved file AND the live state (window title without unsaved marker, the evidence the gate gives you); unsaved work is not done (OSWorld's checker may press Ctrl+S in the open window itself - if the task's document is open and its in-app state is right, say so).\nDECISIONS: done = every requirement verified by at least one probe on the exact named target; failed = state does not match (give diagnosis = actual vs required, and suggestion = a concrete DIFFERENT approach, never \"try again\"); infeasible = the capability genuinely does not exist here, confirmed by probing.\nFINAL ANSWER: {\"decision\": \"done\"|\"failed\"|\"infeasible\", \"reasoning\": \"...\", \"diagnosis\": \"...\", \"suggestion\": \"...\"}";

pub const JUDGE_LOOP_SYSTEM: &str = "You judge whether a GUI agent is stuck. You see the task, its recent steps with their outcomes and the gate's loop counters. Answer KEEP when the recent steps are making real progress or the repetition is intended (e.g. the same menu item for the next paragraph); SWITCH when the agent repeats a failing mechanism. With SWITCH name the step to stop repeating and a genuinely different approach (keyboard shortcut vs menu vs element click vs code/run_command vs asking the person vs declaring infeasible).\nFINAL ANSWER: {\"verdict\": \"KEEP\"|\"SWITCH\", \"blacklist\": \"<the repeated step, short>\", \"advice\": \"<one sentence>\"}";

pub const JUDGE_PICK_SYSTEM: &str = "You pick the best next action for a GUI agent among candidates it proposed. You see the task, the current window (accessibility text), recent steps, and each candidate with the gate's estimate of its chance to work. Prefer the candidate most likely to move the task forward safely; ties go to the higher estimate. You cannot invent new actions.\nFINAL ANSWER: {\"pick\": <candidate number, 1-based>, \"reason\": \"<one sentence>\"}";

pub const SEARCH_SYSTEM: &str = "You answer a GUI agent's \"how do I ... in APP\" question with a short, practical tutorial as plain text: the exact menu path / dialog / setting names / keyboard shortcuts for the app version on a Linux desktop (LibreOffice, Chrome, GIMP, VLC, Thunderbird, VS Code, the Xfce/GNOME desktop). Mention when the feature does not exist in the app (that makes the task infeasible). If you have web search, use it. At most 12 lines.\nFINAL ANSWER: {\"answer\": \"<tutorial text>\", \"exists\": true|false}";

pub const COMPACT_SYSTEM: &str = "You are a concise summarizer of an AI agent's task log on a desktop. Produce a structured digest: '## Task' one sentence; '## Done' each step as '- tool: <max 50 char result>' (ERROR: prefix when it failed), merging repeats; '## Facts' values / paths the agent learned; '## State' one sentence on current progress and what is next.\nFINAL ANSWER: {\"digest\": \"<markdown, at most 25 lines>\"}";

pub const GROUNDER_SYSTEM: &str = "You locate one UI element in a screenshot of a desktop window for a GUI agent. Return the pixel coordinates (in the screenshot's own pixel space) of the CENTER of the element that matches the description. If it is not visible, say so.\nFINAL ANSWER: {\"found\": true|false, \"x\": <int>, \"y\": <int>, \"why\": \"<short>\"}";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readonly_probe_allowlist() {
        for ok in ["ls -la ~/Desktop", "ls /sys/class/power_supply 2>/dev/null", "find ~ -name '*.xlsx' -mmin -60", "which gimp && gimp --version",
                   "dpkg -l | grep -i vlc", "gsettings get org.gnome.desktop.interface color-scheme", "cat /etc/os-release | head -3",
                   "python3 -c 'import openpyxl; print(openpyxl.load_workbook(\"a.xlsx\").sheetnames)'", "rfkill list", "wmctrl -lx",
                   "pdftotext a.pdf -", "unzip -p a.docx word/document.xml | head -c 2000", "test -e ~/x.txt && echo yes", "sqlite3 -readonly db 'select 1'"] {
            assert_eq!(readonly_problem(ok), None, "{ok}");
        }
        for bad in ["rm -rf ~/x", "echo hi > ~/a.txt", "cp a b", "find . -delete", "find . -exec chmod 644 {} +", "python3 -c 'open(\"a\",\"w\").write(1)'",
                    "python3 script.py", "bash -c ls", "sudo ls", "ls $(whoami)", "sleep 100 &", "gsettings set a b c", "pdftotext a.pdf out.txt",
                    "dconf write /a 1", "sqlite3 db 'delete from t'", "systemctl restart x", "tee a < b", "apt install x", "env rm x", "curl http://x"] {
            assert!(readonly_problem(bad).is_some(), "{bad}");
        }
    }

    #[test]
    fn first_json_extracts_objects() {
        assert_eq!(first_json("sure ```json\n{\"final\": {\"feasible\": false, \"reason\": \"no {x}\"}}\n```").unwrap()["final"]["feasible"], json!(false));
        assert_eq!(first_json("{bad} then {\"probe\":\"sh\",\"command\":\"ls\"}").unwrap()["probe"], json!("sh"));
        assert!(first_json("no json").is_none());
    }
}
