//! Append-only JSONL event log (one file per session).
use crate::util::*;
use serde_json::{Map, Value};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;

pub struct EventLog {
    pub session: String,
    pub path: Option<String>,
    f: Mutex<Option<File>>,
}

impl EventLog {
    pub fn new(path: Option<String>) -> Self {
        let session = rand_hex(4);
        let path = match path.as_deref() {
            Some("auto") => {
                let d = env("GUI_GATE_LOG_DIR").unwrap_or_else(|| "~/.gui-gate/logs".into());
                let d = crate::sidefx::expanduser(&d);
                let _ = std::fs::create_dir_all(&d);
                Some(format!("{d}/{}-{session}.jsonl", stamp()))
            }
            Some(p) => Some(p.to_string()),
            None => None,
        };
        let f = path.as_ref().and_then(|p| OpenOptions::new().create(true).append(true).open(p).ok());
        EventLog { session, path, f: Mutex::new(f) }
    }

    pub fn write(&self, ev: Value) {
        let mut g = self.f.lock().unwrap();
        let Some(f) = g.as_mut() else { return };
        let mut m = Map::new();
        m.insert("ts".into(), Value::from(round_to(now(), 3)));
        m.insert("session".into(), Value::from(self.session.clone()));
        if let Value::Object(o) = ev {
            for (k, v) in o {
                m.insert(k, v);
            }
        }
        let _ = writeln!(f, "{}", redact_secrets(&Value::Object(m).to_string()));
        let _ = f.flush();
    }
}
