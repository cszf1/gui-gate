//! Side-effect checks the gate can do because it runs on the controlled machine.
//!
//! A model sees pixels and a tree; it does not see the file system. Before a shell command typed into a
//! terminal is submitted, the gate parses it and looks at the disk: would `cp`/`mv`/`>` replace a file that
//! already exists, would a recursive chmod strip the execute bit from directories, would `rm` delete files.
//! It never runs the command; `find` is the only thing it runs, read-only (its actions are stripped).
//!
//! Also: overwrite-confirmation dialogs turn a plain "Yes"/"Replace" click (or Enter) into a high-risk action,
//! and a file dialog's Save/Export is checked against the folder it would write into.
use regex::Regex;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// (level, reason, targets): targets are the canonical absolute paths the effect would touch (may be empty when the
/// gate cannot tell, e.g. `find -exec rm` without a resolvable root). authz builds structured effect keys from them.
pub type Effect = (String, String, Vec<String>);

/// Canonical absolute form of a path for effect keys (symlinks resolved when the path exists).
pub fn canon(p: &str) -> String {
    realpath(p).to_string_lossy().to_string()
}

fn canon_all(v: &[String]) -> Vec<String> {
    let mut out: Vec<String> = v.iter().map(|x| canon(x)).collect();
    out.sort();
    out.dedup();
    out
}

macro_rules! re {
    ($name:ident, $pat:expr) => {
        pub fn $name() -> &'static Regex {
            static R: OnceLock<Regex> = OnceLock::new();
            R.get_or_init(|| Regex::new($pat).unwrap())
        }
    };
}

re!(overwrite_ctx, r"(?i)(already exists|do you want to (replace|overwrite)|replace the existing|overwrite the existing|file exists|已存在|是否(替换|覆盖)|替换现有)");
re!(in_document_ctx, r"(?i)(pasting data into cells|cells that already contain|overwrite the existing data|replace the (current )?selection|单元格.*已有数据|覆盖现有数据)");
re!(file_words, r#"(?i)(\bfile\b|\bfolder\b|\.[a-z0-9]{2,5}\b["'”]?\s*already exists|文件)"#);
re!(terminal_comm, r"(?i)(term|konsole|tilix|alacritty|kitty|wezterm|guake|terminator|foot|sakura)");
// (backslashes allowed inside: Windows folder paths; an escaped closing quote \" still ends the value)
re!(value_rx, r#"value=\\?"([^"]{1,400}?)\\?""#);
re!(ext_rx, r"\(\*?\.([A-Za-z0-9]{1,6})\)");
re!(chmod_mode, r"^0?[0-7]{3,4}$");
re!(digits_dots, r"^[0-9.]+$");

pub const CONFIRM_LABELS: &[&str] = &["yes", "ok", "replace", "overwrite", "save", "continue", "use", "keep both", "&yes",
    "_yes", "是", "确定", "替换", "覆盖", "保存", "继续"];
pub const SAVE_LABELS: &[&str] = &["save", "export", "ok", "save as", "保存", "导出", "确定"];
const SHELLS: &[&str] = &["bash", "sh", "zsh", "fish", "dash", "ksh"];

pub fn is_terminal(pid: i64) -> bool {
    let comm = fs::read_to_string(format!("/proc/{pid}/comm"));
    let cmd = fs::read_to_string(format!("/proc/{pid}/cmdline"));
    let (Ok(comm), Ok(cmd)) = (comm, cmd) else { return cfg!(target_os = "macos") && ps_is_terminal(pid) };
    let first = cmd.split('\0').next().unwrap_or("").to_string();
    terminal_comm().is_match(comm.trim()) || (!first.is_empty() && terminal_comm().is_match(&first))
}

/// macOS has no /proc: the process table comes from `ps` (Terminal.app is ".../Terminal.app/Contents/MacOS/Terminal",
/// iTerm2 "iTerm2"; only the executable's own name is matched). Cached for a few seconds per pid.
fn ps_is_terminal(pid: i64) -> bool {
    static CACHE: OnceLock<std::sync::Mutex<HashMap<i64, (Instant, bool)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some((t, v)) = cache.lock().unwrap().get(&pid) {
        if t.elapsed() < Duration::from_secs(5) {
            return *v;
        }
    }
    let out = Command::new("ps").args(["-o", "comm=", "-p", &pid.to_string()]).stderr(Stdio::null()).output();
    let v = out.ok().filter(|o| o.status.success())
        .map(|o| exe_name(String::from_utf8_lossy(&o.stdout).trim()))
        .map(|n| !n.is_empty() && terminal_comm().is_match(&n)).unwrap_or(false);
    cache.lock().unwrap().insert(pid, (Instant::now(), v));
    v
}

/// The executable's own name from a `ps` comm column (a full path on macOS; "-zsh" for a login shell).
fn exe_name(comm: &str) -> String {
    comm.rsplit('/').next().unwrap_or("").trim_start_matches('-').to_string()
}

/// `ps -A -o pid=,ppid=,comm=` -> (children by parent, executable name by pid).
fn parse_ps_table(text: &str) -> (HashMap<i64, Vec<i64>>, HashMap<i64, String>) {
    let mut kids: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut comm = HashMap::new();
    for l in text.lines() {
        let mut it = l.split_whitespace();
        let (Some(p), Some(pp)) = (it.next().and_then(|x| x.parse::<i64>().ok()), it.next().and_then(|x| x.parse::<i64>().ok())) else { continue };
        let rest: Vec<&str> = it.collect();
        kids.entry(pp).or_default().push(p);
        comm.insert(p, exe_name(&rest.join(" ")));
    }
    (kids, comm)
}

/// `lsof -a -p PID -d cwd -Fn` -> the working directory (the line starting with "n").
fn parse_lsof_cwd(text: &str) -> Option<String> {
    text.lines().find_map(|l| l.strip_prefix('n')).filter(|p| p.starts_with('/')).map(String::from)
}

/// Working directory of the newest shell running under a terminal emulator.
pub fn shell_cwd(term_pid: i64) -> String {
    let home = crate::util::home();
    let mut kids: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut comm: HashMap<i64, String> = HashMap::new();
    let proc_fs = Path::new("/proc/self").exists();
    if let Ok(rd) = fs::read_dir("/proc") {
        for d in rd.flatten() {
            let name = d.file_name().to_string_lossy().to_string();
            let Ok(pid) = name.parse::<i64>() else { continue };
            let Ok(st) = fs::read_to_string(format!("/proc/{pid}/stat")) else { continue };
            let (Some(lp), Some(rp)) = (st.find('('), st.rfind(')')) else { continue };
            let rest: Vec<&str> = st[rp + 2..].split_whitespace().collect();
            let Some(ppid) = rest.get(1).and_then(|x| x.parse::<i64>().ok()) else { continue };
            kids.entry(ppid).or_default().push(pid);
            comm.insert(pid, st[lp + 1..rp].to_string());
        }
    }
    if !proc_fs && cfg!(target_os = "macos") {
        if let Ok(o) = Command::new("ps").args(["-A", "-o", "pid=,ppid=,comm="]).stderr(Stdio::null()).output() {
            (kids, comm) = parse_ps_table(&String::from_utf8_lossy(&o.stdout));
        }
    }
    let mut best: Option<i64> = None;
    let mut stack = vec![term_pid];
    let mut seen = HashSet::new();
    while let Some(p) = stack.pop() {
        if !seen.insert(p) {
            continue;
        }
        for &k in kids.get(&p).map(|v| v.as_slice()).unwrap_or(&[]) {
            if comm.get(&k).map(|c| SHELLS.contains(&c.as_str())).unwrap_or(false) {
                best = Some(best.map_or(k, |b| b.max(k)));
            }
            stack.push(k);
        }
    }
    match best {
        Some(b) if proc_fs => fs::read_link(format!("/proc/{b}/cwd")).map(|p| p.to_string_lossy().to_string()).unwrap_or(home),
        Some(b) => Command::new("lsof").args(["-a", "-p", &b.to_string(), "-d", "cwd", "-Fn"]).stderr(Stdio::null()).output().ok()
            .and_then(|o| parse_lsof_cwd(&String::from_utf8_lossy(&o.stdout))).unwrap_or(home),
        None => home,
    }
}

// ----------------------------------------------------------------------------- path helpers (Python os.path)
pub fn expandvars(s: &str) -> String {
    let re = {
        static R: OnceLock<Regex> = OnceLock::new();
        R.get_or_init(|| Regex::new(r"\$(\w+|\{[^}]*\})").unwrap())
    };
    re.replace_all(s, |c: &regex::Captures| {
        let name = c[1].trim_start_matches('{').trim_end_matches('}').to_string();
        std::env::var(&name).unwrap_or_else(|_| c[0].to_string())
    }).to_string()
}

pub fn expanduser(s: &str) -> String {
    if s == "~" {
        return crate::util::home();
    }
    if let Some(r) = s.strip_prefix("~/") {
        return format!("{}/{}", crate::util::home(), r);
    }
    s.to_string()
}

pub fn normpath(p: &str) -> String {
    let abs = p.starts_with('/');
    let mut out: Vec<&str> = vec![];
    for part in p.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if out.last().map(|x| *x != "..").unwrap_or(false) {
                    out.pop();
                } else if !abs {
                    out.push("..");
                }
            }
            x => out.push(x),
        }
    }
    let j = out.join("/");
    if abs { format!("/{j}") } else if j.is_empty() { ".".into() } else { j }
}

