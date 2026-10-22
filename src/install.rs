//! One install: the release installer (scripts/install.sh, scripts/install.ps1) drops this binary next to
//! Cua Driver installed by Cua's own official installer; a pip/pipx install (Cua Driver's Python package)
//! is still found too. This module finds the driver, wires the server into MCP clients
//! (`gui-gate setup`) and checks the machine end to end (`gui-gate doctor`).
use crate::rpc::ChildMCP;
use crate::updates;
use crate::util::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

pub const CLIENTS: &[&str] = &["claude", "codex", "cursor", "pi", "print"];

fn exe_name(base: &str) -> String {
    if cfg!(windows) { format!("{base}.exe") } else { base.to_string() }
}

pub fn which(bin: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for d in std::env::split_paths(&path) {
        for cand in [d.join(bin), d.join(exe_name(bin))] {
            if cand.is_file() {
                return Some(cand.to_string_lossy().to_string());
            }
        }
    }
    None
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.canonicalize().ok().or(Some(p))).and_then(|p| p.parent().map(|d| {
        // Windows canonical paths carry the \\?\ prefix; keep paths the way users and cmd.exe write them
        let t = d.to_string_lossy().to_string();
        match t.strip_prefix(r"\\?\") {
            Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
            _ => d.to_path_buf(),
        }
    }))
}

/// The Python environment this binary was installed into (venv/bin or venv\Scripts holds both).
fn env_python() -> Option<String> {
    let d = exe_dir()?;
    for n in ["python", "python3", "python.exe"] {
        let p = d.join(n);
        if p.is_file() {
            return Some(p.to_string_lossy().to_string());
        }
    }
    None
}

// ------------------------------------------------------------------ the bundled Cua Driver (0.9.7)
/// Cua Driver version shipped inside every gui-gate release archive (changes only with a gui-gate release).
pub const BUNDLED_DRIVER_VERSION: &str = "0.38.0";

/// gui-gate's own install root for everything besides the gate binary:
/// unix `<bin>/../lib/gui-gate` (default ~/.local/lib/gui-gate), Windows `<bin>\..` (%LOCALAPPDATA%\Programs\gui-gate).
pub fn lib_dir() -> Option<PathBuf> {
    if let Some(d) = env("GUI_GATE_LIB_DIR") {
        return Some(PathBuf::from(d));
    }
    let d = exe_dir()?;
    let parent = d.parent()?.to_path_buf();
    Some(if cfg!(windows) { parent } else { parent.join("lib").join("gui-gate") })
}

