//! Leave no residue: `gui-gate cleanup` runs after every install / update (the installers call it) and removes
//! what older versions left behind; `gui-gate uninstall` removes gui-gate completely.
//!
//! What counts as ours, and is therefore cleaned:
//!  * temp/old copies next to the binary (.gui-gate.new, gui-gate.exe.old, gui-gate.old);
//!  * other gui-gate copies on PATH or in the known install dirs (old release copies, the Python package of
//!    0.2-0.5 with its entry script), uninstalling the Python package with its own pip;
//!  * Cua Driver's Python package, only when gui-gate's Python package pulled it in, nothing else needs it and the
//!    official driver is installed;
//!  * PATH entries the installers added: duplicate or stale `# gui-gate` lines in shell profiles, duplicate or stale
//!    gui-gate dirs in the Windows user Path (registry HKCU\Environment);
//!  * MCP client entries pointing at a removed copy (repointed to this binary) and flags that no longer exist;
//!    `.bak` copies of client configs that differ from the current file only by the gui-gate entry;
//!  * the update-check cache except its current file, review temp files, logs older than GUI_GATE_LOG_KEEP_DAYS (30).
//! Nothing else is touched: files owned by other tools, or that need admin rights, are only reported.
use crate::util::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

pub struct Report {
    pub dry: bool,
    pub done: Vec<String>,
    pub left: Vec<String>,
}

impl Report {
    fn act(&mut self, what: String, f: impl FnOnce() -> bool) {
        if self.dry {
            self.done.push(format!("would {what}"));
        } else if f() {
            self.done.push(what);
        } else {
            self.left.push(format!("could not {what}"));
        }
    }
}

fn exe_name() -> &'static str {
    if cfg!(windows) { "gui-gate.exe" } else { "gui-gate" }
}

fn canon(p: &Path) -> PathBuf {
    let c = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    // Windows canonical paths carry the \\?\ prefix; configs and Path entries never should
    let t = c.to_string_lossy().to_string();
    match t.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => c,
    }
}

fn read_text(p: &Path) -> Option<String> {
    std::fs::read_to_string(p).ok().map(|t| t.trim_start_matches('\u{feff}').to_string())
}

pub fn current_exe() -> PathBuf {
    std::env::current_exe().map(|p| canon(&p)).unwrap_or_else(|_| PathBuf::from("gui-gate"))
}

fn rm(p: &Path) -> bool {
    if p.is_dir() { std::fs::remove_dir_all(p).is_ok() } else { std::fs::remove_file(p).is_ok() }
}

fn older_than(p: &Path, secs: u64) -> bool {
    p.metadata().and_then(|m| m.modified()).ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok()).map(|d| d > Duration::from_secs(secs)).unwrap_or(false)
}

/// The install dirs gui-gate ever used, plus PATH.
fn candidate_dirs() -> Vec<PathBuf> {
    let h = home();
    let mut v: Vec<PathBuf> = vec![PathBuf::from(format!("{h}/.local/bin"))];
    if let Some(la) = env("LOCALAPPDATA") {
        v.push(PathBuf::from(la).join("Programs").join("gui-gate").join("bin"));
    }
    if let Some(p) = std::env::var_os("PATH") {
        v.extend(std::env::split_paths(&p));
    }
    let mut seen = std::collections::HashSet::new();
    v.into_iter().filter(|d| seen.insert(canon(d))).collect()
}

/// A gui-gate Python entry script (0.2-0.5): returns the Python it runs with.
fn python_entry(p: &Path) -> Option<String> {
    let bytes = std::fs::read(p).ok()?;
    if bytes.len() > 200_000 {
        return None; // a native binary
    }
    let t = String::from_utf8_lossy(&bytes);
    if !t.contains("gui_gate") {
        return None;
    }
    let first = t.lines().next().unwrap_or("");
    first.strip_prefix("#!").map(|s| s.trim().split_whitespace().next().unwrap_or("python3").to_string())
        .or_else(|| Some("python3".into()))
}

fn version_of(p: &Path) -> Option<String> {
    let o = Command::new(p).arg("--version").output().ok()?;
    let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
    let ok = v.split('.').count() == 3 && v.split('.').all(|x| x.chars().all(|c| c.is_ascii_digit()));
    ok.then_some(v)
}

fn writable(dir: &Path) -> bool {
    let t = dir.join(format!(".gui-gate-w-{}", rand_hex(4)));
    let ok = std::fs::write(&t, b"").is_ok();
    let _ = std::fs::remove_file(&t);
    ok
}

fn pip(python: &str, args: &[&str]) -> Option<String> {
    let o = Command::new(python).args(["-m", "pip"]).args(args).env("PIP_DISABLE_PIP_VERSION_CHECK", "1").output().ok()?;
    let out = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    if o.status.success() {
        Some(out)
    } else if out.contains("externally-managed") && !args.contains(&"--break-system-packages") {
        let mut a = args.to_vec();
        a.push("--break-system-packages");
        pip(python, &a)
    } else {
        None
    }
}

fn pip_show(python: &str, pkg: &str) -> Option<String> {
    pip(python, &["show", pkg])
}