pub fn join(a: &str, b: &str) -> String {
    if b.starts_with('/') {
        b.to_string()
    } else if a.ends_with('/') || a.is_empty() {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

pub fn basename(p: &str) -> String {
    p.rsplit('/').next().unwrap_or("").to_string()
}

pub fn relpath(p: &str, start: &str) -> String {
    let a = normpath(&if p.starts_with('/') { p.to_string() } else { join(start, p) });
    let b = normpath(start);
    let pa: Vec<&str> = a.split('/').filter(|x| !x.is_empty()).collect();
    let pb: Vec<&str> = b.split('/').filter(|x| !x.is_empty()).collect();
    let mut i = 0;
    while i < pa.len() && i < pb.len() && pa[i] == pb[i] {
        i += 1;
    }
    let mut parts: Vec<String> = std::iter::repeat("..".to_string()).take(pb.len() - i).collect();
    parts.extend(pa[i..].iter().map(|x| x.to_string()));
    if parts.is_empty() { ".".into() } else { parts.join("/") }
}

pub fn splitext_ext(name: &str) -> String {
    let b = basename(name);
    let t = b.trim_start_matches('.');
    match t.rfind('.') {
        Some(i) => t[i..].to_string(),
        None => String::new(),
    }
}

pub fn p(path: &str, cwd: &str) -> String {
    let e = expandvars(&expanduser(path));
    normpath(&if e.starts_with('/') { e } else { join(cwd, &e) })
}

pub fn is_dir(p: &str) -> bool {
    Path::new(p).is_dir()
}

pub fn nonempty_file(p: &str) -> bool {
    fs::metadata(p).map(|m| m.is_file() && m.len() > 0).unwrap_or(false)
}

pub fn exists(p: &str) -> bool {
    fs::symlink_metadata(p).is_ok() && Path::new(p).exists()
}

pub fn realpath(p: &str) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(normpath(p)))
}

fn expand(args: &[String], cwd: &str) -> Vec<String> {
    let mut out = vec![];
    for a in args {
        if a.contains(['*', '?', '[']) {
            let pat = p(a, cwd);
            let opts = glob::MatchOptions { case_sensitive: true, require_literal_separator: true, require_literal_leading_dot: true };
            let mut hits: Vec<String> = glob::glob_with(&pat, opts).map(|g| g.flatten().map(|x| x.to_string_lossy().to_string()).collect())
                .unwrap_or_default();
            hits.sort();
            if hits.is_empty() {
                out.push(pat);
            } else {
                out.extend(hits);
            }
        } else {
            out.push(p(a, cwd));
        }
    }
    out
}

fn walk_files(root: &str, out: &mut Vec<String>) {
    if let Ok(rd) = fs::read_dir(root) {
        for e in rd.flatten() {
            let path = e.path().to_string_lossy().to_string();
            match e.file_type() {
                Ok(ft) if ft.is_dir() => walk_files(&path, out),
                Ok(_) => out.push(path),
                Err(_) => {}
            }
        }
    }
}

/// Existing files a cp/mv of srcs -> dest would replace.
fn copy_clobbers(srcs: &[String], dest: &str, recursive: bool, parents: bool, cwd: &str) -> Vec<String> {
    let mut hit = vec![];
    let dest_is_dir = is_dir(dest);
    for s in srcs {
        let tgt = if parents {
            join(dest, &relpath(s, cwd))
        } else if dest_is_dir {
            join(dest, &basename(s.trim_end_matches('/')))
        } else {
            dest.to_string()
        };
        if is_dir(s) && recursive {
            if is_dir(&tgt) {
                let mut files = vec![];
                walk_files(s, &mut files);
                for f in files {
                    let t2 = join(&tgt, &relpath(&f, s));
                    if nonempty_file(&t2) {
                        hit.push(t2);
                    }
                }
            }
        } else if nonempty_file(&tgt) && realpath(&tgt) != realpath(s) {
            hit.push(tgt);
        }
    }
    hit
}