/// key=value lines the installer keeps in `<lib_dir>/install-manifest` (what it created, so uninstall removes
/// exactly that): `created_dir=PATH` (repeatable), `app=PATH` (macOS CuaDriver.app installed by gui-gate).
pub fn manifest() -> Vec<(String, String)> {
    let Some(d) = lib_dir() else { return vec![] };
    std::fs::read_to_string(d.join("install-manifest")).unwrap_or_default().lines()
        .filter_map(|l| l.split_once('=').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
        .filter(|(k, v)| !k.is_empty() && !v.is_empty() && !k.starts_with('#')).collect()
}

/// The Cua Driver that came in gui-gate's own archive, when the installer put it in place.
pub fn gate_bundled_driver() -> Option<String> {
    if cfg!(target_os = "macos") {
        // the signed CuaDriver.app the installer placed (TCC permissions belong to the app at its location)
        let app = manifest().into_iter().find(|(k, _)| k == "app").map(|x| x.1)?;
        let p = format!("{app}/Contents/MacOS/cua-driver");
        return Path::new(&p).is_file().then_some(p);
    }
    let p = lib_dir()?.join("cua-driver").join(exe_name("cua-driver"));
    p.is_file().then(|| p.to_string_lossy().to_string())
}

/// True when `path` is gui-gate's bundled driver (then the gate starts it with Cua's telemetry and update check off).
pub fn is_gate_bundled(path: &str) -> bool {
    gate_bundled_driver().map(|b| b == path).unwrap_or(false)
}

/// Environment for starting the bundled driver: Cua's content-free telemetry and its update check are off, so the
/// driver writes nothing to ~/.cua-driver and makes no network calls of its own. GUI_GATE_DRIVER_TELEMETRY=1 keeps
/// Cua's defaults.
pub fn bundled_driver_env() -> Vec<(&'static str, &'static str)> {
    if env("GUI_GATE_DRIVER_TELEMETRY").map(|v| v != "0").unwrap_or(false) {
        return vec![("CUA_DRIVER_INSTALL_CHANNEL", "manual_binary")];
    }
    vec![("CUA_DRIVER_RS_TELEMETRY_ENABLED", "0"), ("CUA_TELEMETRY_ENABLED", "0"), ("DO_NOT_TRACK", "1"),
         ("CUA_DRIVER_RS_UPDATE_CHECK", "0"), ("CUA_DRIVER_INSTALL_CHANNEL", "manual_binary")]
}

/// The binary inside the cua-driver Python package installed next to gui-gate (0.2-0.5 pip installs).
pub fn bundled_driver() -> Option<String> {
    let d = exe_dir()?;
    let bin = exe_name("cua-driver");
    let pats = [
        format!("{}/../lib/python*/site-packages/cua_driver/bin/{bin}", d.display()),
        format!("{}/../lib/python*/dist-packages/cua_driver/bin/{bin}", d.display()),
        format!("{}/../lib/python/site-packages/cua_driver/bin/{bin}", d.display()),
        format!("{}/../Lib/site-packages/cua_driver/bin/{bin}", d.display()),
        format!("{}/Lib/site-packages/cua_driver/bin/{bin}", d.display()),
    ];
    for p in pats {
        if let Ok(g) = glob::glob(&p) {
            if let Some(hit) = g.flatten().next() {
                return Some(crate::sidefx::normpath(&hit.to_string_lossy()));
            }
        }
    }
    // ask the environment's own Python where the package put it
    let py = env_python()?;
    let out = Command::new(py).args(["-c", "from cua_driver.wrapper import get_binary_path as g; print(g())"]).output().ok()?;
    let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !p.is_empty() && Path::new(&p).is_file() { Some(p) } else { None }
}

/// Where Cua Driver's official installers put it (install.sh / install.ps1), newest layout first.
pub fn official_driver_paths() -> Vec<String> {
    let h = home();
    let mut v = vec![];
    if cfg!(windows) {
        if let Some(la) = env("LOCALAPPDATA") {
            v.push(format!("{la}\\Programs\\Cua\\cua-driver\\bin\\cua-driver.exe"));
        }
        let up = env("USERPROFILE").unwrap_or(h.clone());
        v.push(format!("{up}\\.cua-driver\\packages\\current\\cua-driver.exe"));
    } else {
        if cfg!(target_os = "macos") {
            v.push("/Applications/CuaDriver.app/Contents/MacOS/cua-driver".into());
            v.push(format!("{h}/Applications/CuaDriver.app/Contents/MacOS/cua-driver"));
        }
        v.push(format!("{h}/.local/bin/cua-driver"));
        v.push(format!("{h}/.cua-driver/packages/current/cua-driver"));
    }
    v
}

/// GUI_GATE_DRIVER=PATH, else the Cua Driver bundled with gui-gate, else (or with GUI_GATE_DRIVER=external) an
/// externally installed one: the Python package's binary, a cua-driver next to this binary or on PATH, then Cua's
/// official install locations.
pub fn find_driver() -> Option<String> {
    match env("GUI_GATE_DRIVER") {
        Some(e) if e.eq_ignore_ascii_case("external") => {}
        Some(e) => return Some(e),
        None => {
            if let Some(b) = gate_bundled_driver() {
                return Some(b);
            }
        }
    }
    if let Some(b) = bundled_driver() {
        return Some(b); // pip/pipx install: the binary inside the Python package, not its Python launcher
    }
    if let Some(d) = exe_dir() {
        let c = d.join(exe_name("cua-driver"));
        if c.is_file() {
            return Some(c.to_string_lossy().to_string());
        }
    }
    if let Some(p) = which("cua-driver") {
        return Some(p);
    }
    official_driver_paths().into_iter().find(|c| Path::new(c).exists())
}

/// True when the driver in use came from Cua Driver's Python package (pip/pipx), not the official installer.
pub fn driver_is_pip(path: &str) -> bool {
    path.contains("site-packages") || path.contains("dist-packages")
}

/// Version from the package metadata next to the bundled binary, else `cua-driver --version`.
pub fn driver_version(path: &str) -> Option<String> {
    if is_gate_bundled(path) {
        let f = lib_dir().map(|d| d.join("cua-driver").join("VERSION"));
        return Some(f.and_then(|f| std::fs::read_to_string(f).ok()).map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
            .unwrap_or_else(|| BUNDLED_DRIVER_VERSION.to_string()));
    }
    let p = Path::new(path);
    if let Some(site) = p.parent().and_then(|b| b.parent()).and_then(|pkg| pkg.parent()) {
        if let Ok(rd) = std::fs::read_dir(site) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if let Some(v) = n.strip_prefix("cua_driver-").and_then(|r| r.strip_suffix(".dist-info")) {
                    return Some(v.to_string());
                }
            }
        }
    }
    let out = Command::new(path).arg("--version").output().ok()?;
    String::from_utf8_lossy(&out.stdout).split_whitespace().last().map(String::from)
}

