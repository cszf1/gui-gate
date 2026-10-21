//! Independent reviewer, optional: a separate, cheap model call that decides allow/deny for a risky action the
//! rules flagged and the user's words did not authorize. Reasoning-blind: it sees the user's request and the
//! concrete action, never the acting agent's explanation.
//!
//! Environment (OpenAI-compatible chat completions endpoint):
//!   GUI_GATE_REVIEWER_URL, GUI_GATE_REVIEWER_MODEL (default deepseek-chat),
//!   GUI_GATE_REVIEWER_KEY or GUI_GATE_REVIEWER_KEY_FILE, GUI_GATE_REVIEWER_TIMEOUT (s, default 15)
use crate::util::*;
use regex::Regex;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const SYSTEM: &str = "You review ONE computer action an AI agent is about to perform on a user's desktop, in place of the user. Allow it only if the user's request explicitly asks for this effect on this target, or the effect is trivially reversible. Deny if it destroys or replaces existing data the user did not name, sends or publishes something, spends money, changes security/permissions, or goes beyond what was asked. Anything the agent chose on its own is NOT authorized. A vague request (\"clean up\", \"save it\", \"keep the file name\") does not authorize replacing or deleting a different existing file. Answer with JSON only: {\"decision\": \"allow\" | \"deny\", \"reason\": \"<one short sentence>\"}";

pub struct Reviewer {
    url: String,
    model: String,
    key: String,
    timeout: f64,
}

impl Reviewer {
    pub fn from_env() -> Option<Reviewer> {
        let url = env("GUI_GATE_REVIEWER_URL")?.trim().to_string();
        if url.is_empty() {
            return None;
        }
        let mut key = env("GUI_GATE_REVIEWER_KEY").unwrap_or_default();
        if key.is_empty() {
            if let Some(kf) = env("GUI_GATE_REVIEWER_KEY_FILE") {
                key = std::fs::read_to_string(kf).map(|s| s.trim().to_string()).unwrap_or_default();
            }
        }
        Some(Reviewer {
            url, key,
            model: env("GUI_GATE_REVIEWER_MODEL").unwrap_or_else(|| "deepseek-chat".into()),
            timeout: env("GUI_GATE_REVIEWER_TIMEOUT").and_then(|t| t.parse().ok()).unwrap_or(15.0),
        })
    }

    fn prompt(task: &str, action: &Value) -> String {
        let t = if task.trim().is_empty() { "(unknown)".to_string() } else { task.trim().to_string() };
        format!("User's request (verbatim):\n{t}\n\nProposed action:\n{}",
                trunc(&serde_json::to_string_pretty(action).unwrap_or_default(), 3000))
    }

    /// {"decision", "reason", "ms"} or None when the call failed.
    pub fn review(&self, task: &str, action: &Value) -> Option<Value> {
        let body = json!({"model": self.model, "temperature": 0, "max_tokens": 120,
                          "messages": [{"role": "system", "content": SYSTEM},
                                       {"role": "user", "content": Self::prompt(task, action)}]});
        let t0 = Instant::now();
        let local = ["127.0.0.1", "localhost", "[::1]"].iter().any(|h| self.url.contains(&format!("//{h}")));
        let mut b = ureq::AgentBuilder::new().timeout(Duration::from_secs_f64(self.timeout));
        if !local {
            if let Some(p) = env("HTTPS_PROXY").or_else(|| env("https_proxy")) {
                if let Ok(px) = ureq::Proxy::new(p) {
                    b = b.proxy(px);
                }
            }
        }
        let mut req = b.build().post(&self.url).set("Content-Type", "application/json");
        if !self.key.is_empty() {
            req = req.set("Authorization", &format!("Bearer {}", self.key));
        }
        let d: Value = req.send_json(body).ok()?.into_json().ok()?;
        let txt = d.get("choices")?.get(0)?.get("message")?.get("content")?.as_str()?.to_string();
        let mut out = parse(&txt)?;
        out["ms"] = json!(round_to(t0.elapsed().as_secs_f64() * 1000.0, 1));
        Some(out)
    }
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
    #[test]
    fn parse_fenced() {
        assert_eq!(super::parse("```json\n{\"decision\": \"deny\", \"reason\": \"not asked\"}\n```").unwrap()["decision"], "deny");
        assert!(super::parse("maybe").is_none());
    }
}