fn cp_like(argv: &[String], cwd: &str) -> Option<Effect> {
    let tool = basename(&argv[0]);
    let mut flags: BTreeSet<String> = BTreeSet::new();
    let mut pos: Vec<String> = vec![];
    let mut target_dir: Option<String> = None;
    let mut i = 1;
    while i < argv.len() {
        let a = &argv[i];
        if a == "-t" || a == "--target-directory" {
            target_dir = argv.get(i + 1).cloned();
            i += 2;
            continue;
        }
        if let Some(v) = a.strip_prefix("--target-directory=") {
            target_dir = Some(v.to_string());
        } else if a.starts_with("--") {
            flags.insert(a.clone());
        } else if a.starts_with('-') && a.len() > 1 {
            for c in a[1..].chars() {
                flags.insert(format!("-{c}"));
            }
        } else {
            pos.push(a.clone());
        }
        i += 1;
    }
    let has = |f: &[&str]| f.iter().any(|x| flags.contains(*x));
    if has(&["-n", "--no-clobber", "-i", "--interactive", "--update=none", "-u", "--update"]) {
        return None;
    }
    if tool == "rsync" && has(&["--ignore-existing"]) {
        return None;
    }
    let (srcs, dest) = if let Some(t) = target_dir {
        (pos.clone(), t)
    } else if pos.len() >= 2 {
        (pos[..pos.len() - 1].to_vec(), pos[pos.len() - 1].clone())
    } else {
        return None;
    };
    let recursive = has(&["-r", "-R", "-a", "--recursive", "--archive"]) || tool == "mv" || tool == "rsync";
    let parents = has(&["--parents"]) || (has(&["-R"]) && tool == "rsync");
    let clob = copy_clobbers(&expand(&srcs, cwd), &p(&dest, cwd), recursive, parents, cwd);
    if clob.is_empty() {
        return None;
    }
    let names: Vec<String> = clob.iter().take(3).map(|c| relpath(c, cwd)).collect();
    let more = if clob.len() > 3 { " ..." } else { "" };
    Some(("high".into(), format!("`{tool}` would overwrite existing file(s): {}{more}", names.join(", ")), canon_all(&clob)))
}

/// `soffice --headless --convert-to FMT [--outdir D] files...` writes D/<stem>.<fmt> without asking: an existing file
/// of that name (og02: the PDF that is already there) is replaced silently.
fn soffice_convert(argv: &[String], cwd: &str) -> Option<Effect> {
    let mut fmt: Option<String> = None;
    let mut outdir = cwd.to_string();
    let mut inputs: Vec<String> = vec![];
    let mut i = 1;
    while i < argv.len() {
        let a = &argv[i];
        if a == "--convert-to" || a == "-convert-to" {
            fmt = argv.get(i + 1).cloned();
            i += 2;
            continue;
        }
        if a == "--outdir" || a == "-outdir" {
            if let Some(d) = argv.get(i + 1) {
                outdir = p(d, cwd);
            }
            i += 2;
            continue;
        }
        if let Some(v) = a.strip_prefix("--convert-to=") {
            fmt = Some(v.to_string());
        } else if let Some(v) = a.strip_prefix("--outdir=") {
            outdir = p(v, cwd);
        } else if !a.starts_with('-') {
            inputs.push(a.clone());
        }
        i += 1;
    }
    let fmt = fmt?;
    let ext = fmt.split(':').next().unwrap_or("").trim().to_lowercase();
    if ext.is_empty() {
        return None;
    }
    let mut hit = vec![];
    let mut abs = vec![];
    for src in expand(&inputs, cwd) {
        let b = basename(&src);
        let e = splitext_ext(&b);
        let stem = &b[..b.len() - e.len()];
        let tgt = join(&outdir, &format!("{stem}.{ext}"));
        if nonempty_file(&tgt) && realpath(&tgt) != realpath(&src) {
            hit.push(relpath(&tgt, cwd));
            abs.push(tgt);
        }
    }
    if hit.is_empty() {
        return None;
    }
    Some(("high".into(), format!("`soffice --convert-to` would overwrite existing file(s): {}", hit.join(", ")), canon_all(&abs)))
}

static SESSION_START: OnceLock<std::time::SystemTime> = OnceLock::new();

pub fn session_start() -> Option<std::time::SystemTime> {
    SESSION_START.get().copied()
}

/// Remember when this gate started: paths created or modified after it are the agent's own work, not user data.
pub fn mark_session_start() {
    let _ = SESSION_START.set(std::time::SystemTime::now());
}

/// True when the path was created/changed during this gate session (the agent's own work, so moving it is harmless).
pub fn session_owned(path: &str, start: Option<std::time::SystemTime>) -> bool {
    let Some(start) = start else { return false };
    // created in this session (birth time). A user's older file that was merely modified is not ours; where the file
    // system has no birth time nothing is treated as ours.
    fs::symlink_metadata(path).and_then(|m| m.created()).map(|t| t >= start).unwrap_or(false)
}

/// `mv` (or `rsync --remove-source-files`) of an existing path that predates this session: the user's data
/// disappears from where it was (og10: `mv instructor-embedding instructor-embedding.bak && git clone ...`).
fn move_away(argv: &[String], cwd: &str) -> Option<Effect> {
    let tool = basename(&argv[0]);
    if tool == "rsync" && !argv.iter().any(|a| a == "--remove-source-files") {
        return None;
    }
    let mut pos: Vec<String> = vec![];
    let mut target_dir: Option<String> = None;
    let mut i = 1;
    while i < argv.len() {
        let a = &argv[i];
        if a == "-t" || a == "--target-directory" {
            target_dir = argv.get(i + 1).cloned();
            i += 2;
            continue;
        }
        if let Some(v) = a.strip_prefix("--target-directory=") {
            target_dir = Some(v.to_string());
        } else if !(a.starts_with('-') && a.len() > 1) {
            pos.push(a.clone());
        }
        i += 1;
    }
    let (srcs, dest) = match target_dir {
        Some(t) => (pos.clone(), t),
        None if pos.len() >= 2 => (pos[..pos.len() - 1].to_vec(), pos[pos.len() - 1].clone()),
        None => return None,
    };
    let moved: Vec<String> = expand(&srcs, cwd).into_iter().filter(|x| exists(x) && !session_owned(x, SESSION_START.get().copied())).collect();
    if moved.is_empty() {
        return None;
    }
    let names: Vec<String> = moved.iter().take(3).map(|c| relpath(c, cwd)).collect();
    let more = if moved.len() > 3 { " ..." } else { "" };
    Some(("high".into(), format!("`{tool}` would move the existing {}{more} away (to {}); it is no longer where it was",
        names.join(", "), relpath(&p(&dest, cwd), cwd)), canon_all(&moved)))
}

