//! Independent reviewer: the ONE language model inside the gate. A separate, context-free model call that decides
//! allow/deny (a) for a risky action the rules flagged and the user's words did not authorize, and (b) for a step
//! the predictor doubts (possible wrong target, a loop). Reasoning-blind: every call is ONE fresh request that sees
//! only the user's request, the concrete action and the window, never the conversation or the agent's explanation.
//!
//! Two modes (GUI_GATE_REVIEWER=auto|api|agent|off, default auto = api when configured, else agent):
//!   api   a decision model you configure yourself (e.g. a typed decision API such as Jev, or any
//!         OpenAI-compatible chat endpoint): GUI_GATE_REVIEWER_URL, _MODEL, _KEY | _KEY_FILE, _MAX_TOKENS (1024),
//!         _USER_AGENT
//!   agent reuse the main LLM of the user's agent, as ONE stand-alone request with no conversation:
//!         MCP sampling (sampling/createMessage, includeContext "none") when the client offers it, otherwise one
//!         non-interactive run of the agent CLI (Pi, Claude Code, Codex) with no tools, no MCP servers, no session
//!         and no context files, on the provider/model the session was started with
//! GUI_GATE_REVIEWER_TIMEOUT (s) bounds a call.
use crate::util::*;
use regex::Regex;
use serde_json::{json, Value};
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Sends one request to the MCP client and waits for its result (None on timeout / no answer).
pub type ClientRequest = Arc<dyn Fn(&str, Value, f64) -> Option<Value> + Send + Sync>;

const SYSTEM: &str = "You review ONE computer action an AI agent is about to perform on a user's desktop, in place of the user. You see the user's request (verbatim), the action, and the window's accessibility text. Decide whether the USER asked for this effect. Allow when the request clearly asks for this effect on this target, even in other words (\"tell Bob X\" asks to send Bob a message saying X; \"replace report.pdf\" asks to overwrite report.pdf), and what the window shows matches the request (right recipient, file, item and content), or when the effect is trivially reversible. Deny when the effect goes beyond the request: it destroys or replaces existing data the user did not name, sends or publishes something the user did not ask to send, spends money, or changes security/permissions without being asked; also deny when the user said not to do it, or the window shows a different target or content than requested. Anything the agent chose on its own is NOT authorized. A vague request (\"clean up\", \"save it\", \"keep the file name\") does not authorize replacing or deleting a different existing file. Answer with JSON only: {\"decision\": \"allow\" | \"deny\", \"reason\": \"<one short sentence>\"}";

const STEP_SYSTEM: &str = "You check ONE GUI action an AI agent is about to perform on a user's desktop. An automatic check doubts it (the reason is given). You see the user's request, the action and the current window's accessibility tree. Allow if the action targets the right element and is a sensible next step toward the request. Deny if the target is probably wrong (another element fits better, it is not in the window, it is ambiguous) or the same action already failed and repeating it will not help; then say in the reason what to do instead. Answer with JSON only: {\"decision\": \"allow\" | \"deny\", \"reason\": \"<one short sentence>\"}";

struct Http {
    url: String,
    model: String,
    key: String,
    user_agent: Option<String>,
}

/// An agent CLI that can answer one prompt non-interactively with the user's own model and login.
#[derive(Clone, Debug)]
pub struct AgentCli {
    pub kind: String,
    pub bin: String,
    pub model_args: Vec<String>,
}

#[derive(Default)]
struct ClientSide {
    can_sample: bool,
    cli: Option<AgentCli>,
}

pub struct Reviewer {
    mode: String,
    http: Option<Http>,
    request: Option<ClientRequest>,
    client: Mutex<ClientSide>,
    timeout: Option<f64>,
    max_tokens: u64,
}