/// Uninstall gui-gate's old Python package (and Cua Driver's, when only gui-gate needed it) from one Python.
fn clean_python(python: &str, r: &mut Report) {
    if pip_show(python, "gui-gate").is_none() {
        return;
    }
    r.act(format!("uninstall the old gui-gate Python package ({python})"), || pip(python, &["uninstall", "-y", "gui-gate"]).is_some());
    if let Some(show) = pip_show(python, "cua-driver") {
        let required_by = show.lines().find_map(|l| l.strip_prefix("Required-by:")).map(|s| s.trim().to_string()).unwrap_or_default();
        let official = crate::install::official_driver_paths().iter().any(|p| Path::new(p).exists() && !crate::install::driver_is_pip(p));
        if required_by.is_empty() && official {
            r.act(format!("uninstall Cua Driver's Python package that gui-gate pulled in ({python}); the official driver stays"),
                  || pip(python, &["uninstall", "-y", "cua-driver"]).is_some());
        }
    }
}

/// Paths of gui-gate copies that are not this binary and were removed (so configs pointing there get repointed).
fn clean_copies(r: &mut Report) -> Vec<PathBuf> {
    let me = current_exe();
    let my_dir = me.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    let mut gone = vec![];
    for n in [".gui-gate.new", "gui-gate.exe.old", "gui-gate.old", ".gui-gate.exe.new"] {
        let p = my_dir.join(n);
        if p.exists() {
            r.act(format!("remove {}", p.display()), || rm(&p));
        }
    }
    let mut pythons: Vec<String> = vec![];
    for d in candidate_dirs() {
        let p = d.join(exe_name());
        if !p.is_file() || canon(&p) == me {
            continue;
        }
        if let Some(py) = python_entry(&p) {
            if !pythons.contains(&py) {
                pythons.push(py.clone());
            }
            continue; // pip removes its own entry script
        }
        let Some(v) = version_of(&p) else { continue }; // not ours
        if !writable(&d) {
            r.left.push(format!("old gui-gate {v} at {} needs admin rights to remove", p.display()));
            continue;
        }
        r.act(format!("remove old gui-gate {v} at {}", p.display()), || rm(&p));
        gone.push(p);
    }
    for py in ["python3", "python"].iter().map(|s| s.to_string()) {
        if crate::install::which(&py).is_some() && !pythons.contains(&py) {
            pythons.push(py);
        }
    }
    for py in &pythons {
        let before: Vec<PathBuf> = candidate_dirs().into_iter().map(|d| d.join(exe_name())).filter(|p| p.is_file() && canon(p) != me).collect();
        clean_python(py, r);
        gone.extend(before.into_iter().filter(|p| !p.exists()));
    }
    gone
}

// ------------------------------------------------------------------ PATH
fn profile_files() -> Vec<PathBuf> {
    let h = home();
    [".bashrc", ".bash_profile", ".zshrc", ".profile"].iter().map(|f| PathBuf::from(format!("{h}/{f}"))).collect()
}

/// `# gui-gate` + `export PATH="DIR:$PATH"` blocks: keep one per live dir, drop the rest.
pub fn clean_profile_text(text: &str, keep_dir: &str) -> (String, usize) {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<&str> = vec![];
    let mut kept = false;
    let mut removed = 0;
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() == "# gui-gate" && i + 1 < lines.len() && lines[i + 1].trim_start().starts_with("export PATH=\"") {
            let dir = lines[i + 1].trim().trim_start_matches("export PATH=\"").split(':').next().unwrap_or("").to_string();
            if !kept && dir == keep_dir {
                kept = true;
                out.push(lines[i]);
                out.push(lines[i + 1]);
            } else {
                removed += 1;
                if out.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
                    out.pop(); // the blank line the installer put before the block
                }
            }
            i += 2;
            continue;
        }
        out.push(lines[i]);
        i += 1;
    }
    (out.join("\n"), removed)
}

fn clean_profiles(r: &mut Report, keep_dir: &str) {
    for f in profile_files() {
        let Some(t) = read_text(&f) else { continue };
        let (new, n) = clean_profile_text(&t, keep_dir);
        if n > 0 {
            r.act(format!("remove {n} stale/duplicate gui-gate PATH line(s) from {}", f.display()), || std::fs::write(&f, &new).is_ok());
        }
    }
}

/// Windows user Path (HKCU\Environment): drop duplicate and stale gui-gate dirs, keep the current one once.
pub fn clean_path_list(entries: &[String], keep_dir: &str) -> (Vec<String>, usize) {
    let norm = |s: &str| s.trim().trim_end_matches(['\\', '/']).to_lowercase();
    let mut out: Vec<String> = vec![];
    let mut removed = 0;
    for e in entries {
        if e.trim().is_empty() {
            continue;
        }
        let ours = norm(e).contains("gui-gate");
        let dup = out.iter().any(|o| norm(o) == norm(e));
        let stale = ours && norm(e) != norm(keep_dir) && !Path::new(e.trim()).join(exe_name()).is_file();
        if (ours && dup) || stale {
            removed += 1;
            continue;
        }
        out.push(e.clone());
    }
    (out, removed)
}

#[cfg(windows)]
fn clean_windows_path(r: &mut Report, keep_dir: &str) {
    let get = Command::new("powershell").args(["-NoProfile", "-Command", "[Environment]::GetEnvironmentVariable('Path','User')"]).output();
    let Ok(o) = get else { return };
    let cur = String::from_utf8_lossy(&o.stdout).trim().to_string();
    let entries: Vec<String> = cur.split(';').map(String::from).collect();
    let (new, n) = clean_path_list(&entries, keep_dir);
    if n > 0 {
        let joined = new.join(";").replace('\'', "''");
        r.act(format!("remove {n} stale/duplicate gui-gate entr(ies) from the user Path (registry HKCU\\Environment)"), || {
            Command::new("powershell").args(["-NoProfile", "-Command",
                &format!("[Environment]::SetEnvironmentVariable('Path','{joined}','User')")]).status().map(|s| s.success()).unwrap_or(false)
        });
    }
}