fn run_find(args: &[String], cwd: &str) -> Option<Vec<u8>> {
    let mut cmd = Command::new(&args[0]);
    crate::proc::scrub_env(&mut cmd);
    let mut child = cmd.args(&args[1..]).current_dir(cwd).stdin(Stdio::null())
        .stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let mut out = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = vec![];
        let _ = std::io::Read::read_to_end(&mut out, &mut buf);
        buf
    });
    let t0 = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if t0.elapsed() > Duration::from_secs(3) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => return None,
        }
    }
    reader.join().ok()
}

fn find_roots(argv: &[String], cwd: &str) -> Vec<String> {
    let mut roots: Vec<String> = argv[1..].iter().take_while(|a| !a.starts_with('-') && !["(", "!"].contains(&a.as_str()))
        .map(|a| p(a, cwd)).collect();
    if roots.is_empty() {
        roots.push(cwd.to_string());
    }
    canon_all(&roots)
}

fn find_exec(argv: &[String], cwd: &str) -> Option<Effect> {
    let Some(k) = argv.iter().position(|a| ["-exec", "-execdir", "-ok", "-okdir"].contains(&a.as_str())) else {
        return if argv.iter().any(|a| a == "-delete") { Some(("high".into(), "`find -delete` deletes files".into(), find_roots(argv, cwd))) } else { None };
    };
    let inner_all = &argv[k + 1..];
    let end = inner_all.iter().position(|a| a == ";" || a == "\\;" || a == "+").unwrap_or(inner_all.len());
    let inner = &inner_all[..end];
    let head = inner.first().map(|x| basename(x)).unwrap_or_default();
    if inner.is_empty() || !["cp", "mv", "rsync", "install"].contains(&head.as_str()) {
        if head == "rm" {
            return Some(("high".into(), "`find -exec rm` deletes files".into(), find_roots(&argv[..k], cwd)));
        }
        if head == "chmod" {
            return chmod(inner, true, cwd);
        }
        return None;
    }
    if argv[..k].iter().any(|a| ["-fprint", "-fprint0", "-fprintf", "-fls", "-delete"].contains(&a.as_str())) {
        return None; // not read-only: do not run it
    }
    let mut cmd: Vec<String> = argv[..k].to_vec();
    cmd.push("-print0".into());
    let out = run_find(&cmd, cwd)?;
    let files: Vec<String> = out.split(|b| *b == 0).filter(|f| !f.is_empty()).take(2000)
        .map(|f| String::from_utf8_lossy(f).to_string()).collect();
    let mut hits = vec![];
    let mut tg = vec![];
    for f in files {
        let av: Vec<String> = inner.iter().map(|a| if a == "{}" { f.clone() } else { a.clone() }).collect();
        if let Some(r) = cp_like(&av, cwd) {
            hits.push(r.1);
            tg.extend(r.2);
        }
    }
    if hits.is_empty() {
        return None;
    }
    let more = if hits.len() > 1 { format!(" (and {} more)", hits.len() - 1) } else { String::new() };
    Some(("high".into(), format!("{}{more}", hits[0]), canon_all(&tg)))
}

fn chmod(argv: &[String], via_find: bool, cwd: &str) -> Option<Effect> {
    let rec = argv[1..].iter().any(|a| a == "-R" || a == "--recursive" || (a.starts_with('-') && a.contains('R') && !a.starts_with("--")));
    let mode = argv[1..].iter().find(|a| !a.starts_with('-')).cloned().unwrap_or_default();
    if via_find || !rec {
        return None;
    }
    let paths: Vec<String> = argv[1..].iter().filter(|a| !a.starts_with('-')).skip(1).map(|a| p(a, cwd)).collect();
    let tg = canon_all(&paths);
    if chmod_mode().is_match(&mode) {
        let d = mode.chars().nth(mode.len() - 3).and_then(|c| c.to_digit(8)).unwrap_or(1);
        if d % 2 == 0 {
            return Some(("high".into(), format!("`chmod -R {mode}` also hits directories and removes their execute bit (they become unopenable)"), tg));
        }
    }
    Some(("medium".into(), format!("`chmod -R {mode}` changes directories too, not only files"), tg))
}

const PUNCT: &str = "();<>|&";

/// POSIX shell-like tokenizer (Python shlex, posix=True, punctuation_chars=True, whitespace_split=True, '#').
/// None on an unterminated quote.
pub fn tokenize(src: &str) -> Option<Vec<String>> {
    let chars: Vec<char> = src.chars().collect();
    let mut toks = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    let mut punct = false;
    let mut i = 0;
    macro_rules! emit {
        () => {
            if !cur.is_empty() || quoted {
                toks.push(std::mem::take(&mut cur));
            }
            quoted = false;
        };
    }
    #[allow(unused_assignments)]
    while i < chars.len() {
        let c = chars[i];
        if punct {
            if PUNCT.contains(c) {
                cur.push(c);
                i += 1;
                continue;
            }
            emit!();
            punct = false;
        }
        if c.is_whitespace() {
            emit!();
        } else if c == '#' {
            emit!();
            break;
        } else if PUNCT.contains(c) {
            emit!();
            punct = true;
            cur.push(c);
        } else if c == '\'' {
            quoted = true;
            i += 1;
            while i < chars.len() && chars[i] != '\'' {
                cur.push(chars[i]);
                i += 1;
            }
            if i >= chars.len() {
                return None;
            }
        } else if c == '"' {
            quoted = true;
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() && (chars[i + 1] == '"' || chars[i + 1] == '\\') {
                    i += 1;
                }
                cur.push(chars[i]);
                i += 1;
            }
            if i >= chars.len() {
                return None;
            }
        } else if c == '\\' {
            i += 1;
            if i >= chars.len() {
                return None;
            }
            cur.push(chars[i]);
        } else {
            cur.push(c);
        }
        i += 1;
    }
    emit!();
    Some(toks)
}

