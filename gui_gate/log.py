"""Append-only JSONL event log (one file per session)."""
from __future__ import annotations

import json
import os
import threading
import time
import uuid
from typing import Optional


class EventLog:
    def __init__(self, path: Optional[str]):
        self.session = uuid.uuid4().hex[:8]
        if path == "auto":
            d = os.path.expanduser(os.environ.get("GUI_GATE_LOG_DIR", "~/.gui-gate/logs"))
            os.makedirs(d, exist_ok=True)
            path = os.path.join(d, time.strftime("%Y%m%d-%H%M%S") + f"-{self.session}.jsonl")
        self.path = path
        self._f = open(path, "a", encoding="utf-8") if path else None
        self._lock = threading.Lock()

    def write(self, ev: dict) -> None:
        if not self._f:
            return
        ev = {"ts": round(time.time(), 3), "session": self.session, **ev}
        with self._lock:
            self._f.write(json.dumps(ev, ensure_ascii=False, default=str) + "\n")
            self._f.flush()

    def close(self) -> None:
        if self._f:
            self._f.close()