#[cfg(not(windows))]
fn clean_windows_path(_r: &mut Report, _keep_dir: &str) {}

// ------------------------------------------------------------------ MCP client configs
fn is_gate_cmd(cmd: &str, args: &[String]) -> bool {
    let b = cmd.rsplit(['/', '\\']).next().unwrap_or(cmd).to_lowercase();
    b == "gui-gate" || b == "gui-gate.exe" || (b.starts_with("python") && args.windows(2).any(|w| w[0] == "-m" && w[1] == "gui_gate"))
}

/// Fix one server entry in place; true when it changed.
pub fn fix_entry(e: &mut Value, me: &str, gone: &[String]) -> bool {
    let cmd = e.get("command").and_then(|c| c.as_str()).unwrap_or("").to_string();
    let args: Vec<String> = e.get("args").and_then(|a| a.as_array()).map(|a| a.iter().map(|x| pystr(Some(x))).collect()).unwrap_or_default();
    if !is_gate_cmd(&cmd, &args) {
        return false;
    }
    let mut changed = false;
    let mut new_args: Vec<String> = vec![];
    let python_form = !cmd.to_lowercase().contains("gui-gate");
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if python_form && a == "-m" && args.get(i + 1).map(|s| s == "gui_gate").unwrap_or(false) {
            i += 2;
            changed = true;
            continue;
        }
        if a == "--decider" || a == "--gate-ack" {
            i += if a == "--decider" { 2 } else { 1 }; // removed in 0.8: the acting model never approves its own step
            changed = true;
            continue;
        }
        if a.starts_with("--decider=") {
            i += 1;
            changed = true;
            continue;
        }
        if a == "--risk-mode" && args.get(i + 1).map(|s| s == "model").unwrap_or(false) {
            new_args.push(a.clone());
            new_args.push("reviewer".into());
            i += 2;
            changed = true;
            continue;
        }
        new_args.push(a.clone());
        i += 1;
    }
    let stale = python_form || gone.contains(&cmd) || (cmd.contains(['/', '\\']) && !Path::new(&cmd).exists())
        || cmd.starts_with(r"\\?\");
    if stale && cmd != me {
        e["command"] = json!(me);
        changed = true;
    }
    if changed {
        e["args"] = json!(new_args);
    }
    changed
}

fn fix_servers(v: &mut Value, me: &str, gone: &[String]) -> usize {
    let mut n = 0;
    if let Some(m) = v.get_mut("mcpServers").and_then(|m| m.as_object_mut()) {
        for (_, e) in m.iter_mut() {
            n += fix_entry(e, me, gone) as usize;
        }
    }
    n
}

fn without_gate(v: &Value) -> Value {
    let mut v = v.clone();
    let strip = |m: &mut Value| {
        if let Some(o) = m.get_mut("mcpServers").and_then(|m| m.as_object_mut()) {
            let keys: Vec<String> = o.iter().filter(|(_, e)| {
                let cmd = e.get("command").and_then(|c| c.as_str()).unwrap_or("");
                let args: Vec<String> = e.get("args").and_then(|a| a.as_array()).map(|a| a.iter().map(|x| pystr(Some(x))).collect()).unwrap_or_default();
                is_gate_cmd(cmd, &args)
            }).map(|(k, _)| k.clone()).collect();
            for k in keys {
                o.remove(&k);
            }
        }
    };
    strip(&mut v);
    if let Some(ps) = v.get_mut("projects").and_then(|p| p.as_object_mut()) {
        for (_, p) in ps.iter_mut() {
            strip(p);
        }
    }
    v
}

fn write_atomic(path: &Path, text: &str) -> bool {
    let tmp = path.with_extension(format!("gui-gate-tmp-{}", rand_hex(3)));
    if std::fs::write(&tmp, text).is_err() {
        return false;
    }
    std::fs::rename(&tmp, path).is_ok() || { let _ = std::fs::remove_file(&tmp); false }
}

pub fn client_configs() -> Vec<PathBuf> {
    let h = home();
    ["/.pi/agent/mcp.json", "/.cursor/mcp.json", "/.claude.json", "/.codex/config.toml"].iter().map(|p| PathBuf::from(format!("{h}{p}"))).collect()
}

/// The TOML text without any [mcp_servers.X] block whose command is gui-gate (for comparing / uninstalling).
fn toml_without_gate(t: &str) -> String {
    fn flush<'a>(block: &mut Vec<&'a str>, res: &mut Vec<&'a str>) {
        let is_server = block.first().map(|l| l.trim().starts_with("[mcp_servers.")).unwrap_or(false);
        let gate = is_server && block.iter().any(|l| l.trim_start().starts_with("command") && l.contains("gui-gate"));
        if !gate {
            res.extend(block.iter());
        }
        block.clear();
    }
    let mut res: Vec<&str> = vec![];
    let mut block: Vec<&str> = vec![];
    for l in t.lines() {
        if l.trim().starts_with('[') {
            flush(&mut block, &mut res);
        }
        block.push(l);
    }
    flush(&mut block, &mut res);
    let mut out = res.join("\n");
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    out.trim().to_string()
}

