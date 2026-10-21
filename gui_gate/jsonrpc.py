"""Minimal newline-delimited JSON-RPC plumbing for MCP over stdio.

No third-party dependency: MCP's stdio transport is one JSON object per line.
"""
from __future__ import annotations

import itertools
import json
import os
import queue
import shutil
import subprocess
import sys
import threading
from typing import Any, Callable, Optional


class StdioWriter:
    """Thread-safe line writer."""

    def __init__(self, stream):
        self._s = stream
        self._lock = threading.Lock()

    def send(self, msg: dict) -> None:
        line = json.dumps(msg, ensure_ascii=False, separators=(",", ":"))
        with self._lock:
            self._s.write(line + "\n")
            self._s.flush()


def find_driver() -> Optional[str]:
    """Locate the cua-driver executable (GUI_GATE_DRIVER, PATH, then the installer's default places)."""
    from .install import find_driver as _fd
    return _fd()


class ChildMCP:
    """Runs `cua-driver mcp` (or any MCP server) as a child and talks to it.

    * call(): request/response for the gate's own internal calls (ids "gate-N").
    * forward(): send an agent message untouched; its response is delivered to
      `on_unsolicited` like every message the gate did not originate.
    """

    def __init__(self, cmd: list[str], on_unsolicited: Callable[[dict], None],
                 stderr=None):
        env = os.environ.copy()
        if "site-packages" in (cmd[0] if cmd else "") or "dist-packages" in (cmd[0] if cmd else ""):
            # same as the cua-driver package's own launcher: tell the binary it came from the Python package
            env.setdefault("CUA_DRIVER_INSTALL_CHANNEL", "python_package")
        self.p = subprocess.Popen(
            cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=stderr if stderr is not None else subprocess.DEVNULL,
            text=True, bufsize=1, env=env)
        self._w = StdioWriter(self.p.stdin)
        self._ids = itertools.count(1)
        self._pending: dict[str, queue.Queue] = {}
        self._lock = threading.Lock()
        self._on_unsolicited = on_unsolicited
        self._t = threading.Thread(target=self._reader, daemon=True)
        self._t.start()

    def _reader(self) -> None:
        for line in self.p.stdout:
            line = line.strip()
            if not line:
                continue
            try:
                msg = json.loads(line)
            except json.JSONDecodeError:
                continue
            mid = msg.get("id")
            if isinstance(mid, str) and mid.startswith("gate-") and "method" not in msg:
                with self._lock:
                    q = self._pending.pop(mid, None)
                if q is not None:
                    q.put(msg)
                    continue
            self._on_unsolicited(msg)
        # child exited: fail every pending call
        with self._lock:
            pend, self._pending = self._pending, {}
        for q in pend.values():
            q.put({"error": {"code": -32000, "message": "driver exited"}})

    def call(self, method: str, params: dict | None = None, timeout: float = 60.0) -> dict:
        mid = f"gate-{next(self._ids)}"
        q: queue.Queue = queue.Queue(maxsize=1)
        with self._lock:
            self._pending[mid] = q
        msg = {"jsonrpc": "2.0", "id": mid, "method": method}
        if params is not None:
            msg["params"] = params
        self._w.send(msg)
        try:
            return q.get(timeout=timeout)
        except queue.Empty:
            with self._lock:
                self._pending.pop(mid, None)
            return {"error": {"code": -32001, "message": f"timeout calling {method}"}}

    def tool(self, name: str, args: dict, timeout: float = 60.0) -> dict:
        """tools/call helper returning the `result` object (or an error-shaped result)."""
        r = self.call("tools/call", {"name": name, "arguments": args}, timeout)
        if "result" in r:
            return r["result"]
        err = r.get("error", {})
        return {"isError": True, "content": [{"type": "text", "text": err.get("message", "error")}]}

    def forward(self, msg: dict) -> None:
        self._w.send(msg)

    def close(self) -> None:
        try:
            self.p.stdin.close()
        except Exception:
            pass
        try:
            self.p.wait(timeout=3)
        except Exception:
            self.p.kill()


def text_of(result: dict) -> str:
    return "\n".join(c.get("text", "") for c in (result or {}).get("content", [])
                     if c.get("type") == "text")


def log_err(*a) -> None:
    print("[gui-gate]", *a, file=sys.stderr, flush=True)