/// `find A... | xargs [opts] cmd args...` as the equivalent `find A... -exec cmd args... {} ;` (read-only `find` is
/// how the gate sees which files the pipeline would touch). None when it is not that shape.
fn xargs_as_find_exec(find_argv: &[String], xargs_argv: &[String]) -> Option<Vec<String>> {
    if find_argv.first().map(|a| basename(a)) != Some("find".into()) {
        return None;
    }
    let mut i = 1;
    let mut repl: Option<String> = None;
    while i < xargs_argv.len() {
        let a = &xargs_argv[i];
        if a == "-I" || a == "--replace" {
            repl = xargs_argv.get(i + 1).cloned();
            i += 2;
            continue;
        }
        if let Some(r) = a.strip_prefix("-I").filter(|r| !r.is_empty()) {
            repl = Some(r.to_string());
        } else if ["-n", "-L", "-P", "-d", "-s", "-E", "-a"].contains(&a.as_str()) {
            i += 2;
            continue;
        } else if !a.starts_with('-') {
            break;
        }
        i += 1;
    }
    let inner = &xargs_argv[i..];
    if inner.is_empty() {
        return None;
    }
    let mut cmd: Vec<String> = find_argv.iter().filter(|a| !["-print0", "-print"].contains(&a.as_str())).cloned().collect();
    cmd.push("-exec".into());
    match &repl {
        Some(r) => cmd.extend(inner.iter().map(|a| a.replace(r.as_str(), "{}"))),
        None => {
            cmd.extend(inner.iter().cloned());
            cmd.push("{}".into());
        }
    }
    cmd.push(";".into());
    Some(cmd)
}

/// `for V in a b c; do BODY; done` -> `BODY[V=a] ; BODY[V=b] ; BODY[V=c]` (simple word lists only, globs kept for
/// `expand`), so a loop that copies into an existing file is judged like the unrolled commands (a loop over
/// `$(...)` or `"$@"` stays as it was).
pub fn unroll_for_loops(src: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    let rx = R.get_or_init(|| Regex::new(r"\bfor\s+([A-Za-z_][A-Za-z0-9_]*)\s+in\s+([^;\n]*?)\s*[;\n]\s*do\s+(.*?)\s*;?\s*\bdone\b").unwrap());
    let mut out = src.to_string();
    for _ in 0..4 {
        let Some(c) = rx.captures(&out) else { break };
        let (var, list, body) = (c[1].to_string(), c[2].to_string(), c[3].to_string());
        if list.contains('$') || list.contains('`') {
            break;
        }
        let Some(words) = tokenize(&list) else { break };
        let parts: Vec<String> = words.iter().map(|w| {
            body.replace(&format!("\"${{{var}}}\""), w).replace(&format!("\"${var}\""), w)
                .replace(&format!("${{{var}}}"), w).replace(&format!("${var}"), w)
        }).collect();
        let m = c.get(0).unwrap();
        out = format!("{}{}{}", &out[..m.start()], parts.join(" ; "), &out[m.end()..]);
    }
    out
}

/// `echo|printf WORDS | xargs [-n N] [-I R] CMD ARGS` as the command lines xargs would run (one per word).
fn xargs_from_echo(prev: &[String], xargs_argv: &[String]) -> Option<String> {
    let p0 = prev.first().map(|a| basename(a))?;
    if p0 != "echo" && p0 != "printf" {
        return None;
    }
    let words: Vec<String> = prev[1..].iter().filter(|w| !w.starts_with('-') && !w.contains('%') && !w.contains('\\')).cloned().collect();
    let mut i = 1;
    let mut repl: Option<String> = None;
    while i < xargs_argv.len() {
        let a = &xargs_argv[i];
        if a == "-I" || a == "--replace" {
            repl = xargs_argv.get(i + 1).cloned();
            i += 2;
            continue;
        }
        if let Some(r) = a.strip_prefix("-I").filter(|r| !r.is_empty()) {
            repl = Some(r.to_string());
        } else if ["-n", "-L", "-P", "-d", "-s", "-E", "-a"].contains(&a.as_str()) {
            i += 2;
            continue;
        } else if !a.starts_with('-') {
            break;
        }
        i += 1;
    }
    let inner = &xargs_argv[i..];
    if inner.is_empty() || words.is_empty() {
        return None;
    }
    let q = |w: &str| format!("'{}'", w.replace('\'', ""));
    let lines: Vec<String> = words.iter().map(|w| match &repl {
        Some(r) => inner.iter().map(|a| q(&a.replace(r.as_str(), w))).collect::<Vec<_>>().join(" "),
        None => format!("{} {}", inner.iter().map(|a| q(a)).collect::<Vec<_>>().join(" "), q(w)),
    }).collect();
    Some(lines.join(" ; "))
}

/// [(argv, overwrite-redirect targets)] for each simple command.
fn commands(cmdline: &str) -> Vec<(Vec<String>, Vec<String>)> {
    let src = unroll_for_loops(&cmdline.replace('\n', " ; ")).replace("\\;", " __ESC_SEMI__ ");
    let Some(toks) = tokenize(&src) else { return vec![] };
    let mut out = vec![];
    let (mut argv, mut redir): (Vec<String>, Vec<String>) = (vec![], vec![]);
    let mut i = 0;
    while i < toks.len() {
        let t = toks[i].as_str();
        match t {
            ";" | "&&" | "||" | "|" | "&" | "(" | ")" => {
                if !argv.is_empty() {
                    out.push((std::mem::take(&mut argv), std::mem::take(&mut redir)));
                } else {
                    redir.clear();
                }
                if t == "|" {
                    out.push((vec!["__PIPE__".into()], vec![]));
                }
            }
            ">" | ">|" | "&>" => {
                if i + 1 < toks.len() {
                    redir.push(toks[i + 1].clone());
                }
                i += 1;
            }
            ">>" | "<" | "<<" | "2>" | ">&" => i += 1,
            _ => argv.push(if t == "__ESC_SEMI__" { ";".into() } else { t.to_string() }),
        }
        i += 1;
    }
    if !argv.is_empty() {
        out.push((argv, redir));
    }
    out
}

/// Keep the worse of two effects (high beats medium; the first one wins a tie) and the targets of both: a key built
/// from it covers everything risky the command line touches.
pub fn merge(a: Option<Effect>, b: Effect) -> Option<Effect> {
    let Some(mut a) = a else { return Some(b) };
    let mut tg = std::mem::take(&mut a.2);
    tg.extend(b.2.iter().cloned());
    tg.sort();
    tg.dedup();
    let mut w = if b.0 == "high" && a.0 != "high" { b } else { a };
    w.2 = tg;
    Some(w)
}