fn fix_toml(t: &str, me: &str, gone: &[String]) -> (String, usize) {
    let mut n = 0;
    let mut out = vec![];
    let mut in_gate = false;
    let lines: Vec<&str> = t.lines().collect();
    for (i, l) in lines.iter().enumerate() {
        let s = l.trim();
        if s.starts_with('[') {
            in_gate = s.starts_with("[mcp_servers.")
                && lines[i + 1..].iter().take_while(|x| !x.trim().starts_with('[')).any(|x| x.trim_start().starts_with("command") && x.contains("gui-gate"));
        }
        if in_gate && s.starts_with("command") {
            let cmd = s.splitn(2, '=').nth(1).map(|v| v.trim().trim_matches('"').to_string()).unwrap_or_default();
            if (gone.contains(&cmd) || !Path::new(&cmd).exists()) && cmd != me {
                out.push(format!("command = {}", json!(me)));
                n += 1;
                continue;
            }
        }
        if in_gate && s.starts_with("args") {
            if let Some(v) = s.splitn(2, '=').nth(1).and_then(|v| serde_json::from_str::<Value>(v.trim()).ok()) {
                let mut e = json!({"command": "gui-gate", "args": v});
                if fix_entry(&mut e, "gui-gate", &[]) {
                    out.push(format!("args = {}", e["args"]));
                    n += 1;
                    continue;
                }
            }
        }
        out.push(l.to_string());
    }
    let mut s = out.join("\n");
    if t.ends_with('\n') {
        s.push('\n');
    }
    (s, n)
}

fn clean_configs(r: &mut Report, gone: &[PathBuf]) {
    let me = current_exe().to_string_lossy().to_string();
    let gone: Vec<String> = gone.iter().map(|p| p.to_string_lossy().to_string()).collect();
    for f in client_configs() {
        let Some(t) = read_text(&f) else { continue };
        let is_toml = f.extension().map(|e| e == "toml").unwrap_or(false);
        if is_toml {
            let (new, n) = fix_toml(&t, &me, &gone);
            if n > 0 {
                r.act(format!("update {n} gui-gate setting(s) in {}", f.display()), || write_atomic(&f, &new));
            }
            let bak = PathBuf::from(format!("{}.bak", f.display()));
            if let Some(b) = read_text(&bak) {
                let cur = read_text(&f).unwrap_or(new);
                if toml_without_gate(&b) == toml_without_gate(&cur) {
                    r.act(format!("remove {} (gui-gate's own backup, nothing else differs)", bak.display()), || rm(&bak));
                }
            }
            continue;
        }
        let Ok(mut v) = serde_json::from_str::<Value>(&t) else { continue };
        let mut n = fix_servers(&mut v, &me, &gone);
        if let Some(ps) = v.get_mut("projects").and_then(|p| p.as_object_mut()) {
            for (_, p) in ps.iter_mut() {
                n += fix_servers(p, &me, &gone);
            }
        }
        if n > 0 {
            let text = serde_json::to_string_pretty(&v).unwrap_or_default();
            r.act(format!("update {n} gui-gate server entr(ies) in {}", f.display()), || write_atomic(&f, &text));
        }
        let bak = PathBuf::from(format!("{}.bak", f.display()));
        if let Some(b) = read_text(&bak).and_then(|b| serde_json::from_str::<Value>(&b).ok()) {
            if without_gate(&b) == without_gate(&v) {
                r.act(format!("remove {} (gui-gate's own backup, nothing else differs)", bak.display()), || rm(&bak));
            }
        }
    }
}

// ------------------------------------------------------------------ caches, temp, logs
/// ~/.config/gui-gate (Windows: %USERPROFILE%\.config\gui-gate): reviewer.env, reviewer.key, role key files,
/// memory.jsonl from `gui-gate memory add`.
pub fn config_dir() -> PathBuf {
    PathBuf::from(home()).join(".config").join("gui-gate")
}

pub fn cache_dir() -> PathBuf {
    PathBuf::from(env("XDG_CACHE_HOME").unwrap_or_else(|| format!("{}/.cache", home()))).join("gui-gate")
}

pub fn log_dir() -> PathBuf {
    PathBuf::from(env("GUI_GATE_LOG_DIR").unwrap_or_else(|| format!("{}/.gui-gate/logs", home())))
}

fn clean_files(r: &mut Report) {
    if let Ok(rd) = std::fs::read_dir(cache_dir()) {
        for e in rd.flatten() {
            if e.file_name() != "driver-latest.json" {
                let p = e.path();
                r.act(format!("remove old cache file {}", p.display()), || rm(&p));
            }
        }
    }
    for p in temp_files(3600) {
        r.act(format!("remove temp file {}", p.display()), || rm(&p));
    }
    let days: u64 = env("GUI_GATE_LOG_KEEP_DAYS").and_then(|d| d.parse().ok()).unwrap_or(30);
    if days > 0 {
        if let Ok(rd) = std::fs::read_dir(log_dir()) {
            let old: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_file() && older_than(p, days * 86400)).collect();
            if !old.is_empty() {
                let n = old.len();
                r.act(format!("remove {n} log file(s) older than {days} days"), || old.iter().all(|p| rm(p)));
            }
        }
    }
}

/// Session logs (~/.gui-gate/logs/*.jsonl): drop those older than GUI_GATE_LOG_KEEP_DAYS (30; 0 = keep all) and,
/// oldest first, whatever exceeds GUI_GATE_LOG_MAX_MB (200; 0 = no cap). Runs at every gate start.
pub fn prune_logs() {
    let Ok(rd) = std::fs::read_dir(log_dir()) else { return };
    let mut files: Vec<(SystemTime, u64, PathBuf)> = rd.flatten().filter_map(|e| {
        let p = e.path();
        let m = e.metadata().ok()?;
        (m.is_file() && p.extension().map(|x| x == "jsonl").unwrap_or(false)).then(|| (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len(), p))
    }).collect();
    let days: u64 = env("GUI_GATE_LOG_KEEP_DAYS").and_then(|d| d.parse().ok()).unwrap_or(30);
    if days > 0 {
        files.retain(|(_, _, p)| if older_than(p, days * 86400) { let _ = std::fs::remove_file(p); false } else { true });
    }
    let cap: u64 = env("GUI_GATE_LOG_MAX_MB").and_then(|d| d.parse().ok()).unwrap_or(200) * 1024 * 1024;
    if cap > 0 {
        files.sort();
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        for (_, n, p) in &files {
            if total <= cap {
                break;
            }
            if std::fs::remove_file(p).is_ok() {
                total -= n;
            }
        }
    }
}

