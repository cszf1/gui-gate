//! Cua Driver stays Cua's own program; gui-gate only finds it, notices new releases and updates on request.
//!  * the release installer (scripts/install.sh / install.ps1) runs Cua's official installer, so a fresh
//!    install always gets the latest Cua Driver;
//!  * TESTED is the driver version this release was tested with; a newer one works but is flagged untested;
//!  * the check reads the version Cua's official installer would install (one small HTTPS GET to cua.ai,
//!    PyPI as fallback), at most once a day (cached), in the background, 3 s timeout;
//!    GUI_GATE_NO_UPDATE_CHECK=1 turns it off;
//!  * nothing is installed automatically: `gui-gate update` installs on request, checks it, rolls back on failure;
//!    `gui-gate update --self` re-runs gui-gate's own installer (gate + latest driver).
use crate::util::*;
use serde_json::{json, Value};
use std::time::Duration;

pub const TESTED: &str = "0.37.0";
const CUA_INSTALLER_SH: &str = "https://cua.ai/driver/install.sh";
const CUA_INSTALLER_RS: &str = "https://cua.ai/driver/_install-rust.sh";
const CUA_INSTALLER_PS1: &str = "https://cua.ai/driver/install.ps1";
const PYPI_URL: &str = "https://pypi.org/pypi/cua-driver/json";
pub const GATE_INSTALL_SH: &str = "https://github.com/cszf1/gui-gate/releases/latest/download/install.sh";
pub const GATE_INSTALL_PS1: &str = "https://github.com/cszf1/gui-gate/releases/latest/download/install.ps1";
pub const REQUIRED_DRIVER_TOOLS: &[&str] = &["get_window_state", "list_windows", "click", "type_text", "press_key"];
const CACHE_TTL: f64 = 24.0 * 3600.0;

pub fn vtuple(v: &str) -> Vec<u64> {
    let mut out = vec![];
    for part in v.trim().trim_start_matches('v').split('.') {
        let num: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
        out.push(num.parse().unwrap_or(0));
        if !part.chars().all(|c| c.is_ascii_digit()) {
            break;
        }
    }
    if out.is_empty() { vec![0] } else { out }
}

pub fn newer(a: &str, b: &str) -> bool {
    vtuple(a) > vtuple(b)
}

/// The version pinned in Cua's official installer (what a fresh install gets), from its baked-version line.
pub fn baked_version(script: &str) -> Option<String> {
    let re = regex::Regex::new(r#"CUA_DRIVER_RS_BAKED_VERSION="v?([0-9]+\.[0-9]+\.[0-9]+)""#).ok()?;
    re.captures(script).map(|c| c[1].to_string())
}

fn fetch_latest() -> Option<String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(3)).build();
    if let Some(v) = agent.get(CUA_INSTALLER_RS).set("User-Agent", "gui-gate-update-check").call().ok()
        .and_then(|r| r.into_string().ok()).and_then(|t| baked_version(&t)) {
        return Some(v);
    }
    let d: Value = agent.get(PYPI_URL).set("User-Agent", "gui-gate-update-check").call().ok()?.into_json().ok()?;
    d.get("info")?.get("version")?.as_str().map(String::from)
}

pub fn installed() -> Option<String> {
    let d = crate::install::find_driver()?;
    crate::install::driver_version(&d)
}

fn cache_path() -> String {
    let base = env("XDG_CACHE_HOME").unwrap_or_else(|| format!("{}/.cache", home()));
    format!("{base}/gui-gate/driver-latest.json")
}

