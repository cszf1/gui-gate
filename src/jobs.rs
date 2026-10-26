//! 0.9.7 long-running commands (Pointer run_background / install_package): `run_command mode=start` launches the
//! command in the background (after the same risk check as a normal run_command) and returns a job id at once;
//! `mode=poll` returns its status and output tail; `mode=stop` kills it. GUI_GATE_JOBS.
//! Hygiene: the output goes to a temp file that is unlinked right after the job starts on unix (the gate keeps a
//! read handle, so nothing is left on disk even if the gate is killed); every job still running when the gate ends
//! (client closed stdin, SIGTERM/SIGINT/SIGHUP, or the Jobs value is dropped) is killed with its whole process group.
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::process::{Child, Stdio};
use std::sync::Mutex;
use std::time::Instant;

pub struct Job {
    pub id: u32,
    pub command: String,
    pub cwd: String,
    /// the log path while it still exists on disk (Windows; on unix it is unlinked at start)
    pub out: Option<String>,
    /// the log's file name as created (unique per job and process; unlinked at start on unix)
    pub log_name: String,
    reader: std::fs::File,
    pub child: Child,
    pub started: Instant,
    pub exit: Option<Option<i32>>,
}

impl Job {
    fn output(&mut self) -> String {
        let mut buf = vec![];
        let _ = self.reader.seek(SeekFrom::Start(0));
        let _ = (&self.reader).take(8 * 1024 * 1024).read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).to_string()
    }
    fn finish(&mut self) {
        crate::proc::unregister(self.child.id());
    }
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
        let Some(mut c) = crate::proc::shell(cmd) else { return Err("background jobs are not available on this OS".into()) };
        // unique per job: several Jobs values in one process (tests run in parallel threads) all count ids from 1
        let out = std::env::temp_dir().join(format!("gui-gate-job-{}-{id}-{}.log", std::process::id(), crate::util::rand_hex(6)))
            .to_string_lossy().to_string();
        let log_name = out.clone();
        let f = std::fs::File::create(&out).map_err(|e| e.to_string())?;
        let f2 = f.try_clone().map_err(|e| e.to_string())?;
        let reader = std::fs::File::open(&out).map_err(|e| e.to_string())?;
        c.current_dir(cwd).stdin(Stdio::null()).stdout(f).stderr(f2)
            .env("PAGER", "cat").env("TERM", "dumb").env("DEBIAN_FRONTEND", "noninteractive").env("PYTHONUNBUFFERED", "1");
        let child = match c.spawn() {
            Ok(ch) => ch,
            Err(e) => {
                let _ = std::fs::remove_file(&out);
                return Err(format!("could not start bash: {e}"));
            }
        };
        crate::proc::register(child.id());
        // unix: the open handles keep the file alive; no name is left in the temp dir
        let out = if cfg!(unix) && std::fs::remove_file(&out).is_ok() { None } else { Some(out) };
        self.map.lock().unwrap().insert(id, Job { id, command: cmd.to_string(), cwd: cwd.to_string(), out, log_name, reader, child, started: Instant::now(), exit: None });
        Ok(id)
    }

    /// (status text, finished?) for a job; `stop` kills it first.
    pub fn poll(&self, id: u32, stop: bool, tail_chars: usize) -> Result<(String, Value), String> {
        let mut m = self.map.lock().unwrap();
        let mut ks: Vec<String> = m.keys().map(|k| k.to_string()).collect();
        ks.sort();
        let j = m.get_mut(&id).ok_or_else(|| format!("no job {id} (jobs: {})", if ks.is_empty() { "none".to_string() } else { ks.join(", ") }))?;
        if stop && j.exit.is_none() {
            crate::proc::kill_tree(&mut j.child);
            j.exit = Some(None);
        }
        if j.exit.is_none() {
            if let Ok(Some(st)) = j.child.try_wait() {
                j.exit = Some(st.code());
                j.finish();
            }
        }
        let out = j.output();
        let el = j.started.elapsed().as_secs_f64();
        let status = match j.exit {
            None => format!("running for {el:.0} s"),
            Some(Some(c)) => format!("finished, exit {c}"),
            Some(None) => if stop { "stopped".to_string() } else { "killed".to_string() },
        };
        let t = format!("[run_command job {id}] {status} (cwd {}): {}\n{}", j.cwd, crate::util::trunc(&j.command, 120),
                        crate::util::defang(&crate::command::tail_cap(&out, tail_chars)));
        Ok((t, json!({"job": id, "running": j.exit.is_none(), "exit_code": j.exit.flatten(), "seconds": (el * 10.0).round() / 10.0})))
    }

    /// Kill every job still running and delete any log still on disk (the gate is ending). Returns how many were killed.
    pub fn stop_all(&self) -> usize {
        let mut n = 0;
        let mut m = match self.map.lock() { Ok(m) => m, Err(p) => p.into_inner() };
        for (_, j) in m.iter_mut() {
            if j.exit.is_none() && matches!(j.child.try_wait(), Ok(None)) {
                crate::proc::kill_tree(&mut j.child);
                n += 1;
            }
            j.exit.get_or_insert(None);
            j.finish();
            if let Some(p) = j.out.take() {
                let _ = std::fs::remove_file(p);
            }
        }
        m.clear();
        n
    }
}

impl Drop for Jobs {
    fn drop(&mut self) {
        self.stop_all();
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    fn tmp() -> String {
        std::env::temp_dir().to_string_lossy().to_string()
    }

    #[test]
    fn start_poll_stop() {
        let j = Jobs::default();
        let id = j.start("echo begin; sleep 0.3; echo end", &tmp()).unwrap();
        let (t, v) = j.poll(id, false, 1000).unwrap();
        assert!(v["running"].as_bool().unwrap() || t.contains("finished"), "{t}");
        std::thread::sleep(std::time::Duration::from_millis(700));
        let (t, v) = j.poll(id, false, 1000).unwrap();
        assert_eq!(v["exit_code"], json!(0), "{t}");
        assert!(t.contains("begin") && t.contains("end"));
        let id2 = j.start("sleep 30", &tmp()).unwrap();
        let (t, v) = j.poll(id2, true, 100).unwrap();
        assert!(t.contains("stopped") && !v["running"].as_bool().unwrap());
        assert!(j.poll(99, false, 10).is_err());
    }

    /// Jobs never leave a log file in the temp dir, and dropping Jobs (the gate ending) kills what still runs,
    /// grandchildren included.
    #[test]
    fn no_orphans_no_logs() {
        let mark = format!("gg-orphan-{}", crate::util::rand_hex(4));
        let j = Jobs::default();
        let id = j.start(&format!("(sleep 60; echo {mark}) & sleep 60"), &tmp()).unwrap();
        let log = std::path::PathBuf::from(&j.map.lock().unwrap()[&id].log_name);
        assert!(!log.exists(), "log left on disk: {}", log.display());
        let pgid = j.map.lock().unwrap()[&id].child.id() as i32;
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(crate::proc::alive(pgid));
        drop(j);
        std::thread::sleep(std::time::Duration::from_millis(300));
        let in_group = std::fs::read_dir("/proc").map(|rd| rd.flatten().filter(|e| {
            std::fs::read_to_string(e.path().join("stat")).ok()
                .and_then(|s| s.rsplit(')').next().and_then(|r| r.split_whitespace().nth(2).map(|g| g == pgid.to_string())))
                .unwrap_or(false)
        }).count()).unwrap_or(0);
        assert_eq!(in_group, 0, "processes of the job survived");
    }
}
