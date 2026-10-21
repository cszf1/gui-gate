//! Newline-delimited JSON-RPC plumbing for MCP over stdio (one JSON object per line).
use crate::util::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Thread-safe line writer for our own stdout.
pub struct Out {
    lock: Mutex<()>,
}

impl Out {
    pub fn new() -> Self {
        Out { lock: Mutex::new(()) }
    }
    pub fn send(&self, msg: &Value) {
        let line = serde_json::to_string(msg).unwrap_or_default();
        let _g = self.lock.lock().unwrap();
        let mut so = std::io::stdout().lock();
        let _ = so.write_all(line.as_bytes());
        let _ = so.write_all(b"\n");
        let _ = so.flush();
    }
}

pub type Handler = Arc<dyn Fn(Value) + Send + Sync>;

/// Runs `cua-driver mcp` (or any MCP server) as a child and talks to it.
///  * call(): request/response for the gate's own calls (ids "gate-N");
///  * forward(): send an agent message untouched; its response goes to the unsolicited handler.
pub struct ChildMCP {
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: Arc<Mutex<HashMap<String, Sender<Value>>>>,
    ids: AtomicU64,
}

impl ChildMCP {
    pub fn spawn(cmd: &[String], on_unsolicited: Handler, debug_stderr: bool) -> std::io::Result<Arc<ChildMCP>> {
        let mut c = Command::new(&cmd[0]);
        c.args(&cmd[1..]).stdin(Stdio::piped()).stdout(Stdio::piped())
            .stderr(if debug_stderr { Stdio::inherit() } else { Stdio::null() });
        if cmd[0].contains("site-packages") || cmd[0].contains("dist-packages") {
            // same as the cua-driver package's own launcher: the binary came from the Python package
            if std::env::var("CUA_DRIVER_INSTALL_CHANNEL").is_err() {
                c.env("CUA_DRIVER_INSTALL_CHANNEL", "python_package");
            }
        }
        let mut child = c.spawn()?;
        let stdout = child.stdout.take().unwrap();
        let stdin = child.stdin.take();
        let pending: Arc<Mutex<HashMap<String, Sender<Value>>>> = Arc::new(Mutex::new(HashMap::new()));
        let p2 = pending.clone();
        std::thread::spawn(move || {
            let rd = BufReader::new(stdout);
            for line in rd.lines() {
                let Ok(line) = line else { break };
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let Ok(msg) = serde_json::from_str::<Value>(line) else { continue };
                if let Some(mid) = msg.get("id").and_then(|i| i.as_str()) {
                    if mid.starts_with("gate-") && msg.get("method").is_none() {
                        let tx = p2.lock().unwrap().remove(mid);
                        if let Some(tx) = tx {
                            let _ = tx.send(msg);
                            continue;
                        }
                    }
                }
                on_unsolicited(msg);
            }
            // child exited: fail every pending call
            let pend: Vec<Sender<Value>> = p2.lock().unwrap().drain().map(|(_, v)| v).collect();
            for tx in pend {
                let _ = tx.send(json!({"error": {"code": -32000, "message": "driver exited"}}));
            }
        });
        Ok(Arc::new(ChildMCP { child: Mutex::new(child), stdin: Mutex::new(stdin), pending, ids: AtomicU64::new(1) }))
    }

    pub fn alive(&self) -> bool {
        matches!(self.child.lock().unwrap().try_wait(), Ok(None))
    }

    fn write(&self, msg: &Value) -> std::io::Result<()> {
        let line = serde_json::to_string(msg).unwrap_or_default();
        let mut g = self.stdin.lock().unwrap();
        let Some(si) = g.as_mut() else { return Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "closed")) };
        si.write_all(line.as_bytes())?;
        si.write_all(b"\n")?;
        si.flush()
    }

    pub fn call(&self, method: &str, params: Option<Value>, timeout: f64) -> Value {
        let mid = format!("gate-{}", self.ids.fetch_add(1, Ordering::SeqCst));
        let (tx, rx) = channel();
        self.pending.lock().unwrap().insert(mid.clone(), tx);
        let mut msg = json!({"jsonrpc": "2.0", "id": mid, "method": method});
        if let Some(p) = params {
            msg["params"] = p;
        }
        if self.write(&msg).is_err() {
            self.pending.lock().unwrap().remove(&mid);
            return json!({"error": {"code": -32000, "message": "driver exited"}});
        }
        match rx.recv_timeout(Duration::from_secs_f64(timeout.max(0.01))) {
            Ok(v) => v,
            Err(_) => {
                self.pending.lock().unwrap().remove(&mid);
                json!({"error": {"code": -32001, "message": format!("timeout calling {method}")}})
            }
        }
    }

    /// tools/call helper returning the `result` object (or an error-shaped result).
    pub fn tool(&self, name: &str, args: &Value, timeout: f64) -> Value {
        let r = self.call("tools/call", Some(json!({"name": name, "arguments": args})), timeout);
        if let Some(res) = r.get("result") {
            return res.clone();
        }
        let m = r.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()).unwrap_or("error").to_string();
        text_result(m, true)
    }

    pub fn forward(&self, msg: &Value) -> std::io::Result<()> {
        self.write(msg)
    }

    pub fn close(&self) {
        self.stdin.lock().unwrap().take();
        let mut c = self.child.lock().unwrap();
        let t0 = Instant::now();
        loop {
            match c.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) if t0.elapsed() < Duration::from_secs(3) => std::thread::sleep(Duration::from_millis(20)),
                _ => {
                    let _ = c.kill();
                    let _ = c.wait();
                    return;
                }
            }
        }
    }
}