pub fn latest(use_cache: bool) -> Option<String> {
    if let Some(v) = env("GUI_GATE_LATEST_OVERRIDE") {
        return Some(v); // tests / offline mirrors
    }
    if env("GUI_GATE_NO_UPDATE_CHECK").is_some() {
        return None;
    }
    let p = cache_path();
    if use_cache {
        if let Ok(c) = std::fs::read_to_string(&p).map(|t| serde_json::from_str::<Value>(&t).unwrap_or(json!({}))) {
            if now() - c["t"].as_f64().unwrap_or(0.0) < CACHE_TTL {
                if let Some(v) = c["latest"].as_str() {
                    return Some(v.to_string());
                }
            }
        }
    }
    let v = fetch_latest()?;
    if let Some(dir) = std::path::Path::new(&p).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&p, json!({"t": now(), "latest": v}).to_string());
    Some(v)
}

/// inst: the version the running driver reported, which beats looking it up.
pub fn status(use_cache: bool, inst: Option<String>) -> Value {
    let inst = inst.filter(|v| !v.is_empty()).or_else(installed);
    let lat = latest(use_cache);
    let upd = matches!((&inst, &lat), (Some(i), Some(l)) if newer(l, i));
    let untested = inst.as_ref().map(|i| newer(i, TESTED)).unwrap_or(false);
    json!({"installed": inst, "tested_with": TESTED, "latest": lat, "update_available": upd, "untested": untested})
}

/// One line for the user (via the agent / client), or "" when there is nothing to say.
pub fn note(st: &Value) -> String {
    if truthy(st.get("update_available")) {
        return format!("Cua Driver {} is available (installed {}). Tell the user they can run `gui-gate update`.",
                       pystr(st.get("latest")), pystr(st.get("installed")));
    }
    if truthy(st.get("untested")) {
        return format!("Cua Driver {} is newer than the version this gui-gate was tested with ({}); if GUI actions misbehave, `gui-gate update --to {}`.",
                       pystr(st.get("installed")), pystr(st.get("tested_with")), pystr(st.get("tested_with")));
    }
    String::new()
}

pub fn missing_tools(names: &[String]) -> Vec<String> {
    let mut m: Vec<String> = REQUIRED_DRIVER_TOOLS.iter().filter(|t| !names.iter().any(|n| n == *t)).map(|t| t.to_string()).collect();
    m.sort();
    m
}

fn pip_install(spec: &str) -> bool {
    let Some(py) = crate::install::env_python_for_pip() else { return false };
    std::process::Command::new(py).args(["-m", "pip", "install", "-q", spec]).status().map(|s| s.success()).unwrap_or(false)
}

/// Run a remote installer script the way its README says (bash -c "$(curl ...)" / irm | iex), in a child shell.
fn run_installer(sh_url: &str, ps1_url: &str, envs: &[(&str, &str)]) -> bool {
    let mut c = if cfg!(windows) {
        let mut c = std::process::Command::new("powershell");
        c.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &format!("irm {ps1_url} | iex")]);
        c
    } else {
        let mut c = std::process::Command::new("bash");
        c.args(["-c", &format!("set -e; s=\"$(curl -fsSL {sh_url})\"; bash -c \"$s\" installer")]);
        c
    };
    for (k, v) in envs {
        c.env(k, v);
    }
    c.status().map(|s| s.success()).unwrap_or(false)
}

/// Install one Cua Driver version the same way the running one was installed.
fn install_driver(version: &str) -> bool {
    let from_pip = crate::install::find_driver().map(|d| crate::install::driver_is_pip(&d)).unwrap_or(false);
    if from_pip {
        return pip_install(&format!("cua-driver=={version}"));
    }
    run_installer(CUA_INSTALLER_SH, CUA_INSTALLER_PS1, &[("CUA_DRIVER_RS_VERSION", version)])
}