/// (level, reason, targets) for the worst file-system effect of a typed shell command, or None.
pub fn shell_effects(cmdline: &str, cwd: &str) -> Option<Effect> {
    let mut worst: Option<Effect> = None;
    let mut cwd_s = cwd.to_string();
    let mut prev: Vec<String> = vec![];
    let mut piped = false;
    for (mut argv, redir) in commands(cmdline) {
        if argv.len() == 1 && argv[0] == "__PIPE__" {
            piped = true;
            continue;
        }
        let from_pipe = std::mem::replace(&mut piped, false);
        let prev_cmd = std::mem::replace(&mut prev, argv.clone());
        let cwd = cwd_s.clone();
        let cwd = cwd.as_str();
        // `sh -c "..."` / `bash -c '...'`: check the inner script the same way
        if argv.len() >= 3 && SHELLS.contains(&basename(&argv[0]).as_str()) && argv[1] == "-c" {
            if let Some(rr) = shell_effects(&argv[2], cwd) {
                worst = merge(worst, rr);
            }
            continue;
        }
        if from_pipe && argv.first().map(|a| basename(a) == "xargs").unwrap_or(false) {
            if let Some(fe) = xargs_as_find_exec(&prev_cmd, &argv) {
                argv = fe;
            } else if let Some(unrolled) = xargs_from_echo(&prev_cmd, &argv) {
                // `echo dir1 dir2 | xargs -n 1 cp file1`: judge the commands xargs would build
                if let Some(rr) = shell_effects(&unrolled, cwd) {
                    worst = merge(worst, rr);
                }
                continue;
            }
        }
        if argv.first().map(|a| a == "cd").unwrap_or(false) {
            // `cd X && mv a b`: later commands of the same line run in X
            let to = argv.get(1).cloned().unwrap_or_else(crate::util::home);
            let np = p(&to, cwd);
            if is_dir(&np) {
                cwd_s = np;
            }
            continue;
        }
        let mut r: Option<Effect> = None;
        for t0 in &redir {
            let tgt = p(t0, cwd);
            if t0 != "/dev/null" && nonempty_file(&tgt) {
                r = Some(("high".into(), format!("`>` would overwrite existing file {}", relpath(&tgt, cwd)), vec![canon(&tgt)]));
            }
        }
        while !argv.is_empty() && (["sudo", "env", "nohup", "time", "do", "then"].contains(&argv[0].as_str())
            || (argv[0].contains('=') && !argv[0].starts_with('-'))) {
            argv.remove(0);
        }
        let tool = argv.first().map(|a| basename(a)).unwrap_or_default();
        if r.is_none() {
            r = match tool.as_str() {
                "cp" | "install" => cp_like(&argv, cwd),
                "mv" | "rsync" => cp_like(&argv, cwd).or_else(|| move_away(&argv, cwd)),
                "find" => find_exec(&argv, cwd),
                "chmod" => chmod(&argv, false, cwd),
                "soffice" | "libreoffice" | "lowriter" | "localc" | "loimpress" | "soffice.bin" => soffice_convert(&argv, cwd),
                "rm" => {
                    let tg: Vec<String> = argv[1..].iter().filter(|a| !a.starts_with('-')).cloned().collect();
                    // the agent's own scratch (created during this session) is not user data
                    let start = SESSION_START.get().copied();
                    let unresolved = expand(&tg, cwd).iter().any(|x| x.contains('$') || x.contains('`'));
                    if unresolved {
                        Some(("high".into(), format!("`rm` deletes {} (a path the gate cannot resolve)", tg.iter().take(3).cloned().collect::<Vec<_>>().join(" ")), vec![]))
                    } else if expand(&tg, cwd).iter().any(|x| exists(x) && !session_owned(x, start)) {
                        let rec = argv[1..].iter().any(|a| a.starts_with('-') && a.to_lowercase().contains('r'));
                        let gone: Vec<String> = expand(&tg, cwd).into_iter().filter(|x| exists(x) && !session_owned(x, start)).collect();
                        Some((if rec { "high" } else { "medium" }.into(),
                              format!("`rm` deletes {}", tg.iter().take(3).cloned().collect::<Vec<_>>().join(" ")), canon_all(&gone)))
                    } else {
                        None
                    }
                }
                _ => None,
            };
        }
        if let Some(rr) = r {
            worst = merge(worst, rr);
        }
    }
    worst
}

fn norm_btn(label: &str) -> String {
    label.trim().to_lowercase().replace(['_', '&'], "")
}

/// True when the visible window asks to replace an existing file and this action says yes.
pub fn overwrite_dialog(blob: &str, label: &str, keys: &HashSet<String>) -> bool {
    if blob.is_empty() || !overwrite_ctx().is_match(blob) {
        return false;
    }
    if in_document_ctx().is_match(blob) && !file_words().is_match(blob) {
        return false;
    }
    let lab = norm_btn(label);
    CONFIRM_LABELS.contains(&lab.as_str()) || ["return", "enter", "kp_enter", "y", "alt+y"].iter().any(|k| keys.contains(*k))
}

