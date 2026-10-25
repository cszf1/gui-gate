//! Child processes the gate starts for the agent (run_command, run_command mode=start jobs, read-only probes) and how
//! they are kept from outliving the gate (0.9.7 install hygiene):
//!  * every shell command runs in its own process group (unix) and its group id is registered here while it runs;
//!  * when the gate ends - stdin EOF (the MCP client closed it), SIGTERM / SIGINT / SIGHUP, or a normal return -
//!    every registered group is killed (`kill_all`), so nothing the agent started keeps running;
//!  * a program a finished command left running on purpose (`soffice file.ods &` to open an app for the task) is no
//!    longer registered and is NOT killed: it is what the agent was asked to open;
//!  * a gate killed with SIGKILL cannot clean up; its still-running jobs then end on their own.
//!  * Windows: run_command is not offered (its risk rules are written for bash), so no shell children exist there.
use std::process::{Child, Command};
use std::sync::atomic::{AtomicI32, Ordering};

/// Process group ids (unix) / pids (Windows) of running agent commands; 0 = free slot.
static LIVE: [AtomicI32; 128] = [const { AtomicI32::new(0) }; 128];
/// The driver child (cua-driver mcp), terminated together with the gate on a signal.
static DRIVER: AtomicI32 = AtomicI32::new(0);

pub fn register(id: u32) {
    let id = id as i32;
    for s in LIVE.iter() {
        if s.compare_exchange(0, id, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            return;
        }
    }
}

pub fn unregister(id: u32) {
    let id = id as i32;
    for s in LIVE.iter() {
        let _ = s.compare_exchange(id, 0, Ordering::SeqCst, Ordering::SeqCst);
    }
}

/// macOS: whether the CuaDriver.app daemon (`cua-driver serve`, which `cua-driver mcp` starts through LaunchServices)
/// was already running before this gate started; if not, the gate stops it again when it ends.
static MAC_DAEMON_BEFORE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn mac_daemon_running() -> bool {
    cfg!(target_os = "macos")
        && Command::new("pgrep").args(["-f", "CuaDriver.app/Contents/MacOS/cua-driver serve"]).output()
            .map(|o| !o.stdout.is_empty()).unwrap_or(false)
}

pub fn note_mac_daemon() {
    if cfg!(target_os = "macos") {
        MAC_DAEMON_BEFORE.store(mac_daemon_running(), Ordering::SeqCst);
    }
}

/// Stop the CuaDriver.app daemon if `before` says this process started it (macOS; not while another gui-gate runs).
pub fn stop_mac_daemon_unless(before: bool, driver: &str) {
    if !cfg!(target_os = "macos") || before || !driver.contains("CuaDriver.app/") {
        return;
    }
    if !crate::cleanup::running_gates().is_empty() || !mac_daemon_running() {
        return;
    }
    let _ = driver_stop(driver);
}

/// `cua-driver stop` with Cua's telemetry and update check off: `stop` is a "finite command" Cua wraps in a child
/// and follows with a detached telemetry worker (the same binary, no arguments) unless telemetry is off - that
/// worker was the CuaDriver.app process left running after 0.9.8's uninstall, and it re-created ~/.cua-driver.
pub fn driver_stop(driver: &str) -> Option<i32> {
    run_argv_env(&[driver, "stop"], &crate::install::bundled_driver_env(), 10.0).0
}

