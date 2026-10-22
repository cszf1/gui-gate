//! 0.9.7 long-running commands (Pointer run_background / install_package): `run_command mode=start` launches the
//! command detached (after the same risk check as a normal run_command) and returns a job id at once; `mode=poll`
//! returns its status and output tail; `mode=stop` kills it. Output goes to a temp file per job. GUI_GATE_JOBS.
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Instant;

pub struct Job {
    pub id: u32,
    pub command: String,
    pub cwd: String,
    pub out: String,
    pub child: Child,
    pub started: Instant,
    pub exit: Option<Option<i32>>,
}

#[derive(Default)]
pub struct Jobs {
    pub map: Mutex<HashMap<u32, Job>>,
    next: Mutex<u32>,
}

impl Jobs {
    pub fn start(&self, cmd: &str, cwd: &str) -> Result<u32, String> {
        let id = {
            let mut n = self.next.lock().unwrap();
            *n += 1;
            *n
        };
        let out = std::env::temp_dir().join(format!("gui-gate-job-{}-{id}.log", std::process::id())).to_string_lossy().to_string();
        let f = std::fs::File::create(&out).map_err(|e| e.to_string())?;
        let f2 = f.try_clone().map_err(|e| e.to_string())?;
        let mut c = Command::new("bash");
        c.arg("-c").arg(cmd).current_dir(cwd).stdin(Stdio::null()).stdout(f).stderr(f2)
            .env("PAGER", "cat").env("TERM", "dumb").env("DEBIAN_FRONTEND", "noninteractive").env("PYTHONUNBUFFERED", "1");
        {
            use std::os::unix::process::CommandExt;
            c.process_group(0);
        }
        let child = c.spawn().map_err(|e| format!("could not start bash: {e}"))?;
        self.map.lock().unwrap().insert(id, Job { id, command: cmd.to_string(), cwd: cwd.to_string(), out, child, started: Instant::now(), exit: None });
        Ok(id)
    }

    /// (status text, finished?) for a job; `stop` kills it first.
    pub fn poll(&self, id: u32, stop: bool, tail_chars: usize) -> Result<(String, Value), String> {
        let mut m = self.map.lock().unwrap();
        let mut ks: Vec<String> = m.keys().map(|k| k.to_string()).collect();
        ks.sort();
        let j = m.get_mut(&id).ok_or_else(|| format!("no job {id} (jobs: {})", if ks.is_empty() { "none".to_string() } else { ks.join(", ") }))?;
        if stop && j.exit.is_none() {
            let _ = Command::new("kill").args(["-TERM", "--", &format!("-{}", j.child.id())]).status();
            std::thread::sleep(std::time::Duration::from_millis(200));
            let _ = Command::new("kill").args(["-KILL", "--", &format!("-{}", j.child.id())]).status();
            let _ = j.child.kill();
            let _ = j.child.wait();
            j.exit = Some(None);
        }
        if j.exit.is_none() {
            if let Ok(Some(st)) = j.child.try_wait() {
                j.exit = Some(st.code());
            }
        }
        let out = std::fs::read_to_string(&j.out).unwrap_or_default();
        let el = j.started.elapsed().as_secs_f64();
        let status = match j.exit {
            None => format!("running for {el:.0} s"),
            Some(Some(c)) => format!("finished, exit {c}"),
            Some(None) => if stop { "stopped".to_string() } else { "killed".to_string() },
        };
        let t = format!("[run_command job {id}] {status} (cwd {}): {}\n{}", j.cwd, crate::util::trunc(&j.command, 120),
                        crate::command::tail_cap(&out, tail_chars));
        Ok((t, json!({"job": id, "running": j.exit.is_none(), "exit_code": j.exit.flatten(), "seconds": (el * 10.0).round() / 10.0})))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_poll_stop() {
        let j = Jobs::default();
        let id = j.start("echo begin; sleep 0.3; echo end", "/tmp").unwrap();
        let (t, v) = j.poll(id, false, 1000).unwrap();
        assert!(v["running"].as_bool().unwrap() || t.contains("finished"), "{t}");
        std::thread::sleep(std::time::Duration::from_millis(700));
        let (t, v) = j.poll(id, false, 1000).unwrap();
        assert_eq!(v["exit_code"], json!(0), "{t}");
        assert!(t.contains("begin") && t.contains("end"));
        let id2 = j.start("sleep 30", "/tmp").unwrap();
        let (t, v) = j.poll(id2, true, 100).unwrap();
        assert!(t.contains("stopped") && !v["running"].as_bool().unwrap());
        assert!(j.poll(99, false, 10).is_err());
    }
}