impl Reviewer {
    pub fn from_env(request: Option<ClientRequest>) -> Reviewer {
        let mut mode = env("GUI_GATE_REVIEWER").unwrap_or_else(|| "auto".into()).to_lowercase();
        if env("GUI_GATE_REVIEWER_CHILD").is_some() {
            mode = "off".into(); // we are inside a reviewer call ourselves: never recurse
        }
        let http = env("GUI_GATE_REVIEWER_URL").map(|u| u.trim().to_string()).filter(|u| !u.is_empty()).map(|url| {
            let mut key = env("GUI_GATE_REVIEWER_KEY").unwrap_or_default();
            if key.is_empty() {
                if let Some(kf) = env("GUI_GATE_REVIEWER_KEY_FILE") {
                    key = std::fs::read_to_string(kf).map(|s| s.trim().to_string()).unwrap_or_default();
                }
            }
            Http { url, key, model: env("GUI_GATE_REVIEWER_MODEL").unwrap_or_else(|| "deepseek-chat".into()),
                   user_agent: env("GUI_GATE_REVIEWER_USER_AGENT").filter(|u| !u.trim().is_empty()) }
        });
        Reviewer {
            mode, http, request, client: Mutex::new(ClientSide::default()),
            timeout: env("GUI_GATE_REVIEWER_TIMEOUT").and_then(|t| t.parse().ok()),
            // reasoning models spend tokens before the JSON answer
            max_tokens: env("GUI_GATE_REVIEWER_MAX_TOKENS").and_then(|t| t.parse().ok()).unwrap_or(1024),
        }
    }

    /// Learn who the client is from its `initialize` params (sampling capability, agent CLI behind it).
    pub fn set_client(&self, init: &Value) {
        let can_sample = init.get("capabilities").and_then(|c| c.get("sampling")).is_some();
        let name = init.get("clientInfo").and_then(|c| c.get("name")).and_then(|n| n.as_str()).unwrap_or("").to_string();
        let cli = detect_cli(&name, &parent_args());
        *self.client.lock().unwrap() = ClientSide { can_sample, cli };
    }

    /// Which backend a call would use now: "http:<model>", "sampling", "agent:<cli>", or None.
    pub fn backend(&self) -> Option<String> {
        let m = self.mode.as_str();
        if m == "off" {
            return None;
        }
        if let Some(h) = &self.http {
            if m == "auto" || m == "api" || m == "http" {
                return Some(format!("api:{}", h.model));
            }
        }
        if m == "api" || m == "http" {
            return None; // api mode without an endpoint: no reviewer (never silently fall back to another model)
        }
        let c = self.client.lock().unwrap();
        if c.can_sample && self.request.is_some() {
            return Some("agent:sampling".into());
        }
        c.cli.as_ref().map(|cli| format!("agent:{}", cli.kind))
    }

    pub fn available(&self) -> bool {
        self.backend().is_some()
    }

    /// A doubtful (not risky) step: is this the right element and a sensible next step?
    pub fn check_step(&self, task: &str, action: &Value, window: &str) -> Option<Value> {
        let t = if task.trim().is_empty() { "(unknown)".to_string() } else { task.trim().to_string() };
        let prompt = format!("User's request (verbatim):\n{t}\n\nProposed action:\n{}\n\nCurrent window (accessibility tree, may be partial):\n{}",
                             trunc(&serde_json::to_string_pretty(action).unwrap_or_default(), 2000), tail(window, 4000));
        self.call(STEP_SYSTEM, &prompt)
    }

    pub fn prompt(task: &str, action: &Value) -> String {
        let t = if task.trim().is_empty() { "(unknown)".to_string() } else { task.trim().to_string() };
        format!("User's request (verbatim):\n{t}\n\nProposed action:\n{}",
                trunc(&serde_json::to_string_pretty(action).unwrap_or_default(), 3000))
    }

    /// {"decision", "reason", "ms", "via"} or None when the call failed.
    pub fn review(&self, task: &str, action: &Value) -> Option<Value> {
        self.call(SYSTEM, &Self::prompt(task, action))
    }

    fn call(&self, system: &str, prompt: &str) -> Option<Value> {
        let via = self.backend()?;
        let t0 = Instant::now();
        let txt = if via.starts_with("api:") {
            self.call_http(system, prompt)
        } else if via == "agent:sampling" {
            self.call_sampling(system, prompt)
        } else {
            let cli = self.client.lock().unwrap().cli.clone()?;
            run_cli(&cli, system, prompt, self.timeout.unwrap_or(45.0))
        }?;
        let mut out = parse(&txt)?;
        out["ms"] = json!(round_to(t0.elapsed().as_secs_f64() * 1000.0, 1));
        out["via"] = json!(via);
        Some(out)
    }