/// The pid a temp name of ours carries (gui-gate-job-<pid>-<n>.log, gui-gate-lo-<pid>).
fn temp_owner(name: &str) -> Option<i32> {
    let rest = name.strip_prefix("gui-gate-job-").or_else(|| name.strip_prefix("gui-gate-lo-"))?;
    rest.split(['-', '.']).next()?.parse().ok()
}

/// gui-gate's own temp files and dirs (job logs, the throwaway LibreOffice profile, review / installer temp dirs)
/// older than `min_age` seconds whose gate is not running any more.
pub fn temp_files(min_age: u64) -> Vec<PathBuf> {
    let mut out = vec![];
    let me = std::process::id() as i32;
    let Ok(rd) = std::fs::read_dir(std::env::temp_dir()) else { return out };
    for e in rd.flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if !n.starts_with("gui-gate-") {
            continue;
        }
        let old = min_age == 0 || older_than(&e.path(), min_age);
        match temp_owner(&n) {
            Some(pid) if pid == me => {}
            // unix knows whether the owning gate still runs; elsewhere only age tells
            Some(pid) if cfg!(unix) => if !crate::proc::alive(pid) { out.push(e.path()) },
            _ => if old { out.push(e.path()) },
        }
    }
    out
}

/// Quiet startup housekeeping (no subprocesses): the renamed exe a Windows update could not delete while it ran, and
/// temp files of gates that are gone.
pub fn startup() {
    for p in temp_files(3600) {
        let _ = rm(&p);
    }
    prune_logs();
    let me = current_exe();
    if let Some(d) = me.parent() {
        for n in ["gui-gate.exe.old", ".gui-gate.new"] {
            let p = d.join(n);
            if p.exists() && older_than(&p, 5) {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
}

pub fn run(dry: bool) -> Report {
    let mut r = Report { dry, done: vec![], left: vec![] };
    let me = current_exe();
    let keep_dir = me.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
    let gone = clean_copies(&mut r);
    clean_profiles(&mut r, &keep_dir);
    clean_windows_path(&mut r, &keep_dir);
    clean_configs(&mut r, &gone);
    clean_files(&mut r);
    r
}

pub fn cleanup_main(argv: &[String]) -> i32 {
    let dry = argv.iter().any(|a| a == "--dry-run");
    let quiet = argv.iter().any(|a| a == "--quiet");
    if argv.iter().any(|a| a == "-h" || a == "--help") {
        println!("usage: gui-gate cleanup [--dry-run] [--quiet]\nremove what older gui-gate versions left behind (the installers run this after every install/update)");
        return 0;
    }
    let r = run(dry);
    if !quiet || !r.done.is_empty() || !r.left.is_empty() {
        println!("{}", serde_json::to_string_pretty(&json!({"cleaned": r.done, "not_cleaned": r.left})).unwrap());
    }
    0
}

/// gui-gate processes other than this one (an MCP client still has the server open), for the uninstall report.
fn running_gates() -> Vec<String> {
    let me = std::process::id();
    let mut out = vec![];
    if cfg!(target_os = "linux") {
        if let Ok(rd) = std::fs::read_dir("/proc") {
            for e in rd.flatten() {
                let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else { continue };
                if pid == me {
                    continue;
                }
                let comm = std::fs::read_to_string(e.path().join("comm")).unwrap_or_default();
                if comm.trim() == "gui-gate" {
                    out.push(pid.to_string());
                }
            }
        }
    } else if cfg!(target_os = "macos") {
        if let Ok(o) = Command::new("pgrep").args(["-x", "gui-gate"]).output() {
            out.extend(String::from_utf8_lossy(&o.stdout).split_whitespace().filter(|p| *p != me.to_string()).map(String::from));
        }
    } else if let Ok(o) = Command::new("tasklist").args(["/FI", "IMAGENAME eq gui-gate.exe", "/FO", "CSV", "/NH"]).output() {
        for l in String::from_utf8_lossy(&o.stdout).lines() {
            if let Some(pid) = l.split(',').nth(1).map(|x| x.trim_matches('"').to_string()) {
                if pid.chars().all(|c| c.is_ascii_digit()) && !pid.is_empty() && pid != me.to_string() {
                    out.push(pid);
                }
            }
        }
    }
    out
}

/// Remove `# Added by cua-driver-rs installer` + its PATH line from shell profiles when that dir no longer has a
/// cua-driver (Cua's own uninstaller leaves the line).
pub fn clean_cua_profile_text(text: &str) -> (String, usize) {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<&str> = vec![];
    let mut n = 0;
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim_start().starts_with("# Added by cua-driver-rs installer") && i + 1 < lines.len() && lines[i + 1].trim_start().starts_with("export PATH=\"") {
            let dir = lines[i + 1].trim().trim_start_matches("export PATH=\"").split(':').next().unwrap_or("").to_string();
            if !Path::new(&dir).join("cua-driver").exists() {
                n += 1;
                if out.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
                    out.pop();
                }
                i += 2;
                continue;
            }
        }
        out.push(lines[i]);
        i += 1;
    }
    (out.join("\n"), n)
}

/// `--with-driver`: an externally installed Cua Driver (Cua's own installer) is removed with Cua's own uninstaller
/// (purging its telemetry id; macOS also revokes its permissions), then the PATH line / user Path entry Cua's
/// installer added and its uninstaller leaves.
fn remove_external_driver(r: &mut Report) {
    let ok = if cfg!(windows) {
        Command::new("powershell").args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", "irm https://cua.ai/driver/uninstall.ps1 | iex"])
            .env("CUA_DRIVER_RS_UNINSTALL_FORCE", "1").env("CUA_DRIVER_RS_UNINSTALL_PURGE", "1").status().map(|s| s.success()).unwrap_or(false)
    } else {
        let mut args = vec!["-c".to_string(), "set -e; s=\"$(curl -fsSL https://cua.ai/driver/uninstall.sh)\"; bash -c \"$s\" cua-driver-uninstall \"$@\"".into(), "x".into(), "--purge".into()];
        if cfg!(target_os = "macos") {
            args.push("--reset-permissions".into());
        }
        Command::new("bash").args(&args).status().map(|s| s.success()).unwrap_or(false)
    };
    if ok { r.done.push("removed the externally installed Cua Driver with Cua's own uninstaller (--purge)".into()) } else { r.left.push("Cua's uninstaller did not finish (see its output above)".into()) }
    for f in profile_files() {
        let Some(t) = read_text(&f) else { continue };
        let (new, n) = clean_cua_profile_text(&t);
        if n > 0 {
            r.act(format!("remove Cua Driver's PATH line from {}", f.display()), || std::fs::write(&f, &new).is_ok());
        }
    }
    #[cfg(windows)]
    if let Some(la) = env("LOCALAPPDATA") {
        let d = format!("{la}\\Programs\\Cua\\cua-driver\\bin").to_lowercase();
        if let Ok(o) = Command::new("powershell").args(["-NoProfile", "-Command", "[Environment]::GetEnvironmentVariable('Path','User')"]).output() {
            let cur = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let keep: Vec<&str> = cur.split(';').filter(|e| !(e.trim().trim_end_matches('\\').to_lowercase() == d && !Path::new(e.trim()).exists())).collect();
            if keep.len() != cur.split(';').count() {
                let joined = keep.join(";").replace('\'', "''");
                r.act("remove Cua Driver's dir from the user Path".into(), || Command::new("powershell").args(["-NoProfile", "-Command",
                    &format!("[Environment]::SetEnvironmentVariable('Path','{joined}','User')")]).status().map(|s| s.success()).unwrap_or(false));
            }
        }
    }
}

/// macOS: the CuaDriver.app gui-gate's installer placed (only that one): stop its daemon, unregister it from
/// LaunchServices, delete it, and drop its Accessibility / Screen Recording rows.
fn remove_bundled_app(r: &mut Report, app: &str) {
    if !Path::new(app).exists() {
        return;
    }
    if r.dry {
        r.done.push(format!("would remove {app} (installed by gui-gate) and its permission entries"));
        return;
    }
    let bin = format!("{app}/Contents/MacOS/cua-driver");
    let _ = crate::proc::run_argv(&[&bin, "stop"], 10.0);
    let ls = "/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister";
    let _ = crate::proc::run_argv(&[ls, "-u", app], 10.0);
    r.act(format!("remove {app} (the Cua Driver app gui-gate installed)"), || rm(Path::new(app)));
    for svc in ["Accessibility", "ScreenCapture"] {
        let _ = crate::proc::run_argv(&["tccutil", "reset", svc, "com.trycua.driver"], 10.0);
    }
    r.done.push("reset the Accessibility / Screen Recording entries of com.trycua.driver".into());
}

/// Remove gui-gate completely: the binary, the bundled Cua Driver (and on macOS the CuaDriver.app only if gui-gate
/// installed it), its PATH entries, its MCP client entries, settings, cache, logs, temp files, and the dirs its
/// installer created. A Cua Driver installed with Cua's own installer stays unless --with-driver.
pub fn uninstall_main(argv: &[String]) -> i32 {
    let dry = argv.iter().any(|a| a == "--dry-run");
    let keep_logs = argv.iter().any(|a| a == "--keep-logs");
    let keep_config = argv.iter().any(|a| a == "--keep-config");
    let with_driver = argv.iter().any(|a| a == "--with-driver");
    if argv.iter().any(|a| a == "-h" || a == "--help") {
        println!("usage: gui-gate uninstall [--dry-run] [--keep-config] [--keep-logs] [--with-driver]
remove gui-gate and everything it created: the binary, the bundled Cua Driver, PATH entries, MCP client entries,
~/.config/gui-gate (reviewer settings and key, memory; --keep-config keeps it), ~/.gui-gate (logs; --keep-logs),
~/.cache/gui-gate, temp files. A Cua Driver installed by Cua's own installer is left alone unless --with-driver
(runs Cua's uninstaller with --purge).");
        return 0;
    }
    let mut r = run(dry);
    let me = current_exe();
    let dir = me.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    let manifest = crate::install::manifest();
    let lib = crate::install::lib_dir();
    // MCP entries
    for f in client_configs() {
        let Some(t) = read_text(&f) else { continue };
        if f.extension().map(|e| e == "toml").unwrap_or(false) {
            let new = toml_without_gate(&t);
            if new.trim() != t.trim() {
                r.act(format!("remove gui-gate from {}", f.display()), || write_atomic(&f, &format!("{new}\n")));
            }
        } else if let Ok(v) = serde_json::from_str::<Value>(&t) {
            let nv = without_gate(&v);
            if nv != v {
                let text = serde_json::to_string_pretty(&nv).unwrap_or_default();
                r.act(format!("remove gui-gate from {}", f.display()), || write_atomic(&f, &text));
            }
        }
    }
    // PATH lines: a `# gui-gate` block is ours whatever else lives in that dir (Cua's installer adds its own line)
    for f in profile_files() {
        let Some(t) = read_text(&f) else { continue };
        let (new, n) = clean_profile_text(&t, "\u{0}"); // keep nothing
        if n > 0 {
            r.act(format!("remove gui-gate PATH line(s) from {}", f.display()), || std::fs::write(&f, &new).is_ok());
        }
    }
    #[cfg(windows)]
    {
        let ours = dir.to_string_lossy().to_lowercase().contains("gui-gate");
        let shared = std::fs::read_dir(&dir).map(|rd| rd.flatten().any(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n != exe_name() && n != "gui-gate.exe.old"
        })).unwrap_or(true);
        if ours || !shared {
            if let Ok(o) = Command::new("powershell").args(["-NoProfile", "-Command", "[Environment]::GetEnvironmentVariable('Path','User')"]).output() {
                let cur = String::from_utf8_lossy(&o.stdout).trim().to_string();
                let d = dir.to_string_lossy().to_lowercase();
                let keep: Vec<&str> = cur.split(';').filter(|e| e.trim().trim_end_matches('\\').to_lowercase() != d.trim_end_matches('\\')).collect();
                if keep.len() != cur.split(';').count() {
                    let joined = keep.join(";").replace('\'', "''");
                    r.act("remove gui-gate from the user Path (registry HKCU\\Environment)".into(), || {
                        Command::new("powershell").args(["-NoProfile", "-Command", &format!("[Environment]::SetEnvironmentVariable('Path','{joined}','User')")])
                            .status().map(|s| s.success()).unwrap_or(false)
                    });
                }
            }
        }
    }
    // the bundled driver: macOS app (only the one gui-gate installed), then gui-gate's own lib dir
    if let Some((_, app)) = manifest.iter().find(|(k, _)| k == "app") {
        remove_bundled_app(&mut r, app);
        // state the app gui-gate installed created (~/.cua-driver, its caches): recorded only when missing before
        for (_, d) in manifest.iter().filter(|(k, _)| k == "owned_dir") {
            let p = PathBuf::from(d);
            if p.exists() {
                r.act(format!("remove {} (created by the Cua Driver app gui-gate installed)", p.display()), || rm(&p));
            }
        }
    }
    if let Some(l) = lib.as_ref() {
        if cfg!(windows) {
            for n in ["cua-driver", "install-manifest"] {
                let p = l.join(n);
                if p.exists() {
                    r.act(format!("remove {}", p.display()), || rm(&p));
                }
            }
            if let Ok(rd) = std::fs::read_dir(l) {
                for e in rd.flatten().filter(|e| e.file_name().to_string_lossy().starts_with("cua-driver.old")) {
                    let p = e.path();
                    r.act(format!("remove {}", p.display()), || rm(&p));
                }
            }
        } else if l.exists() && l.file_name().map(|n| n == "gui-gate").unwrap_or(false) {
            r.act(format!("remove {} (bundled Cua Driver)", l.display()), || rm(l));
        }
    }
    if with_driver {
        if dry { r.done.push("would run Cua's own uninstaller (--purge) for an externally installed Cua Driver".into()) } else { remove_external_driver(&mut r) }
    }
    let cd = cache_dir();
    if cd.exists() {
        r.act(format!("remove {}", cd.display()), || rm(&cd));
    }
    let cfg = config_dir();
    if !keep_config && cfg.exists() {
        r.act(format!("remove {} (reviewer settings and key, role keys, memory)", cfg.display()), || rm(&cfg));
    }
    let root = PathBuf::from(home()).join(".gui-gate");
    if !keep_logs && root.exists() {
        r.act(format!("remove {} (logs)", root.display()), || rm(&root));
    }
    for p in temp_files(0) {
        r.act(format!("remove temp file {}", p.display()), || rm(&p));
    }
    let mut created: Vec<PathBuf> = manifest.iter().filter(|(k, _)| k == "created_dir").map(|(_, v)| PathBuf::from(v)).collect();
    created.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    // the binary last; on Windows a running exe cannot be deleted, so rename it and let a detached cmd delete it and
    // gui-gate's (then empty) install dirs once this process has exited
    if cfg!(windows) {
        let old = dir.join("gui-gate.exe.old");
        let parent = dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| dir.clone());
        r.act(format!("remove {}", me.display()), || {
            let _ = std::fs::remove_file(&old);
            std::fs::rename(&me, &old).is_ok() && {
                let mut script = format!("ping -n 3 127.0.0.1 >nul & del /f /q \"{}\" & rmdir \"{}\" 2>nul & rmdir \"{}\" 2>nul", old.display(), dir.display(), parent.display());
                for d in &created {
                    script.push_str(&format!(" & rmdir \"{}\" 2>nul", d.display()));
                }
                let mut c = Command::new("cmd");
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    // cmd does not understand the \" escaping Rust applies to a quoted argument: pass the line raw
                    c.arg("/C").raw_arg(&script);
                    c.creation_flags(0x0800_0000 | 0x0000_0008); // CREATE_NO_WINDOW | DETACHED_PROCESS
                }
                #[cfg(not(windows))]
                c.args(["/C", &script]);
                let _ = c.spawn();
                true
            }
        });
        r.done.push(format!("{} and {} are removed (when empty) a few seconds after this exits", dir.display(), parent.display()));
    } else {
        r.act(format!("remove {}", me.display()), || rm(&me));
    }
    // dirs the installer created (recorded in its manifest), deepest first, only when empty now
    for d in created {
        let empty = std::fs::read_dir(&d).map(|mut rd| rd.next().is_none()).unwrap_or(false);
        if empty {
            r.act(format!("remove empty {} (created by the installer)", d.display()), || std::fs::remove_dir(&d).is_ok());
        }
    }
    let running = running_gates();
    let mut notes = vec![];
    if !running.is_empty() {
        notes.push(format!("gui-gate is still running as pid {} (an MCP client has it open); it ends when that client closes it", running.join(", ")));
    }
    if !with_driver {
        if let Some(ext) = crate::install::official_driver_paths().into_iter().find(|p| Path::new(p).exists()) {
            let ours = manifest.iter().any(|(k, v)| k == "app" && ext.starts_with(v.as_str()));
            if !ours && !dry {
                notes.push(format!("a Cua Driver installed with Cua's own installer is still at {ext}; it was left alone (to remove it too, use `gui-gate uninstall --with-driver`, or Cua's own uninstaller: {})", if cfg!(windows) { "irm https://cua.ai/driver/uninstall.ps1 | iex" } else { "bash -c \"$(curl -fsSL https://cua.ai/driver/uninstall.sh)\" -- --purge" }));
            }
        }
    }
    println!("{}", serde_json::to_string_pretty(&json!({"removed": r.done, "not_removed": r.left, "notes": notes})).unwrap());
    if r.left.is_empty() { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_lines() {
        let keep = "/home/u/.local/bin";
        let t = format!("a\n\n# gui-gate\nexport PATH=\"{keep}:$PATH\"\nb\n\n# gui-gate\nexport PATH=\"{keep}:$PATH\"\n\n# gui-gate\nexport PATH=\"/gone/dir:$PATH\"\n");
        let (new, n) = clean_profile_text(&t, keep);
        assert_eq!(n, 2);
        assert_eq!(new.matches("# gui-gate").count(), 1);
        assert!(new.starts_with("a\n\n# gui-gate") && !new.contains("/gone/dir"));
        assert_eq!(clean_profile_text(&new, keep).1, 0);
    }

    #[test]
    fn windows_path_entries() {
        let keep = r"C:\Users\u\AppData\Local\Programs\gui-gate\bin";
        let e: Vec<String> = vec![r"C:\Windows".into(), keep.into(), format!("{keep}\\"), r"C:\old\gui-gate\bin".into(), r"C:\Windows".into()];
        let (out, n) = clean_path_list(&e, keep);
        assert_eq!(n, 2); // the duplicate of ours and the stale one; other people's duplicates are not ours to touch
        assert_eq!(out, vec![r"C:\Windows".to_string(), keep.into(), r"C:\Windows".into()]);
    }

    #[test]
    fn mcp_entries() {
        let mut e = json!({"command": "/data/venv/bin/gui-gate", "args": ["--policy", "gate", "--decider", "model", "--risk-mode", "model"]});
        assert!(fix_entry(&mut e, "/home/u/.local/bin/gui-gate", &["/data/venv/bin/gui-gate".into()]));
        assert_eq!(e["command"], "/home/u/.local/bin/gui-gate");
        assert_eq!(e["args"], json!(["--policy", "gate", "--risk-mode", "reviewer"]));
        let mut p = json!({"command": "python3", "args": ["-m", "gui_gate", "--policy", "direct"]});
        assert!(fix_entry(&mut p, "/x/gui-gate", &[]));
        assert_eq!(p, json!({"command": "/x/gui-gate", "args": ["--policy", "direct"]}));
        let mut other = json!({"command": "npx", "args": ["-y", "something"]});
        assert!(!fix_entry(&mut other, "/x/gui-gate", &[]));
        let mut fine = json!({"command": "gui-gate", "args": ["--policy", "gate"]});
        assert!(!fix_entry(&mut fine, "/x/gui-gate", &[]));
    }

    #[test]
    fn toml_block() {
        let t = "model = \"o3\"\n\n[mcp_servers.gui]\ncommand = \"/gone/gui-gate\"\nargs = [\"--decider\",\"model\"]\n\n[mcp_servers.other]\ncommand = \"npx\"\n";
        let (new, n) = fix_toml(t, "/x/gui-gate", &[]);
        assert_eq!(n, 2);
        assert!(new.contains("command = \"/x/gui-gate\"") && new.contains("args = []") && new.contains("command = \"npx\""));
        let stripped = toml_without_gate(t);
        assert!(!stripped.contains("gui-gate") && stripped.contains("[mcp_servers.other]") && stripped.contains("model = \"o3\""));
    }

    #[test]
    fn backup_equal_but_for_gate() {
        let a = json!({"theme": "x", "mcpServers": {"gui": {"command": "/a/gui-gate", "args": []}, "o": {"command": "npx"}}});
        let b = json!({"theme": "x", "mcpServers": {"o": {"command": "npx"}}});
        assert_eq!(without_gate(&a), without_gate(&b));
        let c = json!({"theme": "y", "mcpServers": {"o": {"command": "npx"}}});
        assert_ne!(without_gate(&a), without_gate(&c));
    }
}
