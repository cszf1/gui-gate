"""Independent reviewer (design-v0.4 T3), optional.

A separate, cheap model call that decides allow/deny for a risky action the rules flagged and the user's
words did not authorize. Like Claude Code's auto-mode classifier and Codex auto-review it is
reasoning-blind: it sees the user's request and the concrete action (tool, target role/label, the dialog
text, the rule that fired) but never the acting agent's own explanation, so the agent cannot talk it round.

Configured by environment (OpenAI-compatible chat completions endpoint, zero dependencies):
  GUI_GATE_REVIEWER_URL    e.g. https://api.deepseek.com/v1/chat/completions
  GUI_GATE_REVIEWER_MODEL  e.g. deepseek-chat
  GUI_GATE_REVIEWER_KEY    bearer token (or GUI_GATE_REVIEWER_KEY_FILE)
Without a URL the reviewer is off and the gate falls back to its previous behaviour.
"""
from __future__ import annotations

import json
import os
import re
import time
import urllib.parse
import urllib.request
from typing import Optional

SYSTEM = (
    "You review ONE computer action an AI agent is about to perform on a user's desktop, in place of the user. "
    "Allow it only if the user's request explicitly asks for this effect on this target, or the effect is "
    "trivially reversible. Deny if it destroys or replaces existing data the user did not name, sends or "
    "publishes something, spends money, changes security/permissions, or goes beyond what was asked. Anything "
    "the agent chose on its own is NOT authorized. A vague request (\"clean up\", \"save it\", \"keep the file "
    "name\") does not authorize replacing or deleting a different existing file. Answer with JSON only: "
    "{\"decision\": \"allow\" | \"deny\", \"reason\": \"<one short sentence>\"}"
)


class Reviewer:
    def __init__(self, url: str, model: str, key: str = "", timeout: float = 15.0):
        self.url, self.model, self.key, self.timeout = url, model, key, timeout

    @classmethod
    def from_env(cls) -> Optional["Reviewer"]:
        url = os.environ.get("GUI_GATE_REVIEWER_URL", "").strip()
        if not url:
            return None
        key = os.environ.get("GUI_GATE_REVIEWER_KEY", "")
        kf = os.environ.get("GUI_GATE_REVIEWER_KEY_FILE")
        if not key and kf and os.path.exists(kf):
            key = open(kf).read().strip()
        return cls(url, os.environ.get("GUI_GATE_REVIEWER_MODEL", "deepseek-chat"), key,
                   float(os.environ.get("GUI_GATE_REVIEWER_TIMEOUT", "15")))

    @staticmethod
    def prompt(task: str, action: dict) -> str:
        return ("User's request (verbatim):\n" + (task.strip() or "(unknown)") + "\n\nProposed action:\n"
                + json.dumps(action, ensure_ascii=False, indent=1)[:3000])

    def review(self, task: str, action: dict) -> Optional[dict]:
        """{"decision": allow|deny, "reason": str, "ms": float} or None when the call failed."""
        body = {"model": self.model, "temperature": 0, "max_tokens": 120,
                "messages": [{"role": "system", "content": SYSTEM},
                             {"role": "user", "content": self.prompt(task, action)}]}
        req = urllib.request.Request(self.url, data=json.dumps(body).encode(), method="POST",
                                     headers={"Content-Type": "application/json",
                                              **({"Authorization": f"Bearer {self.key}"} if self.key else {})})
        t0 = time.time()
        try:
            host = urllib.parse.urlparse(self.url).hostname or ""
            opener = (urllib.request.build_opener(urllib.request.ProxyHandler({}))  # never proxy a local endpoint
                      if host in ("127.0.0.1", "localhost", "::1") else urllib.request.build_opener())
            with opener.open(req, timeout=self.timeout) as r:
                d = json.loads(r.read().decode())
            txt = d["choices"][0]["message"]["content"]
        except Exception:
            return None
        out = parse(txt)
        if out is not None:
            out["ms"] = round((time.time() - t0) * 1000, 1)
        return out


def parse(txt: str) -> Optional[dict]:
    m = re.search(r"\{.*\}", txt or "", re.S)
    if not m:
        return None
    try:
        d = json.loads(m.group(0))
    except json.JSONDecodeError:
        return None
    dec = str(d.get("decision", "")).lower()
    if dec not in ("allow", "deny"):
        return None
    return {"decision": dec, "reason": str(d.get("reason", ""))[:300]}