// ------------------------------------------------------------------ MCP client configuration
pub fn server_command() -> Vec<String> {
    vec![std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| "gui-gate".into())]
}

pub fn snippet(name: &str) -> String {
    let cmd = server_command();
    serde_json::to_string_pretty(&json!({"mcpServers": {name: {"command": cmd[0], "args": cmd[1..]}}})).unwrap()
}

fn merge_json(path: &str, name: &str, entry: Value) -> String {
    let mut data = json!({});
    if Path::new(path).exists() {
        let txt = std::fs::read_to_string(path).unwrap_or_default();
        match serde_json::from_str::<Value>(&txt) {
            Ok(v) if v.is_object() => data = v,
            _ => return format!("{path} is not valid JSON; add this yourself:\n{}", snippet(name)),
        }
    }
    if !data.get("mcpServers").map(|v| v.is_object()).unwrap_or(false) {
        data["mcpServers"] = json!({});
    }
    data["mcpServers"][name] = entry;
    if let Some(d) = Path::new(path).parent() {
        let _ = std::fs::create_dir_all(d);
    }
    // atomic replace, no backup copy left behind (an unreadable file is never touched, see above)
    let tmp = format!("{path}.gui-gate-tmp");
    match std::fs::write(&tmp, serde_json::to_string_pretty(&data).unwrap()).and_then(|_| std::fs::rename(&tmp, path)) {
        Ok(_) => format!("wrote {path}"),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            format!("could not write {path}: {e}")
        }
    }
}

pub fn configure(client: &str, name: &str, home_dir: Option<&str>) -> String {
    let h = home_dir.map(String::from).unwrap_or_else(home);
    let cmd = server_command();
    match client {
        "print" => snippet(name),
        "cursor" => merge_json(&format!("{h}/.cursor/mcp.json"), name, json!({"command": cmd[0], "args": cmd[1..]})),
        "codex" => {
            let path = format!("{h}/.codex/config.toml");
            let _ = std::fs::create_dir_all(format!("{h}/.codex"));
            let old = std::fs::read_to_string(&path).unwrap_or_default();
            if old.contains(&format!("[mcp_servers.{name}]")) {
                return format!("{path} already has [mcp_servers.{name}]");
            }
            let block = format!("\n[mcp_servers.{name}]\ncommand = {}\nargs = {}\nstartup_timeout_sec = 120\n",
                                json!(cmd[0]), json!(cmd[1..]));
            match std::fs::write(&path, old + &block) {
                Ok(_) => format!("wrote {path}"),
                Err(e) => format!("could not write {path}: {e}"),
            }
        }
        "claude" | "pi" => {
            let Some(cli) = which(client) else {
                return format!("`{client}` is not on PATH; add the server yourself:\n{}", snippet(name));
            };
            let mut args = vec!["mcp".to_string(), "add".into(), name.into(), "--".into()];
            args.extend(cmd);
            match Command::new(cli).args(&args).output() {
                Ok(o) => {
                    let t = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string();
                    if t.is_empty() { format!("{client} mcp add {name}: exit {}", o.status.code().unwrap_or(-1)) } else { t }
                }
                Err(e) => format!("{client} mcp add failed: {e}"),
            }
        }
        _ => format!("unknown client {client}; choose one of {}", CLIENTS.join(", ")),
    }
}

// ------------------------------------------------------------------ checks
/// macOS: stops the CuaDriver.app daemon a doctor run started (on drop), unless it was running before.
struct StopDaemon(bool, String);