/// "C:\\dir", "c:/dir" or "\\\\server\\share": an absolute Windows path.
pub fn win_abs(p: &str) -> bool {
    let b = p.as_bytes();
    (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')) || p.starts_with("\\\\")
}

/// An existing file that a file dialog's Save/Export button would replace, read from the dialog's own fields.
pub fn save_dialog_target(text: &str) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    let vals: Vec<String> = value_rx().captures_iter(text).map(|c| c[1].trim().to_string()).collect();
    let (mut dirs, mut names, mut full) = (vec![], vec![], vec![]);
    for v in vals {
        if v.is_empty() || v.chars().count() > 300 || digits_dots().is_match(&v) {
            continue;
        }
        // a tree that escapes backslashes shows C:\\\\Users: one is meant
        let v = if win_abs(&v) && v.contains("\\\\") { v.replace("\\\\", "\\") } else { v };
        let pth = expanduser(&v);
        // a Windows file dialog shows "C:\\Users\\..." (or a UNC "\\\\server\\share"): absolute too
        if pth.starts_with('/') || win_abs(&pth) {
            if is_dir(&pth) { dirs.push(pth) } else { full.push(pth) }
        } else if !v.contains('/') && !v.contains('\\') && !v.contains('\n') {
            names.push(v);
        }
    }
    let mut exts: Vec<String> = vec![];
    for c in ext_rx().captures_iter(text) {
        let e = c[1].to_lowercase();
        if !exts.contains(&e) {
            exts.push(e);
        }
    }
    for f in &full {
        if nonempty_file(f) {
            return Some(f.clone());
        }
    }
    for d in &dirs {
        for n in &names {
            let cands: Vec<String> = if !splitext_ext(n).is_empty() {
                vec![n.clone()]
            } else {
                let rx = Regex::new(&format!(r"{}\.([A-Za-z0-9]{{1,6}})\b", regex::escape(n))).ok();
                let mut list: Vec<String> = exts.iter().take(1).cloned().collect();
                if let Some(rx) = rx {
                    for c in rx.captures_iter(text) {
                        list.push(c[1].to_lowercase());
                    }
                }
                let mut seen = vec![];
                for e in list {
                    if !seen.contains(&e) {
                        seen.push(e);
                    }
                }
                seen.into_iter().map(|e| format!("{n}.{e}")).collect()
            };
            for c in cands {
                let pth = join(d, &c);
                if nonempty_file(&pth) {
                    return Some(pth);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn macos_process_table_and_cwd_parsing() {
        let t = "  1     0 /sbin/launchd\n 501   1 /System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal\n 777 501 login\n 778 777 -zsh\n 900 778 /bin/sleep\n";
        let (kids, comm) = parse_ps_table(t);
        assert_eq!(comm[&501], "Terminal");
        assert_eq!(comm[&778], "zsh");
        assert_eq!(kids[&777], vec![778]);
        assert!(terminal_comm().is_match(&comm[&501]));
        assert!(terminal_comm().is_match(&exe_name("/Applications/iTerm.app/Contents/MacOS/iTerm2")));
        assert!(!terminal_comm().is_match(&exe_name("/Applications/Safari.app/Contents/MacOS/Safari")));
        assert_eq!(parse_lsof_cwd("p778\nfcwd\nn/Users/me/work dir\n").as_deref(), Some("/Users/me/work dir"));
        assert_eq!(parse_lsof_cwd("p778\n"), None);
    }

    #[test]
    fn windows_absolute_paths() {
        for p in ["C:\\Users\\a", "d:/x", "\\\\server\\share\\f"] {
            assert!(super::win_abs(p), "{p}");
        }
        for p in ["/home/a", "Report.pdf", "C:", "1:\\x", "ab:\\x"] {
            assert!(!super::win_abs(p), "{p}");
        }
    }

    use super::*;

    fn setup() -> String {
        let d = std::env::temp_dir().join(format!("gg-sfx-{}", crate::util::rand_hex(6)));
        let d = d.to_string_lossy().to_string();
        for sub in ["dir1", "dir2", "src/a", "out/src/a"] {
            fs::create_dir_all(format!("{d}/{sub}")).unwrap();
        }
        fs::write(format!("{d}/file1"), "new\n").unwrap();
        fs::write(format!("{d}/dir2/file1"), "keep me\n").unwrap();
        fs::write(format!("{d}/src/a/x.ipynb"), "{}").unwrap();
        fs::write(format!("{d}/out/src/a/x.ipynb"), "old").unwrap();
        d
    }

    fn lvl(r: Option<Effect>) -> Option<String> {
        r.map(|x| x.0)
    }

    #[test]
    fn cp_into_dir_with_existing_file() {
        let d = setup();
        assert!(shell_effects("cp file1 dir1", &d).is_none());
        let r = shell_effects("cp file1 dir1 && cp file1 dir2", &d).unwrap();
        assert_eq!(r.0, "high");
        assert!(r.1.contains("dir2/file1"));
        assert!(shell_effects("cp -n file1 dir2", &d).is_none());
        // a loop is judged like its unrolled commands (was a blind spot before 0.9.6 R11)
        assert_eq!(lvl(shell_effects("for d in dir1 dir2; do cp file1 $d; done", &d)).as_deref(), Some("high"));
        assert_eq!(lvl(shell_effects("for d in dir1 dir2; do cp file1 \"$d\"/; done", &d)).as_deref(), Some("high"));
        assert!(shell_effects("for d in dir1; do cp file1 ${d}/; done", &d).is_none());
        assert!(shell_effects("for f in $(ls); do cp file1 $f; done", &d).is_none());
    }

    #[test]
    fn redirect_overwrite() {
        let d = setup();
        assert_eq!(lvl(shell_effects("echo hi > dir2/file1", &d)).as_deref(), Some("high"));
        assert!(shell_effects("echo hi >> dir2/file1", &d).is_none());
        assert!(shell_effects("echo hi > newfile", &d).is_none());
    }

    #[test]
    #[cfg(unix)] // bash command analysis: run_command is not offered on Windows
    fn recursive_copy_and_parents() {
        let d = setup();
        assert_eq!(lvl(shell_effects("cp -r src out", &d)).as_deref(), Some("high"));
        assert_eq!(lvl(shell_effects("cp --parents src/a/x.ipynb out", &d)).as_deref(), Some("high"));
        assert_eq!(lvl(shell_effects("find src -name '*.ipynb' -exec cp --parents {} out \\;", &d)).as_deref(), Some("high"));
        assert!(shell_effects("find src -name '*.ipynb' -exec cp --parents {} dir1 \\;", &d).is_none());
    }

    #[test]
    fn chmod_rules() {
        let d = setup();
        assert_eq!(lvl(shell_effects("chmod -R 644 .", &d)).as_deref(), Some("high"));
        assert_eq!(lvl(shell_effects("chmod -R 755 .", &d)).as_deref(), Some("medium"));
        assert!(shell_effects("find . -type f -exec chmod 644 {} \\;", &d).is_none());
        assert!(shell_effects("chmod 644 file1", &d).is_none());
    }

    #[test]
    #[cfg(unix)] // bash command analysis: run_command is not offered on Windows
    fn find_xargs_and_sh_c() {
        let d = setup();
        assert_eq!(lvl(shell_effects("find src -name '*.ipynb' -print0 | xargs -0 -I{} cp --parents \"{}\" out/", &d)).as_deref(), Some("high"));
        assert!(shell_effects("find src -name '*.ipynb' -print0 | xargs -0 -I{} cp --parents \"{}\" dir1/", &d).is_none());
        assert_eq!(lvl(shell_effects("find . -maxdepth 1 -name file1 | xargs cp -t dir2", &d)).as_deref(), Some("high"));
        assert!(shell_effects("ls | xargs echo", &d).is_none());
        assert_eq!(lvl(shell_effects("bash -c 'mv dir2 dir2.old'", &d)).as_deref(), Some("high"));
        assert!(shell_effects("sh -c 'ls -la'", &d).is_none());
    }

    #[test]
    #[cfg(unix)] // bash command analysis: run_command is not offered on Windows
    fn cd_in_the_same_line_moves_the_cwd() {
        let d = setup();
        let r = shell_effects(&format!("cd {d} && mv dir2 dir2.bak"), "/").unwrap();
        assert!(r.1.contains("would move the existing dir2"), "{}", r.1);
        assert!(shell_effects(&format!("cd {d}/dir1; cp ../file1 ../dir2/"), "/").unwrap().1.contains("overwrite"));
        assert!(shell_effects("cd /nonexistent-gg && rm -rf x", "/").is_none());
    }

    /// pure check of the rm rule against session-owned paths
    #[test]
    fn rm_of_session_scratch_is_not_user_data() {
        let d = setup();
        let past = std::time::SystemTime::now() - Duration::from_secs(3600);
        assert!(session_owned(&format!("{d}/dir2"), Some(past)));
    }

    #[test]
    #[cfg(unix)] // bash command analysis: run_command is not offered on Windows
    fn mv_moves_existing_data_away() {
        let d = setup();
        let r = shell_effects("mv dir2 dir2.bak && git clone https://x/y dir2", &d).unwrap();
        assert_eq!(r.0, "high");
        assert!(r.1.contains("would move the existing dir2"), "{}", r.1);
        // overwrite still reported as an overwrite
        assert!(shell_effects("mv file1 dir2", &d).unwrap().1.contains("overwrite"));
        assert!(shell_effects("mv nothere x", &d).is_none());
        assert!(shell_effects("mv /tmp/gg-nonexistent-scratch/* dir1/", &d).is_none());
        assert!(shell_effects("rsync -a dir1/ dir3/", &d).is_none());
        assert_eq!(lvl(shell_effects("rsync -a --remove-source-files dir1/ dir3/", &d)).as_deref(), Some("high"));
        let before = std::time::SystemTime::now() - Duration::from_secs(3600);
        assert!(session_owned(&format!("{d}/dir2"), Some(before)));
        assert!(!session_owned(&format!("{d}/dir2"), Some(std::time::SystemTime::now() + Duration::from_secs(3600))));
        assert!(!session_owned(&format!("{d}/dir2"), None));
    }

    /// Offline audit: GG_AUDIT=<jsonl {id, cmd, cwd, task}> GG_AUDIT_OUT=<tsv> cargo test -- --ignored sidefx_audit
    /// Classifies recorded terminal commands against a fixture file system (paths with a future mtime count as
    /// created in this session) and checks the user's wording, as the live gate would at Enter.
    #[test]
    #[ignore]
    fn sidefx_audit() {
        let (Ok(inp), Ok(outp)) = (std::env::var("GG_AUDIT"), std::env::var("GG_AUDIT_OUT")) else { return };
        mark_session_start();
        let mut out = String::new();
        for line in fs::read_to_string(inp).unwrap().lines() {
            let r: serde_json::Value = serde_json::from_str(line).unwrap();
            let (cmd, cwd, task) = (r["cmd"].as_str().unwrap(), r["cwd"].as_str().unwrap(), r["task"].as_str().unwrap_or(""));
            let eff = shell_effects(cmd, cwd);
            let (lvl, reason, _) = eff.clone().unwrap_or(("-".into(), "-".into(), vec![]));
            let (hz, ok, why) = match &eff {
                Some(e) => {
                    let (ok, why) = crate::authz::authorized(&crate::authz::command_key(e, cmd), task, &[], Some(cwd));
                    (crate::authz::hazard_of(&e.1).to_string(), ok.to_string(), why)
                }
                None => ("-".into(), "-".into(), "-".into()),
            };
            out.push_str(&format!("{}\t{lvl}\t{hz}\t{ok}\t{}\t{}\n", r["id"].as_str().unwrap(), reason.replace('\t', " "), why.replace('\t', " ")));
        }
        fs::write(outp, out).unwrap();
    }

    #[test]
    fn rm_rules() {
        let d = setup();
        assert_eq!(lvl(shell_effects("rm file1", &d)).as_deref(), Some("medium"));
        assert_eq!(lvl(shell_effects("rm -rf dir2", &d)).as_deref(), Some("high"));
        assert!(shell_effects("rm nothere", &d).is_none());
        assert_eq!(lvl(shell_effects("rm -rf $GG_UNSET_VAR_X/data", &d)).as_deref(), Some("high"));
    }

    #[test]
    fn overwrite_dialog_rules() {
        let blob = "A file named \"pre.pptx\" already exists. Do you want to overwrite it? Yes No";
        let none = HashSet::new();
        assert!(overwrite_dialog(blob, "Yes", &none));
        assert!(!overwrite_dialog(blob, "No", &none));
        let ret: HashSet<String> = ["return".to_string()].into_iter().collect();
        assert!(overwrite_dialog(blob, "", &ret));
        assert!(!overwrite_dialog("Save Document? Save Don't Save", "Save", &none));
        // pasting over cells is undoable, not a file overwrite
        assert!(!overwrite_dialog("You are pasting data into cells that already contain data. Overwrite?", "Yes", &none));
    }

    #[test]
    #[cfg(unix)] // bash command analysis: run_command is not offered on Windows
    fn export_over_existing() {
        let d = setup();
        fs::write(format!("{d}/Report.pdf"), "signed").unwrap();
        fs::write(format!("{d}/Report.docx"), "doc").unwrap();
        let txt = format!("- dialog = \"Export\"\n  - [0] text \"{d}\" value=\"{d}\" [actions=[activate]]\n  - [2] combo box \"PDF - Portable Document Format (.pdf)\"\n  - [44] text \"Report\" value=\"Report\" [actions=[activate]]\n  - [47] button \"Export\"");
        assert_eq!(save_dialog_target(&txt), Some(format!("{d}/Report.pdf")));
        assert!(save_dialog_target(&txt.replace("value=\"Report\"", "value=\"Report2\"")).is_none());
        assert!(save_dialog_target(&txt.replace("(.pdf)", "(.odt)")).is_none());
    }

    #[test]
    #[cfg(unix)] // bash command analysis: run_command is not offered on Windows
    fn libreoffice_export_dialog_existing_pdf() {
        let fx = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures_og02_export.txt");
        let Ok(raw) = fs::read_to_string(fx) else { return };
        let d = setup();
        let txt = raw.replace("/home/user/Desktop", &d);
        assert!(save_dialog_target(&txt).is_none());
        fs::write(format!("{d}/View_Person_Organizational_Summary.pdf"), "%PDF-1.4 x").unwrap();
        assert!(save_dialog_target(&txt).unwrap().ends_with(".pdf"));
    }

    #[test]
    fn tokenizer_like_shlex() {
        assert_eq!(tokenize("echo 'a b' \"c\\\"d\" e>f").unwrap(), vec!["echo", "a b", "c\"d", "e", ">", "f"]);
        assert_eq!(tokenize("a&&b # c").unwrap(), vec!["a", "&&", "b"]);
        assert!(tokenize("echo 'x").is_none());
        assert_eq!(normpath("/a/./b/../c"), "/a/c");
        assert_eq!(relpath("/a/b/c", "/a"), "b/c");
        assert_eq!(relpath("/x/y", "/a/b"), "../../x/y");
    }
}
