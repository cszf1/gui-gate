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
    std::env::current_exe().ok().and_then(|p| p.canonicalize().ok().or(Some(p))).and_then(|p| p.parent().map(|d| d.to_path_buf()))
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

/// The binary inside the cua-driver package installed next to gui-gate.
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

/// GUI_GATE_DRIVER, then a cua-driver next to this binary or on PATH (the official installer),
/// then the official install locations, then the Python package's bundled binary.
pub fn find_driver() -> Option<String> {
    if let Some(e) = env("GUI_GATE_DRIVER") {
        return Some(e);
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
/// Real end-to-end check: start the driver's MCP server, initialize, list tools.
pub fn doctor(cmd: Option<Vec<String>>) -> Value {
    let mut rep = json!({"platform": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH), "gui_gate": crate::server::VERSION,
                         "driver_source": if find_driver().map(|d| driver_is_pip(&d)).unwrap_or(false) { "pip" } else { "official" }});
    let d = if cmd.is_some() { None } else { find_driver() };
    rep["driver"] = json!(d.clone().or_else(|| cmd.as_ref().map(|c| c.join(" "))));
    rep["driver_version"] = json!(d.as_ref().and_then(|p| driver_version(p)));
    rep["driver_tools"] = Value::Null;
    let full = cmd.clone().or_else(|| d.as_ref().map(|p| vec![p.clone(), "mcp".into()]));
    if let Some(full) = full {
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
    let ok = as_i64(rep.get("driver_tools")).unwrap_or(0) > 0 && rep.get("missing_tools").and_then(|m| m.as_array()).map(|a| a.is_empty()).unwrap_or(true);
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