    fn call_http(&self, system: &str, prompt: &str) -> Option<String> {
        let h = self.http.as_ref()?;
        let body = json!({"model": h.model, "temperature": 0, "max_tokens": self.max_tokens,
                          "messages": [{"role": "system", "content": system},
                                       {"role": "user", "content": prompt}]});
        let local = ["127.0.0.1", "localhost", "[::1]"].iter().any(|x| h.url.contains(&format!("//{x}")));
        let mut b = ureq::AgentBuilder::new().timeout(Duration::from_secs_f64(self.timeout.unwrap_or(15.0)));
        if !local {
            if let Some(p) = env("HTTPS_PROXY").or_else(|| env("https_proxy")) {
                if let Ok(px) = ureq::Proxy::new(p) {
                    b = b.proxy(px);
                }
            }
        }
        let mut req = b.build().post(&h.url).set("Content-Type", "application/json");
        if !h.key.is_empty() {
            req = req.set("Authorization", &format!("Bearer {}", h.key));
        }
        if let Some(ua) = &h.user_agent {
            req = req.set("User-Agent", ua); // some relays only serve known clients
        }
        let d: Value = req.send_json(body).ok()?.into_json().ok()?;
        Some(d.get("choices")?.get(0)?.get("message")?.get("content")?.as_str()?.to_string())
    }

    /// MCP sampling: the client runs its own model on this one message, without the conversation.
    fn call_sampling(&self, system: &str, prompt: &str) -> Option<String> {
        let req = self.request.as_ref()?;
        let params = json!({"messages": [{"role": "user", "content": {"type": "text", "text": prompt}}],
                            "systemPrompt": system, "includeContext": "none", "temperature": 0,
                            "maxTokens": self.max_tokens});
        let r = req("sampling/createMessage", params, self.timeout.unwrap_or(30.0))?;
        let c = r.get("content")?;
        let text = if let Some(arr) = c.as_array() {
            arr.iter().filter_map(|x| x.get("text").and_then(|t| t.as_str())).collect::<Vec<_>>().join("\n")
        } else {
            c.get("text")?.as_str()?.to_string()
        };
        Some(text)
    }
}

/// The command line of the process that started us (the agent), best effort.
pub fn parent_args() -> Vec<String> {
    #[cfg(unix)]
    {
        let ppid = std::os::unix::process::parent_id();
        if let Ok(raw) = std::fs::read(format!("/proc/{ppid}/cmdline")) {
            return raw.split(|b| *b == 0).filter(|a| !a.is_empty()).map(|a| String::from_utf8_lossy(a).to_string()).collect();
        }
        if let Ok(o) = Command::new("ps").args(["-o", "args=", "-p", &ppid.to_string()]).output() {
            return String::from_utf8_lossy(&o.stdout).split_whitespace().map(String::from).collect();
        }
    }
    vec![]
}

fn base_name(p: &str) -> String {
    let b = p.rsplit(['/', '\\']).next().unwrap_or(p).to_lowercase();
    b.trim_end_matches(".exe").trim_end_matches(".cmd").trim_end_matches(".js").to_string()
}

fn flag_values(args: &[String], names: &[&str]) -> Vec<String> {
    let mut out = vec![];
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        for n in names {
            if a == n && i + 1 < args.len() {
                out.push(n.to_string());
                out.push(args[i + 1].clone());
            } else if let Some(v) = a.strip_prefix(&format!("{n}=")) {
                out.push(n.to_string());
                out.push(v.to_string());
            }
        }
        i += 1;
    }
    out
}

/// Which agent CLI is our client, and how to ask it for the same model the session runs.
pub fn detect_cli(client_name: &str, parent: &[String]) -> Option<AgentCli> {
    let cn = client_name.to_lowercase();
    let pnames: Vec<String> = parent.iter().take(3).map(|a| base_name(a)).collect();
    let is = |k: &str, aliases: &[&str]| {
        aliases.iter().any(|a| cn == *a || cn.starts_with(&format!("{a}-")) || cn.starts_with(&format!("{a} ")))
            || pnames.iter().any(|p| p == k || (k == "pi" && p == "pi-coding-agent"))
    };
    let kind = if is("pi", &["pi", "pi-coding-agent"]) {
        "pi"
    } else if is("claude", &["claude-code", "claude"]) {
        "claude"
    } else if is("codex", &["codex", "codex-mcp-client", "codex_cli_rs"]) {
        "codex"
    } else {
        return None;
    };
    // the binary: on PATH, else the path the parent was started from
    let bin = crate::install::which(kind).or_else(|| {
        parent.iter().take(3).find(|a| base_name(a) == kind && std::path::Path::new(a.as_str()).is_file()).cloned()
    })?;
    let model_args = match kind {
        "pi" => flag_values(parent, &["--provider", "--model"]),
        "claude" => flag_values(parent, &["--model"]),
        _ => {
            let mut v = flag_values(parent, &["--model"]);
            let m = flag_values(parent, &["-m"]);
            v.extend(m);
            v
        }
    };
    Some(AgentCli { kind: kind.into(), bin, model_args })
}

