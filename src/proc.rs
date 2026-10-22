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
    let _ = run_argv(&[driver, "stop"], 10.0);
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

/// The shell the gate runs agent commands with: `bash -c CMD` in a new process group (unix). None on Windows.
pub fn shell(cmd: &str) -> Option<Command> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let mut c = Command::new("bash");
        c.arg("-c").arg(cmd);
        c.process_group(0);
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
    use std::io::Read;
    use std::process::Stdio;
    let Some((prog, rest)) = argv.split_first() else { return (None, String::new(), "empty command".into()) };
    let mut c = Command::new(prog);
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