pub fn update_main(argv: &[String]) -> i32 {
    let mut check = false;
    let mut yes = false;
    let mut to: Option<String> = None;
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--check" => check = true,
            "-y" | "--yes" => yes = true,
            "--to" => { to = argv.get(i + 1).cloned(); i += 1; }
            "--self" => {
                println!("Re-running gui-gate's installer (latest gui-gate + latest Cua Driver)...");
                return if run_installer(GATE_INSTALL_SH, GATE_INSTALL_PS1, &[]) { 0 } else { 1 };
            }
            "-h" | "--help" => { println!("usage: gui-gate update [--check] [--to VERSION] [-y] | --self\ncheck for / install a newer Cua Driver; --self updates gui-gate and the driver together"); return 0; }
            other => { eprintln!("gui-gate update: unknown argument {other}"); return 2; }
        }
        i += 1;
    }
    let st = status(false, None);
    println!("{}", serde_json::to_string_pretty(&st).unwrap());
    let target = to.clone().or_else(|| st["latest"].as_str().map(String::from));
    let Some(target) = target else {
        println!("Could not reach cua.ai or PyPI to look for a newer Cua Driver.");
        return 0;
    };
    if check || (to.is_none() && !truthy(st.get("update_available"))) {
        let n = note(&st);
        println!("{}", if n.is_empty() { "Cua Driver is up to date.".to_string() } else { n });
        return 0;
    }
    if vtuple(&target)[0] >= 1 {
        println!("Cua Driver {target} is a new major version; wait for a gui-gate release that supports it.");
        return 1;
    }
    let prev = st["installed"].as_str().map(String::from);
    if !yes {
        print!("Install Cua Driver {target} (now {})? [y/N] ", prev.clone().unwrap_or_else(|| "none".into()));
        let _ = std::io::Write::flush(&mut std::io::stdout());
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        if !matches!(line.trim().to_lowercase().as_str(), "y" | "yes") {
            return 0;
        }
    }
    if !install_driver(&target) {
        println!("Could not install Cua Driver {target} from here. Re-run the installer: curl -fsSL {GATE_INSTALL_SH} | bash  (Windows: irm {GATE_INSTALL_PS1} | iex)");
        return 1;
    }
    let rep = crate::install::doctor(None);
    let miss = rep.get("missing_tools").and_then(|m| m.as_array()).map(|a| a.len()).unwrap_or(0);
    if truthy(rep.get("ok")) && miss == 0 {
        let untested = if newer(&target, TESTED) { format!(" Not yet tested with this gui-gate (tested {TESTED}).") } else { String::new() };
        println!("Cua Driver {target} installed and checked ({} tools).{untested}", rep["driver_tools"]);
        return 0;
    }
    println!("Cua Driver {target} failed the check ({}); going back to {}.",
             rep.get("driver_error").map(|e| pystr(Some(e))).unwrap_or_else(|| format!("missing tools: {}", rep["missing_tools"])),
             prev.clone().unwrap_or_else(|| "the previous version".into()));
    if let Some(p) = prev {
        install_driver(&p);
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_order() {
        assert!(newer("0.37.10", "0.37.9"));
        assert!(newer("0.38.0", "0.37.0"));
        assert_eq!(vtuple("v0.37.0"), vec![0, 37, 0]);
        assert_eq!(&vtuple("0.38.0rc1")[..2], &[0, 38]);
    }
    #[test]
    fn notes() {
        let st = json!({"installed": "0.37.0", "latest": "0.38.1", "tested_with": "0.37.0", "update_available": true, "untested": false});
        assert!(note(&st).contains("gui-gate update"));
        let st2 = json!({"installed": "0.37.0", "latest": "0.37.0", "tested_with": "0.37.0", "update_available": false, "untested": false});
        assert_eq!(note(&st2), "");
        let st3 = json!({"installed": "0.39.0", "latest": "0.37.0", "tested_with": "0.37.0", "update_available": false, "untested": true});
        assert!(note(&st3).contains("--to 0.37.0"));
    }
    #[test]
    fn baked() {
        assert_eq!(baked_version("x\nCUA_DRIVER_RS_BAKED_VERSION=\"0.38.0\" # published-installer-version\n").as_deref(), Some("0.38.0"));
        assert_eq!(baked_version("nothing"), None);
    }
    #[test]
    fn missing() {
        assert_eq!(missing_tools(&["click".into(), "type_text".into()]), vec!["get_window_state", "list_windows", "press_key"]);
    }
}
