"""Hand a decision to a person.

Channels, tried in this order when channel="auto":
  elicit   MCP elicitation/create to the client (clients that declare the
           `elicitation` capability show a native prompt to their user)
  desktop  a small always-on-top GTK dialog on the machine's own display
           (whoever watches the desktop answers it)
  chat     nothing the gate can show itself: the refusal tells the agent to ask
           its user and come back with a one-time confirm code

ask() returns {"answer": "allow"|"deny"|"timeout"|<text>, "channel": ..., "ms": ...}.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from typing import Callable, Optional

_DIALOG = r'''
import json, sys
import gi
gi.require_version("Gtk", "3.0")
from gi.repository import Gtk, GLib
q = json.loads(sys.argv[1])
d = Gtk.Dialog(title=q.get("title", "gui-gate: a person is needed"))
d.set_keep_above(True); d.set_modal(True); d.set_default_size(460, 160)
box = d.get_content_area(); box.set_spacing(8); box.set_border_width(12)
lab = Gtk.Label(label=q["text"]); lab.set_line_wrap(True); lab.set_xalign(0); box.add(lab)
entry = None
if q.get("want_text"):
    entry = Gtk.Entry(); box.add(entry)
    d.add_button("Send", 1); d.add_button("Cancel", 2)
else:
    d.add_button("Allow", 1); d.add_button("Deny", 2)
GLib.timeout_add_seconds(int(q.get("timeout", 120)), lambda: (d.response(3), False)[1])
d.show_all(); d.present()
r = d.run()
out = {1: "allow", 2: "deny", 3: "timeout"}.get(r, "deny")
if entry is not None and r == 1:
    out = entry.get_text() or "allow"
print(json.dumps({"answer": out}))
'''


class Human:
    def __init__(self, channel: str = "auto", timeout: float = 120.0,
                 elicit: Optional[Callable[[dict, float], Optional[dict]]] = None,
                 client_can_elicit: Callable[[], bool] = lambda: False):
        self.channel = channel
        self.timeout = timeout
        self._elicit = elicit
        self._can_elicit = client_can_elicit
        self.asked = 0
        self.answered = 0

    def resolve_channel(self) -> str:
        if self.channel != "auto":
            return self.channel
        if self._elicit and self._can_elicit():
            return "elicit"
        if os.environ.get("DISPLAY") and _has_gtk():
            return "desktop"
        return "chat"

    def ask(self, text: str, want_text: bool = False, title: str = "") -> dict:
        ch = self.resolve_channel()
        t0 = time.time()
        self.asked += 1
        ans = "timeout"
        if ch == "elicit" and self._elicit:
            schema = ({"type": "object", "properties": {"answer": {"type": "string", "title": "Your answer"}},
                       "required": ["answer"]} if want_text else
                      {"type": "object", "properties": {"allow": {"type": "boolean", "title": "Allow this action?"}},
                       "required": ["allow"]})
            r = self._elicit({"message": text, "requestedSchema": schema}, self.timeout)
            if r is None:
                ans = "timeout"
            elif r.get("action") != "accept":
                ans = "deny"
            else:
                c = r.get("content") or {}
                ans = str(c.get("answer") or "allow") if want_text else ("allow" if c.get("allow") else "deny")
        elif ch == "desktop":
            q = {"text": text, "want_text": want_text, "timeout": self.timeout,
                 "title": title or "gui-gate: a person is needed"}
            try:
                p = subprocess.run([sys.executable, "-c", _DIALOG, json.dumps(q)], capture_output=True, text=True,
                                   timeout=self.timeout + 10)
                ans = json.loads(p.stdout.strip().splitlines()[-1]).get("answer", "deny")
            except Exception:
                ans = "timeout"
        else:
            return {"answer": "chat", "channel": "chat", "ms": 0.0}
        if ans not in ("timeout",):
            self.answered += 1
        return {"answer": ans, "channel": ch, "ms": round((time.time() - t0) * 1000, 1)}


_GTK = None


def _has_gtk() -> bool:
    global _GTK
    if _GTK is None:
        try:
            r = subprocess.run([sys.executable, "-c", "import gi; gi.require_version('Gtk','3.0'); "
                                "from gi.repository import Gtk"], capture_output=True, timeout=10)
            _GTK = r.returncode == 0
        except Exception:
            _GTK = False
    return _GTK