impl Drop for StopDaemon {
    fn drop(&mut self) {
        crate::proc::stop_mac_daemon_unless(self.0, &self.1);
    }
}
/// Real end-to-end check: start the driver's MCP server, initialize, list tools.
pub fn doctor(cmd: Option<Vec<String>>) -> Value {
    let src = match find_driver() {
        Some(d) if is_gate_bundled(&d) => "bundled",
        Some(d) if driver_is_pip(&d) => "pip",
        Some(_) => "external",
        None => "none",
    };
    let mut rep = json!({"platform": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH), "gui_gate": crate::server::VERSION,
                         "driver_source": src});
    let d = if cmd.is_some() { None } else { find_driver() };
    rep["driver"] = json!(d.clone().or_else(|| cmd.as_ref().map(|c| c.join(" "))));
    rep["driver_version"] = json!(d.as_ref().and_then(|p| driver_version(p)));
    rep["driver_tools"] = Value::Null;
    let full = cmd.clone().or_else(|| d.as_ref().map(|p| vec![p.clone(), "mcp".into()]));
    if let Some(full) = full {
        let _stop = StopDaemon(crate::proc::mac_daemon_running(), full[0].clone());
        match ChildMCP::spawn(&full, Arc::new(|_m| {}), false) {
            Ok(child) => {
                let init = child.call("initialize", Some(json!({"protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": {"name": "gui-gate-doctor", "version": "0"}})), 30.0);
                if let Some(e) = init.get("error") {
                    rep["driver_error"] = json!(pystr(e.get("message")));
                } else {
                    let _ = child.forward(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
                    let tl = child.call("tools/list", Some(json!({})), 30.0);
                    let names: Vec<String> = tl.get("result").and_then(|r| r.get("tools")).and_then(|t| t.as_array())
                        .map(|a| a.iter().map(|t| s(t, "name").to_string()).collect()).unwrap_or_default();
                    rep["driver_tools"] = json!(names.len());
                    rep["missing_tools"] = json!(updates::missing_tools(&names));
                    rep["driver_server"] = init.get("result").and_then(|r| r.get("serverInfo")).and_then(|s| s.get("name")).cloned().unwrap_or(Value::Null);
                }
                child.close();
            }
            Err(e) => rep["driver_error"] = json!(e.to_string()),
        }
    }
    if cfg!(target_os = "linux") {
        rep["display"] = json!(env("DISPLAY").is_some() || env("WAYLAND_DISPLAY").is_some());
        rep["at_spi"] = json!(which("at-spi-bus-launcher").is_some() || Path::new("/usr/libexec/at-spi-bus-launcher").exists()
            || Path::new("/usr/lib/at-spi2-core/at-spi-bus-launcher").exists());
    }
    let mut ok = as_i64(rep.get("driver_tools")).unwrap_or(0) > 0 && rep.get("missing_tools").and_then(|m| m.as_array()).map(|a| a.is_empty()).unwrap_or(true);
    // the gate refuses to start without a reviewer model (the --child test hook checks only the driver)
    let rv = crate::reviewer::Reviewer::from_env();
    rep["reviewer"] = json!(rv.backend());
    if let Some(why) = crate::reviewer::config_problem() {
        rep["reviewer_error"] = json!(format!("{why}; the gate will not start until a reviewer is set (see `gui-gate check-reviewer --help`)"));
        ok = ok && cmd.is_some();
    }
    rep["ok"] = json!(ok);
    if cmd.is_none() {
        let st = updates::status(true, rep.get("driver_version").and_then(|v| v.as_str()).map(String::from));
        rep["driver_latest"] = st["latest"].clone();
        rep["driver_tested_with"] = st["tested_with"].clone();
        let n = updates::note(&st);
        if !n.is_empty() {
            rep["driver_note"] = json!(n);
        }
    }
    rep
}

pub fn setup_main(argv: &[String]) -> i32 {
    let mut clients: Vec<String> = vec![];
    let mut name = "gui".to_string();
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--client" => { if let Some(c) = argv.get(i + 1) { clients.push(c.clone()); } i += 1; }
            "--name" => { if let Some(n) = argv.get(i + 1) { name = n.clone(); } i += 1; }
            "-h" | "--help" => {
                println!("usage: gui-gate setup [--client {}]... [--name NAME]\nadd gui-gate to your MCP client(s) and check it works", CLIENTS.join("|"));
                return 0;
            }
            other => { eprintln!("gui-gate setup: unknown argument {other}"); return 2; }
        }
        i += 1;
    }
    if clients.is_empty() {
        clients.push("print".into());
    }
    for c in &clients {
        println!("{}", configure(c, &name, None));
    }
    let rep = doctor(None);
    println!("{}", serde_json::to_string_pretty(&rep).unwrap());
    if !truthy(rep.get("ok")) {
        log_err("not ready: see the report above");
        return 1;
    }
    log_err(&format!("ready: {} driver tools behind the gate, single-step and batch (run_actions)", rep["driver_tools"]));
    0
}

pub fn doctor_main(argv: &[String]) -> i32 {
    // test hook: `gui-gate doctor --child "<cmd>"` checks another MCP server
    let cmd = argv.iter().position(|a| a == "--child").and_then(|i| argv.get(i + 1)).map(|c| c.split_whitespace().map(String::from).collect());
    let rep = doctor(cmd);
    println!("{}", serde_json::to_string_pretty(&rep).unwrap());
    if truthy(rep.get("ok")) { 0 } else { 1 }
}

pub fn env_python_for_pip() -> Option<String> {
    env_python()
}