/// (pid, command line) of every process whose program lives inside `app` (`app/Contents/...`; unix, via ps).
pub fn app_processes(app: &str) -> Vec<(String, String)> {
    if cfg!(windows) || app.is_empty() {
        return vec![];
    }
    let me = std::process::id().to_string();
    let prefix = format!("{}/Contents/", app.trim_end_matches('/'));
    let out = Command::new("ps").args(["-A", "-o", "pid=,command="]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    out.lines().filter_map(|l| {
        let l = l.trim();
        let (pid, cmd) = l.split_once(char::is_whitespace)?;
        let cmd = cmd.trim();
        (pid != me && cmd.starts_with(&prefix)).then(|| (pid.to_string(), cmd.to_string()))
    }).collect()
}

/// Stop every process running from `app` and verify: the daemon gets `cua-driver stop` first (it closes its
/// socket and exits), whatever is still there after 3 s gets SIGTERM, after another 5 s SIGKILL. Returns the
/// processes still alive at the end ("pid command"), empty when all are gone.
pub fn stop_app_processes(app: &str) -> Vec<String> {
    let wait_gone = |secs: f64| -> Vec<(String, String)> {
        let t0 = std::time::Instant::now();
        loop {
            let left = app_processes(app);
            if left.is_empty() || t0.elapsed().as_secs_f64() >= secs {
                return left;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    };
    if app_processes(app).is_empty() {
        return vec![];
    }
    let bin = format!("{}/Contents/MacOS/cua-driver", app.trim_end_matches('/'));
    if std::path::Path::new(&bin).is_file() {
        let _ = driver_stop(&bin);
    }
    #[allow(unused_mut)]
    let mut left = wait_gone(3.0);
    #[cfg(unix)]
    {
        for (sig, secs) in [(sys::SIGTERM, 5.0), (sys::SIGKILL, 2.0)] {
            if left.is_empty() {
                break;
            }
            for (pid, _) in &left {
                if let Ok(p) = pid.parse::<i32>() {
                    unsafe {
                        sys::kill(p, sig);
                    }
                }
            }
            left = wait_gone(secs);
        }
    }
    left.into_iter().map(|(p, c)| format!("{p} {c}")).collect()
}

/// At gate exit on macOS: stop the daemon this session started.
pub fn stop_mac_daemon_if_ours(driver: &str) {
    stop_mac_daemon_unless(MAC_DAEMON_BEFORE.load(Ordering::SeqCst), driver);
}

pub fn set_driver(pid: u32) {
    DRIVER.store(pid as i32, Ordering::SeqCst);
}

pub fn live_count() -> usize {
    LIVE.iter().filter(|s| s.load(Ordering::SeqCst) != 0).count()
}

/// Variables a child process (agent commands, launched apps, the driver, role probes) must never inherit: model keys,
/// the shell's ask-a-person token, and the gate's own configuration (the agent must not read or change them).
pub fn is_secret_var(k: &str) -> bool {
    let u = k.to_ascii_uppercase();
    u.starts_with("GUI_GATE_") || u.starts_with("GUI_PI_") || u.ends_with("_KEY") || u.ends_with("_API_KEY") || u.contains("TOKEN")
        || u.ends_with("_SECRET") || u.contains("PASSWORD") || u == "PI_CODING_AGENT_DIR"
}

/// Exact variable names a child (driver, app, agent command, probe) may inherit: what a desktop session, a shell,
/// locale and proxies need. Everything else is dropped.
const CHILD_ENV_NAMES: &[&str] = &[
    "PATH", "HOME", "USER", "LOGNAME", "SHELL", "PWD", "TERM", "COLORTERM", "LANG", "LANGUAGE", "TZ", "TMPDIR",
    "DISPLAY", "WAYLAND_DISPLAY", "XAUTHORITY", "DBUS_SESSION_BUS_ADDRESS", "AT_SPI_BUS_ADDRESS", "NO_AT_BRIDGE",
    "DESKTOP_SESSION", "SESSION_MANAGER", "SSH_AUTH_SOCK",
    "http_proxy", "https_proxy", "no_proxy", "all_proxy", "HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY", "ALL_PROXY",
    // Windows
    "SYSTEMROOT", "SystemRoot", "SYSTEMDRIVE", "SystemDrive", "WINDIR", "windir", "COMSPEC", "ComSpec", "PATHEXT",
    "TEMP", "TMP", "USERPROFILE", "USERNAME", "USERDOMAIN", "APPDATA", "LOCALAPPDATA", "PROGRAMDATA", "ProgramData",
    "ProgramFiles", "ProgramFiles(x86)", "ProgramW6432", "CommonProgramFiles", "PUBLIC", "HOMEDRIVE", "HOMEPATH",
    "NUMBER_OF_PROCESSORS", "PROCESSOR_ARCHITECTURE", "OS", "PSModulePath",
    // macOS
    "__CF_USER_TEXT_ENCODING",
];
/// Name prefixes a child may inherit (locale, XDG dirs, toolkit settings, the driver's own settings).
const CHILD_ENV_PREFIXES: &[&str] = &["LC_", "XDG_", "GTK_", "GDK_", "QT_", "CUA_", "CUA_DRIVER_"];

/// Whether a child may inherit `k`: on the allowlist (or named by the launcher in GUI_GATE_CHILD_ENV, a
/// comma-separated list read from the gate's own start-up environment; `NAME*` is a prefix) and never a secret.
pub fn child_may_inherit(k: &str, extra: &[String]) -> bool {
    if is_secret_var(k) {
        return false;
    }
    CHILD_ENV_NAMES.contains(&k) || CHILD_ENV_PREFIXES.iter().any(|p| k.starts_with(p))
        || extra.iter().any(|e| e == k || (e.ends_with('*') && k.starts_with(&e[..e.len() - 1])))
}

fn child_env_extra() -> &'static Vec<String> {
    static X: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    X.get_or_init(|| std::env::var("GUI_GATE_CHILD_ENV").unwrap_or_default().split(',')
        .map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
}

/// Give a child an explicit allowlisted environment: start empty, copy only allowed, non-secret variables.
/// Call before any `env()` the caller sets on purpose.
pub fn scrub_env(c: &mut Command) {
    let extra = child_env_extra();
    c.env_clear();
    for (k, v) in std::env::vars_os() {
        if let Some(ks) = k.to_str() {
            if child_may_inherit(ks, extra) {
                c.env(&k, v);
            }
        }
    }
}

/// The shell the gate runs agent commands with: `bash -c CMD` in a new process group (unix). None on Windows.
pub fn shell(cmd: &str) -> Option<Command> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let mut c = Command::new("bash");
        c.arg("-c").arg(cmd);
        c.process_group(0);
        scrub_env(&mut c);
        Some(c)
    }
    #[cfg(not(unix))]
    {
        let _ = cmd;
        None
    }
}

#[cfg(unix)]
mod sys {
    extern "C" {
        pub fn kill(pid: i32, sig: i32) -> i32;
        pub fn signal(sig: i32, handler: usize) -> usize;
        pub fn raise(sig: i32) -> i32;
    }
    pub const SIGHUP: i32 = 1;
    pub const SIGINT: i32 = 2;
    pub const SIGKILL: i32 = 9;
    pub const SIGTERM: i32 = 15;
}

/// Kill a command's whole process group (TERM, then KILL) and reap the leader.
pub fn kill_tree(child: &mut Child) {
    let pid = child.id();
    #[cfg(unix)]
    unsafe {
        sys::kill(-(pid as i32), sys::SIGTERM);
        std::thread::sleep(std::time::Duration::from_millis(150));
        sys::kill(-(pid as i32), sys::SIGKILL);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill").args(["/T", "/F", "/PID", &pid.to_string()]).creation_flags(0x0800_0000).output();
    }
    let _ = child.kill();
    let _ = child.wait();
    unregister(pid);
}

/// Kill one registered command group at once (async-signal-safe on unix: only kill(2)).
fn kill_id(id: i32) {
    if id <= 0 {
        return;
    }
    #[cfg(unix)]
    unsafe {
        sys::kill(-id, sys::SIGKILL);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill").args(["/T", "/F", "/PID", &id.to_string()]).creation_flags(0x0800_0000).output();
    }
}

/// Kill every registered command group now (async-signal-safe on unix: only atomics and kill(2)).
pub fn kill_all() {
    for s in LIVE.iter() {
        kill_id(s.swap(0, Ordering::SeqCst));
    }
}

#[cfg(unix)]
extern "C" fn on_signal(sig: i32) {
    kill_all();
    let d = DRIVER.load(Ordering::SeqCst);
    unsafe {
        if d > 0 {
            sys::kill(d, sys::SIGTERM);
        }
        sys::signal(sig, 0); // SIG_DFL, then die of the same signal
        sys::raise(sig);
    }
}

/// Make SIGTERM / SIGINT / SIGHUP take the agent's commands and the driver down with the gate.
pub fn install_signal_handlers() {
    #[cfg(unix)]
    unsafe {
        for s in [sys::SIGTERM, sys::SIGINT, sys::SIGHUP] {
            sys::signal(s, on_signal as extern "C" fn(i32) as usize);
        }
    }
}

/// Is a process alive? (unix: kill(pid, 0); elsewhere unknown = true)
pub fn alive(pid: i32) -> bool {
    #[cfg(unix)]
    unsafe {
        if pid <= 0 {
            return false;
        }
        if sys::kill(pid, 0) == 0 {
            return true;
        }
        // EPERM: exists but not ours
        std::io::Error::last_os_error().raw_os_error() == Some(1)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

/// Run a program directly (no shell) with a timeout: (exit code, stdout, stderr). Used for the read-only helpers
/// (pdftotext, python3 reading a document, tesseract) so they work the same on every OS and never outlive the gate.
pub fn run_argv(argv: &[&str], timeout_s: f64) -> (Option<i32>, String, String) {
    run_argv_env(argv, &[], timeout_s)
}

/// `run_argv` with extra environment variables.
pub fn run_argv_env(argv: &[&str], envs: &[(&str, &str)], timeout_s: f64) -> (Option<i32>, String, String) {
    use std::io::Read;
    use std::process::Stdio;
    let Some((prog, rest)) = argv.split_first() else { return (None, String::new(), "empty command".into()) };
    let mut c = Command::new(prog);
    scrub_env(&mut c);
    c.envs(envs.iter().copied());
    c.args(rest).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flashing up on the user's desktop
    }
    let mut ch = match c.spawn() {
        Ok(ch) => ch,
        Err(e) => return (None, String::new(), format!("could not start {prog}: {e}")),
    };
    register(ch.id());
    let rd = |r: Option<Box<dyn Read + Send>>| std::thread::spawn(move || {
        let mut b = vec![];
        if let Some(mut r) = r {
            let _ = r.read_to_end(&mut b);
        }
        String::from_utf8_lossy(&b).to_string()
    });
    let ho = rd(ch.stdout.take().map(|x| Box::new(x) as Box<dyn Read + Send>));
    let he = rd(ch.stderr.take().map(|x| Box::new(x) as Box<dyn Read + Send>));
    let t0 = std::time::Instant::now();
    let code = loop {
        match ch.try_wait() {
            Ok(Some(st)) => break st.code(),
            Ok(None) if t0.elapsed().as_secs_f64() > timeout_s => {
                kill_tree(&mut ch);
                break None;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(15)),
            Err(_) => break None,
        }
    };
    unregister(ch.id());
    (code, ho.join().unwrap_or_default(), he.join().unwrap_or_default())
}

/// The Python interpreter on PATH (python3, else python), for the read-only document helpers.
pub fn python() -> Option<String> {
    crate::install::which("python3").or_else(|| crate::install::which("python"))
}

/// Last change of a path: the newer of mtime and (unix) ctime - a rename or chmod changes only the ctime.
pub fn changed_at(m: &std::fs::Metadata) -> Option<std::time::SystemTime> {
    #[cfg(unix)]
    let c = {
        use std::os::unix::fs::MetadataExt;
        std::time::SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::new(m.ctime().max(0) as u64, m.ctime_nsec().clamp(0, 999_999_999) as u32))
    };
    #[cfg(not(unix))]
    let c: Option<std::time::SystemTime> = None;
    match (m.modified().ok(), c) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn stop_app_processes_leaves_nothing() {
        // a fake app bundle whose "driver" ignores `stop` and keeps running (like a stray Cua telemetry worker)
        let d = std::env::temp_dir().join(format!("gg-app-{}", std::process::id()));
        let app = d.join("CuaDriver.app");
        let mac = app.join("Contents").join("MacOS");
        std::fs::create_dir_all(&mac).unwrap();
        let bin = mac.join("cua-driver");
        std::fs::copy("/bin/sleep", &bin).unwrap();
        let mut kids: Vec<Child> = (0..2).map(|_| Command::new(&bin).arg("30").spawn().unwrap()).collect();
        let app_s = app.to_string_lossy().to_string();
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(app_processes(&app_s).len(), 2);
        let reaper = std::thread::spawn(move || for k in kids.iter_mut() { let _ = k.wait(); });
        let left = stop_app_processes(&app_s);
        assert!(left.is_empty(), "{left:?}");
        reaper.join().unwrap();
        assert!(app_processes(&app_s).is_empty());
        assert!(stop_app_processes(&app_s).is_empty()); // nothing running: a no-op
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn kill_all_takes_grandchildren() {
        // a command that leaves a background grandchild behind
        let mut c = shell("sleep 30 & sleep 30; wait").unwrap();
        c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        let mut ch = c.spawn().unwrap();
        register(ch.id());
        std::thread::sleep(std::time::Duration::from_millis(200));
        let pg = ch.id() as i32;
        assert!(alive(pg));
        // what kill_all does for each registered slot (kill_all itself would also hit other tests' commands)
        unregister(ch.id());
        kill_id(pg);
        let _ = ch.wait();
        std::thread::sleep(std::time::Duration::from_millis(200));
        // no process of that group is left
        let left = std::fs::read_dir("/proc").map(|rd| rd.flatten().filter(|e| {
            std::fs::read_to_string(e.path().join("stat")).ok().and_then(|s| s.rsplit(')').next().map(|r| r.split_whitespace().nth(2).map(|g| g == pg.to_string()).unwrap_or(false))).unwrap_or(false)
        }).count()).unwrap_or(0);
        assert_eq!(left, 0);
    }
}

#[cfg(test)]
mod env_tests {
    use super::*;
    #[test]
    fn child_env_is_an_allowlist() {
        let none: Vec<String> = vec![];
        for ok in ["PATH", "HOME", "DISPLAY", "LC_ALL", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS", "https_proxy"] {
            assert!(child_may_inherit(ok, &none), "{ok}");
        }
        for bad in ["GUI_GATE_REVIEW_KEY", "GUI_PI_MAIN_KEY", "OPENAI_API_KEY", "GITHUB_TOKEN", "AWS_SECRET_ACCESS_KEY",
                    "LD_PRELOAD", "NODE_OPTIONS", "PYTHONPATH", "RANDOM_THING", "XDG_TOKEN"] {
            assert!(!child_may_inherit(bad, &none), "{bad}");
        }
        assert!(child_may_inherit("JAVA_HOME", &["JAVA_HOME".to_string()]));
        assert!(!child_may_inherit("MY_API_KEY", &["MY_API_KEY".to_string()]));
        assert!(child_may_inherit("FAKE_PNG", &["FAKE_*".to_string()]));
        assert!(!child_may_inherit("FAKE_TOKEN", &["FAKE_*".to_string()]));
    }
}
