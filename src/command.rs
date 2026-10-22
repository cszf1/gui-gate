//! run_command: a gated command channel (bash with a cwd) so the agent can do terminal / file / spreadsheet work in
//! code instead of by clicking. Every command is judged by the same rules as a command typed into a terminal and
//! committed with Enter (sidefx::shell_effects against the disk, the dangerous-command patterns), plus checks on
//! the code it hands to an interpreter (heredoc bodies, `python3 -c`, a script file it runs). The decision path
//! (authorization by the user's words, the reviewer for medium risk, a person for high risk, deny-and-continue)
//! is the gate's usual one; this module only judges and executes.
use crate::sidefx;
use crate::util::*;
use regex::Regex;
use serde_json::{json, Value};
use std::io::Read;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime};

macro_rules! re {
    ($name:ident, $pat:expr) => {
        fn $name() -> &'static Regex {
            static R: OnceLock<Regex> = OnceLock::new();
            R.get_or_init(|| Regex::new($pat).unwrap())
        }
    };
}
// the terminal Enter rule (features::shell_high) without `rm`, which shell_effects judges by what it would delete
re!(shell_high_no_rm, r"(?i)(\bmkfs\b|\bdd\s+if=|\bshutdown\b|\breboot\b|:\(\)\s*\{|\bchmod\s+-R\s+777|\bdrop\s+(table|database)\b|\btruncate\s+table\b|\bdelete\s+from\b|git\s+push\s+.*--force|\bcurl\b[^|]*\|\s*(ba)?sh|\bwget\b[^|]*\|\s*(ba)?sh|\bsudo\b|>\s*/dev/sd|\bformat\s+[a-z]:)");
re!(shell_high_all, r"(?i)(\brm\s+-[a-z]*[rf]|\bmkfs\b|\bdd\s+if=|\bshutdown\b|\breboot\b|\bchmod\s+-R\s+777|\bdrop\s+(table|database)\b|\btruncate\s+table\b|\bdelete\s+from\b|git\s+push\s+.*--force|\bsudo\b|>\s*/dev/sd)");
re!(heredoc_start, r#"<<-?\s*(['"]?)([A-Za-z_][A-Za-z0-9_]*)['"]?"#);
// interpreter code that deletes or moves files (Python, Perl, Node, Ruby)
re!(code_delete, r"(?i)(\bos\.(remove|unlink|rmdir|removedirs|rename|replace)\s*\(|\bshutil\.(rmtree|move)\s*\(|\.unlink\s*\(|\.rmdir\s*\(|\.rename\s*\(|\.replace\s*\(\s*['\x22/]|\bfs\.(unlinkSync|rmSync|rmdirSync|renameSync|unlink|rm|rename)\s*\(|\bunlink\s*\(|\bFileUtils\.rm|\bsend2trash\b)");
re!(code_shell, r#"(?i)\b(os\.system|subprocess\.\w+|os\.popen|check_output|child_process|exec(Sync)?)\s*\("#);
// a string literal that looks like a file path with an extension: the write targets we can check on disk
re!(path_lit, r#"['"]((?:~|/|\.{1,2}/)?[^'"\n\\]{0,200}?\.[A-Za-z0-9]{1,6})['"]"#);
re!(write_call, r#"(?i)(\.save\s*\(|\.to_(csv|excel|json|parquet|html)\s*\(|\.write_(text|bytes)\s*\(|open\s*\([^)]*['"][wax]b?\+?['"]|\.saveas\s*\(|\.write\s*\(\s*['"]|savefig\s*\(|\.export\s*\(|wb\.close|writeFileSync|copyfile|copy2?\s*\()"#);

pub type Effect = (String, String);

const INTERPRETERS: &[&str] = &["python", "python3", "python2", "perl", "node", "ruby", "php"];

fn worse(a: Option<Effect>, b: Option<Effect>) -> Option<Effect> {
    match (&a, &b) {
        (None, _) => b,
        (_, None) => a,
        (Some(x), Some(y)) => if y.0 == "high" && x.0 != "high" { b } else { a },
    }
}

/// The shell part of a command (heredoc bodies cut out) and the heredoc bodies.
pub fn split_heredocs(cmd: &str) -> (String, Vec<String>) {
    let mut shell = vec![];
    let mut bodies = vec![];
    let mut lines = cmd.lines();
    while let Some(line) = lines.next() {
        shell.push(line.to_string());
        let tags: Vec<String> = heredoc_start().captures_iter(line).map(|c| c[2].to_string()).collect();
        for tag in tags {
            let mut body = vec![];
            for l in lines.by_ref() {
                if l.trim() == tag {
                    break;
                }
                body.push(l);
            }
            bodies.push(body.join("\n"));
        }
    }
    (shell.join("\n"), bodies)
}

/// Code handed to an interpreter in the shell part: `python3 -c CODE`, `perl -e CODE`, `node -e CODE`, and the
/// content of a script file it runs (`python3 x.py`, `bash x.sh`) when that file exists (<= 256 KB).
fn inline_code(shell: &str, cwd: &str) -> (Vec<String>, Vec<String>) {
    let mut code = vec![];
    let mut scripts = vec![];
    let src = shell.replace('\n', " ; ");
    let Some(toks) = sidefx::tokenize(&src) else { return (code, scripts) };
    let mut cwd_s = cwd.to_string();
    let mut i = 0;
    while i < toks.len() {
        let b = sidefx::basename(&toks[i]);
        let base = b.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
        let base = if base.is_empty() { b.as_str() } else { base };
        if toks[i] == "cd" {
            if let Some(d) = toks.get(i + 1) {
                let np = sidefx::p(d, &cwd_s);
                if sidefx::is_dir(&np) {
                    cwd_s = np;
                }
            }
        }
        let is_interp = INTERPRETERS.contains(&base) || INTERPRETERS.contains(&b.as_str());
        let is_shell = ["bash", "sh", "zsh", "dash"].contains(&b.as_str());
        if is_interp || is_shell {
            let mut j = i + 1;
            while j < toks.len() && toks[j].starts_with('-') && !["-c", "-e"].contains(&toks[j].as_str()) {
                j += 1;
            }
            match toks.get(j).map(|s| s.as_str()) {
                Some("-c") | Some("-e") if is_interp => {
                    if let Some(c) = toks.get(j + 1) {
                        code.push(c.clone());
                    }
                }
                Some(f) if !f.is_empty() && !";|&<>()".contains(&f[..1]) && f != "-" && f != "-c" => {
                    let path = sidefx::p(f, &cwd_s);
                    if let Ok(m) = std::fs::metadata(&path) {
                        if m.is_file() && m.len() <= 256 * 1024 {
                            if let Ok(t) = std::fs::read_to_string(&path) {
                                if is_shell { scripts.push(t) } else { code.push(t) }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    (code, scripts)
}

/// Existing files (non-empty, older than this gate session) that interpreter code names as a write target.
fn code_overwrites(code: &str, cwd: &str) -> Vec<String> {
    if !write_call().is_match(code) {
        return vec![];
    }
    let start = sidefx::session_start();
    let mut hit = vec![];
    for c in path_lit().captures_iter(code) {
        let lit = c[1].trim();
        if lit.contains("://") || lit.starts_with('.') && !lit.starts_with("./") && !lit.starts_with("../") {
            continue;
        }
        let path = sidefx::p(lit, cwd);
        if sidefx::nonempty_file(&path) && !sidefx::session_owned(&path, start) && !hit.contains(&path) {
            hit.push(path);
        }
    }
    hit
}

fn code_effects(code: &str, cwd: &str) -> Option<Effect> {
    let mut worst = None;
    if let Some(m) = shell_high_all().find(code) {
        worst = worse(worst, Some(("high".into(), format!("run_command would run: script code with a dangerous command ({})", trunc(m.as_str(), 30)))));
    }
    if let Some(m) = code_delete().find(code) {
        worst = worse(worst, Some(("medium".into(), format!("script code deletes or moves files ({})", trunc(m.as_str().trim(), 30)))));
    }
    if code_shell().is_match(code) {
        // a shell command inside the code: judge the string literals it passes as commands like typed ones
        for c in Regex::new(r#"['"]([^'"\n]{2,300})['"]"#).unwrap().captures_iter(code) {
            if let Some(e) = sidefx::shell_effects(&c[1], cwd) {
                worst = worse(worst, Some(e));
            }
        }
    }
    let ow = code_overwrites(code, cwd);
    if !ow.is_empty() {
        let names: Vec<String> = ow.iter().take(3).map(|x| sidefx::relpath(x, cwd)).collect();
        worst = worse(worst, Some(("medium".into(), format!("script code would overwrite existing file(s): {}", names.join(", ")))));
    }
    worst
}

/// (level, reason) for the worst effect of a run_command command line in `cwd`, or None.
pub fn command_risk(cmd: &str, cwd: &str) -> Option<Effect> {
    let (shell, bodies) = split_heredocs(cmd);
    let flat = shell.replace("\\;", " ").replace('\n', " ; ");
    if sidefx::tokenize(&flat).is_none() {
        return Some(("medium".into(), "run_command: the gate could not parse this command (unbalanced quotes), so it cannot see what it touches".into()));
    }
    let mut worst: Option<Effect> = None;
    if let Some(m) = shell_high_no_rm().find(&shell) {
        worst = worse(worst, Some(("high".into(), format!("run_command would run: {}", trunc(m.as_str(), 60)))));
    }
    worst = worse(worst, sidefx::shell_effects(&shell, cwd));
    let (code, scripts) = inline_code(&shell, cwd);
    for s in scripts {
        worst = worse(worst, command_risk(&s, cwd));
    }
    // heredoc bodies: shell bodies (`bash <<EOF`) are commands, the rest is interpreter code or file content
    for b in bodies.iter().chain(code.iter()) {
        worst = worse(worst, code_effects(b, cwd));
        if let Some(e) = sidefx::shell_effects(b, cwd).filter(|_| looks_like_shell(b)) {
            worst = worse(worst, Some(e));
        }
    }
    worst
}

fn looks_like_shell(body: &str) -> bool {
    let first = body.lines().map(|l| l.trim()).find(|l| !l.is_empty() && !l.starts_with('#')).unwrap_or("");
    let w = first.split_whitespace().next().unwrap_or("");
    ["cp", "mv", "rm", "find", "chmod", "rsync", "cd", "install", "soffice", "libreoffice", "xargs"].contains(&w)
}

pub fn enabled() -> bool {
    // bash only: the risk rules judge bash command lines, so Windows does not get the tool
    cfg!(unix) && std::env::var("GUI_GATE_RUN_COMMAND").map(|v| v != "0").unwrap_or(true)
}

/// The throwaway LibreOffice profile run_command's description suggests for headless conversions (one per gate
/// process, under the temp dir, removed when the gate ends; `gui-gate cleanup` sweeps leftovers).
pub fn lo_profile_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("gui-gate-lo-{}", std::process::id()))
}

pub fn tool_schema() -> Value {
    let lo = lo_profile_dir().to_string_lossy().to_string();
    json!({"name": "run_command",
        "description": format!("Run a bash command (or a short script) on this computer and get its exit code and output. Use it for terminal, file and document work done in code instead of by clicking: inspect files (ls, cat, head), edit spreadsheets / documents with python3 (openpyxl, odfpy, python-docx, python-pptx when installed), convert with `soffice --headless -env:UserInstallation=file://{lo} --convert-to FMT --outdir DIR FILE` (the -env part lets it run while LibreOffice is open). Every command is checked by the gate like a typed terminal command (overwrites, moves, deletes, permission changes and other risky effects need the user's words, the gate's reviewer or a person). If you change a file that is open in an app, reload it there (LibreOffice: File > Reload) or the app's own save will write its old copy over your change. No interactive programs (no editors, pagers or password prompts): stdin is empty."),
        "inputSchema": {"type": "object", "properties": {
            "command": {"type": "string", "description": "bash command line or script (heredocs allowed)"},
            "cwd": {"type": "string", "description": "working directory (default: the user's home)"},
            "timeout_s": {"type": "number", "description": "seconds before the command is killed (default 30, max 300)"}},
            "required": ["command"]}})
}

/// 0.9.7 GUI_GATE_JOBS: run_command gets mode start | poll | stop (detached long jobs) next to run.
pub fn add_job_modes(schema: &mut Value) {
    let props = &mut schema["inputSchema"]["properties"];
    props["mode"] = json!({"type": "string", "enum": ["run", "start", "poll", "stop"], "description": "run (default): wait for the result. start: launch a long command (installs, downloads, slow scripts) in the background and get a job number at once. poll / stop {job}: its status and output so far / kill it."});
    props["job"] = json!({"type": "integer", "description": "mode=poll / stop: the job number from mode=start"});
    schema["inputSchema"]["required"] = json!([]);
    let d = schema["description"].as_str().unwrap_or("").to_string();
    schema["description"] = json!(format!("{d} Commands longer than a minute or two: mode=start, then mode=poll."));
}

pub struct Ran {
    pub code: Option<i32>,
    pub timed_out: bool,
    pub ms: f64,
    pub stdout: String,
    pub stderr: String,
    pub out_bytes: usize,
    pub changed: Vec<(String, u64)>,
}

/// Last `max` characters of `s`, cut at a line start, with a marker of what was dropped.
pub fn tail_cap(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    let t: String = s.chars().skip(n - max).collect();
    let t = match t.find('\n') { Some(i) if i < t.len() / 2 => t[i + 1..].to_string(), _ => t };
    format!("[... {} earlier characters cut ...]\n{t}", n - t.chars().count())
}

fn changed_files(root: &str, since: SystemTime, out: &mut Vec<(String, u64)>, depth: usize, budget: &mut usize) {
    if depth > 3 || *budget == 0 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(root) else { return };
    for e in rd.flatten() {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" || name == "__pycache__" {
            continue;
        }
        let path = e.path().to_string_lossy().to_string();
        let Ok(m) = e.metadata() else { continue };
        if m.is_dir() {
            changed_files(&path, since, out, depth + 1, budget);
        } else if m.modified().map(|t| t >= since).unwrap_or(false) {
            out.push((path, m.len()));
        }
    }
}

pub fn run(cmd: &str, cwd: &str, timeout_s: f64) -> Ran {
    let t0 = Instant::now();
    // file timestamps use the kernel's coarse clock (a tick behind the precise one): leave a margin
    let since = SystemTime::now() - Duration::from_millis(200);
    let no_shell = |why: String| Ran { code: None, timed_out: false, ms: 0.0, stdout: String::new(), stderr: why, out_bytes: 0, changed: vec![] };
    let Some(mut c) = crate::proc::shell(cmd) else { return no_shell("run_command is not available on this OS (bash commands only)".into()) };
    c.current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .env("PAGER", "cat").env("GIT_PAGER", "cat").env("TERM", "dumb").env("DEBIAN_FRONTEND", "noninteractive")
        .env("PYTHONUNBUFFERED", "1");
    let mut child = match c.spawn() {
        Ok(ch) => ch,
        Err(e) => return no_shell(format!("could not start bash: {e}")),
    };
    // registered while it runs: a gate that ends now (client gone, signal) takes the command's group down with it
    crate::proc::register(child.id());
    let pid = child.id();
    type Buf = std::sync::Arc<std::sync::Mutex<(Vec<u8>, usize, bool)>>;
    let rd = |mut r: Box<dyn Read + Send>| -> Buf {
        let buf: Buf = Default::default();
        let b2 = buf.clone();
        std::thread::spawn(move || {
            let mut chunk = [0u8; 8192];
            loop {
                match r.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let mut g = b2.lock().unwrap();
                        g.1 += n;
                        g.0.extend_from_slice(&chunk[..n]);
                        if g.0.len() > 256 * 1024 {
                            let cut = g.0.len() - 128 * 1024;
                            g.0.drain(..cut);
                        }
                    }
                }
            }
            b2.lock().unwrap().2 = true;
        });
        buf
    };
    let ho = rd(Box::new(child.stdout.take().unwrap()));
    let he = rd(Box::new(child.stderr.take().unwrap()));
    let deadline = t0 + Duration::from_secs_f64(timeout_s.clamp(1.0, 300.0));
    let mut timed_out = false;
    let code = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st.code(),
            Ok(None) if Instant::now() >= deadline => {
                timed_out = true;
                crate::proc::kill_tree(&mut child);
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => break None,
        }
    };
    // a background child holding the pipes open must not hang the gate: take what arrived within 1.5 s
    let take = |b: &Buf| {
        let t = Instant::now();
        while !b.lock().unwrap().2 && t.elapsed() < Duration::from_millis(1500) {
            std::thread::sleep(Duration::from_millis(10));
        }
        let g = b.lock().unwrap();
        (String::from_utf8_lossy(&g.0).to_string(), g.1)
    };
    let (so, n1) = take(&ho);
    let (se, n2) = take(&he);
    // finished: a program it left running on purpose (`app &`) is the agent's to keep, not killed at shutdown
    crate::proc::unregister(pid);
    let mut changed = vec![];
    let mut budget = 4000;
    changed_files(cwd, since, &mut changed, 0, &mut budget);
    changed.sort();
    Ran { code, timed_out, ms: t0.elapsed().as_secs_f64() * 1000.0, stdout: so, stderr: se, out_bytes: n1 + n2, changed }
}

/// The agent-facing result text of a finished command.
pub fn result_text(r: &Ran, cwd: &str, out_cap: usize) -> (String, bool) {
    let mut t = String::new();
    let status = if r.timed_out { "killed after the timeout".to_string() } else { match r.code { Some(c) => format!("exit {c}"), None => "ended by a signal".into() } };
    t.push_str(&format!("[run_command] {status} in {:.1} s (cwd {cwd})", r.ms / 1000.0));
    let so = r.stdout.trim_end();
    let se = r.stderr.trim_end();
    if so.is_empty() && se.is_empty() {
        t.push_str("\n(no output)");
    }
    if !so.is_empty() {
        t.push_str(&format!("\n--- stdout ---\n{}", tail_cap(so, out_cap)));
    }
    if !se.is_empty() {
        t.push_str(&format!("\n--- stderr ---\n{}", tail_cap(se, out_cap / 2)));
    }
    if !r.changed.is_empty() {
        let names: Vec<String> = r.changed.iter().take(12).map(|(p, n)| format!("{} ({n} B)", sidefx::relpath(p, cwd))).collect();
        let more = if r.changed.len() > 12 { format!(" and {} more", r.changed.len() - 12) } else { String::new() };
        t.push_str(&format!("\n[gate] Files under the cwd changed by this command: {}{more}", names.join(", ")));
    }
    let err = r.timed_out || r.code != Some(0);
    (t, err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fx() -> String {
        let d = format!("/tmp/gg-cmd-test-{}-{}", std::process::id(), rand_hex(3));
        fs::create_dir_all(format!("{d}/dir1")).unwrap();
        fs::create_dir_all(format!("{d}/dir2")).unwrap();
        fs::create_dir_all(format!("{d}/dir3")).unwrap();
        fs::write(format!("{d}/file1"), "new\n").unwrap();
        fs::write(format!("{d}/dir2/file1"), "keep\n").unwrap();
        fs::write(format!("{d}/report.pdf"), "%PDF old").unwrap();
        fs::write(format!("{d}/report.docx"), "docx").unwrap();
        fs::write(format!("{d}/data.xlsx"), "xlsx").unwrap();
        d
    }
    fn lvl(e: Option<Effect>) -> String {
        e.map(|x| x.0).unwrap_or_else(|| "-".into())
    }

    #[test]
    fn same_rules_as_a_typed_terminal_command() {
        let d = fx();
        assert_eq!(lvl(command_risk("cp file1 dir1 && cp file1 dir2 && cp file1 dir3", &d)), "high");
        assert_eq!(lvl(command_risk("for x in dir1 dir3; do cp file1 $x; done", &d)), "-");
        assert_eq!(lvl(command_risk("cp -n file1 dir2", &d)), "-");
        assert_eq!(lvl(command_risk("sh -c 'cp file1 dir2'", &d)), "high");
        assert_eq!(lvl(command_risk("mv dir2 dir2.bak", &d)), "high");
        assert_eq!(lvl(command_risk("rm -rf dir2", &d)), "high");
        assert_eq!(lvl(command_risk("sudo ls", &d)), "high");
        assert_eq!(lvl(command_risk("ls -la && cat file1 | wc -l", &d)), "-");
        // multi-line: a later line still counts, and a `cd` on an earlier line moves the cwd
        assert_eq!(lvl(command_risk("echo start\ncd dir2\necho x > file1", &d)), "high");
    }

    #[test]
    fn heredoc_bodies_and_interpreter_code() {
        let d = fx();
        // the body is not parsed as shell (an apostrophe in it must not hide the rest)
        assert_eq!(lvl(command_risk("cat > notes.txt <<'EOF'\nit's fine\nEOF\nls", &d)), "-");
        assert_eq!(lvl(command_risk("cat > notes.txt <<'EOF'\nit's fine\nEOF\ncp file1 dir2", &d)), "high");
        assert_eq!(lvl(command_risk("python3 - <<'EOF'\nimport shutil\nshutil.rmtree('dir2')\nEOF", &d)), "medium");
        assert_eq!(lvl(command_risk("python3 -c \"import os; os.system('rm -rf dir2')\"", &d)), "high");
        assert_eq!(lvl(command_risk("python3 -c \"import subprocess; subprocess.run('cp file1 dir2', shell=True)\"", &d)), "high");
        assert_eq!(lvl(command_risk("bash <<EOF\ncp file1 dir2\nEOF", &d)), "high");
        // writing to a new file is fine; over an existing pre-session file is an overwrite for the reviewer
        assert_eq!(lvl(command_risk("python3 - <<'EOF'\nopen('out_new.txt','w').write('x')\nEOF", &d)), "-");
        let r = command_risk("python3 - <<'EOF'\nimport openpyxl\nwb=openpyxl.load_workbook('data.xlsx')\nwb.save('data.xlsx')\nEOF", &d).unwrap();
        assert_eq!(r.0, "medium");
        assert!(r.1.contains("overwrite existing file(s): data.xlsx"), "{}", r.1);
        assert_eq!(crate::authz::hazard_of(&r.1), "overwrite");
        // a script file written earlier is read and judged when it is run
        fs::write(format!("{d}/s.py"), "import os\nos.remove('file1')\n").unwrap();
        assert_eq!(lvl(command_risk("python3 s.py", &d)), "medium");
        fs::write(format!("{d}/s.sh"), "cp file1 dir2\n").unwrap();
        assert_eq!(lvl(command_risk("bash s.sh", &d)), "high");
        assert_eq!(lvl(command_risk("echo 'unbalanced", &d)), "medium");
    }

    /// Offline audit, the run_command route: GG_AUDIT=<jsonl {id, cmd, cwd, task}> GG_AUDIT_CMD_OUT=<tsv>
    /// cargo test -- --ignored command_audit. Same rows as sidefx_audit, judged as run_command would.
    #[test]
    #[ignore]
    fn command_audit() {
        let (Ok(inp), Ok(outp)) = (std::env::var("GG_AUDIT"), std::env::var("GG_AUDIT_CMD_OUT")) else { return };
        crate::sidefx::mark_session_start();
        let mut out = String::new();
        for line in fs::read_to_string(inp).unwrap().lines() {
            let r: serde_json::Value = serde_json::from_str(line).unwrap();
            let (cmd, cwd, task) = (r["cmd"].as_str().unwrap(), r["cwd"].as_str().unwrap(), r["task"].as_str().unwrap_or(""));
            let eff = command_risk(cmd, cwd);
            let (lvl, reason) = eff.clone().unwrap_or(("-".into(), "-".into()));
            let (hz, ok) = match &eff {
                Some((_, rs)) => (crate::authz::hazard_of(rs).to_string(), crate::authz::authorized(rs, task, "", "", &[]).0.to_string()),
                None => ("-".into(), "-".into()),
            };
            out.push_str(&format!("{}\t{lvl}\t{hz}\t{ok}\t{}\n", r["id"].as_str().unwrap(), reason.replace('\t', " ")));
        }
        fs::write(outp, out).unwrap();
    }

    #[test]
    fn soffice_convert_over_an_existing_file() {
        let d = fx();
        let r = command_risk("soffice --headless --convert-to pdf report.docx", &d).unwrap();
        assert_eq!(r.0, "high");
        assert!(r.1.contains("report.pdf"));
        assert_eq!(crate::authz::hazard_of(&r.1), "overwrite");
        assert_eq!(lvl(command_risk("soffice --headless --convert-to pdf --outdir /tmp/gg-new-out report.docx", &d)), "-");
        assert_eq!(lvl(command_risk("soffice --headless --convert-to csv:\"Text - txt - csv (StarCalc)\":44,34,76 data.xlsx", &d)), "-");
        // the user's words authorize it like any overwrite
        let (ok, _) = crate::authz::authorized(&r.1, "Export report.docx to PDF and overwrite report.pdf", "", "", &[]);
        assert!(ok);
        let (ok, _) = crate::authz::authorized(&r.1, "Export the current document into PDF, keep the file name", "", "", &[]);
        assert!(!ok);
    }

    #[test]
    fn runs_with_timeout_and_tail() {
        let d = fx();
        let r = run("echo hello; echo oops >&2; exit 3", &d, 10.0);
        assert_eq!(r.code, Some(3));
        let (t, err) = result_text(&r, &d, 4000);
        assert!(err && t.contains("exit 3") && t.contains("hello") && t.contains("oops"), "{t}");
        let r = run("sleep 5; echo never", &d, 1.0);
        assert!(r.timed_out && r.ms < 4000.0, "{}", r.ms);
        let r = run("seq 1 100000", &d, 10.0);
        let (t, _) = result_text(&r, &d, 500);
        assert!(t.contains("100000") && t.contains("earlier characters cut") && t.len() < 900, "{}", t.len());
        let r = run("echo x > made.txt", &d, 10.0);
        let (t, _) = result_text(&r, &d, 500);
        assert!(t.contains("made.txt"), "{t}");
        // a background child keeping the pipe open does not hang the call
        let r = run("(sleep 20 &) ; echo bg", &d, 10.0);
        assert!(r.ms < 5000.0 && r.stdout.contains("bg"), "{} {}", r.ms, r.stdout);
    }
}