/// The argv for one stand-alone, tool-less, context-free answer from the agent CLI.
pub fn cli_argv(cli: &AgentCli, system: &str, prompt: &str, out_file: &str) -> Vec<String> {
    let mut a: Vec<String> = vec![cli.bin.clone()];
    let push = |a: &mut Vec<String>, xs: &[&str]| a.extend(xs.iter().map(|x| x.to_string()));
    match cli.kind.as_str() {
        "pi" => {
            push(&mut a, &["-p", "--no-session", "--no-tools", "--no-mcp", "--no-skills", "--no-prompt-templates",
                           "--no-context-files", "--no-themes", "--system-prompt", system]);
            a.extend(cli.model_args.iter().cloned());
            push(&mut a, &["--", prompt]);
        }
        "claude" => {
            push(&mut a, &["-p", "--output-format", "text", "--strict-mcp-config", "--mcp-config", "{\"mcpServers\":{}}",
                           "--max-turns", "1", "--system-prompt", system]);
            a.extend(cli.model_args.iter().cloned());
            push(&mut a, &["--", prompt]);
        }
        _ => {
            push(&mut a, &["exec", "--skip-git-repo-check", "--ephemeral", "-s", "read-only", "-c", "mcp_servers={}",
                           "--color", "never", "-o", out_file]);
            a.extend(cli.model_args.iter().cloned());
            a.push(format!("{system}\n\n{prompt}"));
        }
    }
    a
}

fn run_cli(cli: &AgentCli, system: &str, prompt: &str, timeout: f64) -> Option<String> {
    let out_file = std::env::temp_dir().join(format!("gui-gate-review-{}.txt", rand_hex(6))).to_string_lossy().to_string();
    let argv = cli_argv(cli, system, prompt, &out_file);
    let mut child = Command::new(&argv[0]).args(&argv[1..]).env("GUI_GATE_REVIEWER_CHILD", "1")
        .current_dir(std::env::temp_dir()).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });
    let t0 = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if t0.elapsed().as_secs_f64() < timeout => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(&out_file);
                return None;
            }
        }
    }
    let mut text = reader.join().unwrap_or_default();
    if let Ok(f) = std::fs::read_to_string(&out_file) {
        text = f;
    }
    let _ = std::fs::remove_file(&out_file);
    Some(text)
}

pub fn parse(txt: &str) -> Option<Value> {
    let re = Regex::new(r"(?s)\{.*\}").unwrap();
    let m = re.find(txt)?;
    let d: Value = serde_json::from_str(m.as_str()).ok()?;
    let dec = pystr(d.get("decision")).to_lowercase();
    if dec != "allow" && dec != "deny" {
        return None;
    }
    Some(json!({"decision": dec, "reason": trunc(&pystr(d.get("reason")), 300)}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_agent_and_keeps_its_model() {
        let sh = if cfg!(windows) { "cmd" } else { "sh" };
        let parent: Vec<String> = ["node", "/usr/local/bin/pi", "--no-session", "--provider", "agentrouter", "--model=deepseek-v4-flash", "-p", "x"]
            .iter().map(|s| s.to_string()).collect();
        // the binary must exist: fake it with a path that does
        let mut p2 = parent.clone();
        let fake = std::env::temp_dir().join("pi");
        std::fs::write(&fake, "").unwrap();
        p2[1] = fake.to_string_lossy().to_string();
        if crate::install::which("pi").is_none() {
            let c = detect_cli("", &p2).expect("pi from the parent command line");
            assert_eq!(c.kind, "pi");
            assert_eq!(c.model_args, vec!["--provider", "agentrouter", "--model", "deepseek-v4-flash"]);
            let a = cli_argv(&c, "SYS", "PROMPT", "/tmp/o");
            assert!(a.contains(&"--no-mcp".to_string()) && a.contains(&"--no-tools".to_string()));
            assert_eq!(a.last().unwrap(), "PROMPT");
        }
        let _ = sh;
        assert!(detect_cli("cursor-vscode", &["/opt/cursor/cursor".to_string()]).is_none());
    }

    #[test]
    fn parse_fenced() {
        assert_eq!(super::parse("```json\n{\"decision\": \"deny\", \"reason\": \"not asked\"}\n```").unwrap()["decision"], "deny");
        assert!(super::parse("maybe").is_none());
    }
}
