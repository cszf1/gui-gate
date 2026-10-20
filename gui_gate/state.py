"""What the gate knows at runtime: every observation and action that passed
through it. Nothing here uses ground truth that an agent could not have.
"""
from __future__ import annotations

import re
import time
from dataclasses import dataclass, field
from typing import Optional

# "- [3] push button "Save" [actions=[click]]"  or  "[3] button "OK""
_ROW = re.compile(r'\[(\d+)\]\s+([^"\[]+?)\s+"((?:[^"\\]|\\.)*)"')
_ROW_NOLABEL = re.compile(r'\[(\d+)\]\s+([a-z][a-z \-]*?)(?:\s+\[|\s*$)')
_LOADING = re.compile(r"(progress bar|busy|loading|spinner|加载|正在|请稍候|please wait)", re.I)


@dataclass
class Element:
    index: int
    role: str
    label: str
    frame: Optional[dict] = None  # {x,y,w,h} when the read carried it
    enabled: Optional[bool] = None


@dataclass
class Observation:
    snapshot_id: str
    pid: Optional[int]
    window_id: Optional[int]
    t: float
    with_screenshot: bool
    elements: dict[int, Element] = field(default_factory=dict)
    window_bounds: Optional[dict] = None
    shot_w: Optional[int] = None
    shot_h: Optional[int] = None
    loading_cue: bool = False
    truncated: bool = False
    window_count: Optional[int] = None  # windows of this pid at read time
    text: str = ""  # the read's text (tree incl. unindexed labels such as a dialog's message)


@dataclass
class ActionRecord:
    t: float
    tool: str
    key: str  # normalized signature for repeat detection
    window_key: Optional[str]
    is_error: bool = False
    unverifiable: bool = False
    popup: bool = False
    stale_error: bool = False


def parse_elements(sc: dict) -> dict[int, Element]:
    els: dict[int, Element] = {}
    for e in sc.get("elements") or []:
        idx = e.get("element_index")
        if idx is None:
            continue
        els[int(idx)] = Element(int(idx), str(e.get("role", "")), str(e.get("label") or ""),
                                e.get("frame"), e.get("enabled"))
    md = sc.get("tree_markdown") or ""
    for line in md.splitlines():
        m = _ROW.search(line)
        if m:
            idx = int(m.group(1))
            if idx not in els:
                els[idx] = Element(idx, m.group(2).strip(), m.group(3))
            continue
        m = _ROW_NOLABEL.search(line)
        if m and int(m.group(1)) not in els:
            els[int(m.group(1))] = Element(int(m.group(1)), m.group(2).strip(), "")
    return els


class GateState:
    def __init__(self) -> None:
        self.obs_by_snapshot: dict[str, Observation] = {}
        self.latest_by_window: dict[str, Observation] = {}
        self.actions: list[ActionRecord] = []
        self.window_of_pid: dict[int, int] = {}
        self.last_typed: dict[int, str] = {}  # pid -> text typed since the last Enter

    @staticmethod
    def wkey(pid, window_id) -> Optional[str]:
        if pid is None and window_id is None:
            return None
        return f"{pid}:{window_id}"

    # ---- observations -------------------------------------------------
    def record_observation(self, args: dict, result: dict, now: Optional[float] = None) -> Optional[Observation]:
        sc = (result or {}).get("structuredContent") or {}
        snap = sc.get("snapshot_id")
        pid = sc.get("pid", args.get("pid"))
        wid = sc.get("window_id", args.get("window_id"))
        with_shot = any(c.get("type") == "image" for c in (result or {}).get("content", [])) or bool(sc.get("capture_id"))
        if not snap:
            if not with_shot or pid is None:
                return None
            # a screenshot-only read (no tree): still a fresh look at the window for pixel actions;
            # earlier rows and tokens stay as they were
            prev = self.latest_by_window.get(self.wkey(pid, wid))
            self._shots = getattr(self, "_shots", 0) + 1
            o = Observation(snapshot_id=(prev.snapshot_id if prev else f"shot{self._shots}"), pid=pid, window_id=wid,
                            t=now or time.time(), with_screenshot=True,
                            elements=dict(prev.elements) if prev else {}, window_bounds=sc.get("window_bounds")
                            or (prev.window_bounds if prev else None),
                            shot_w=sc.get("screenshot_width"), shot_h=sc.get("screenshot_height"),
                            text=prev.text if prev else "")
            self.latest_by_window[self.wkey(pid, wid)] = o
            if prev is None:
                self.obs_by_snapshot[o.snapshot_id] = o
            if wid is not None:
                self.window_of_pid[int(pid)] = int(wid)
            return o
        els = parse_elements(sc)
        prev = self.latest_by_window.get(self.wkey(pid, wid))
        md = str(sc.get("tree_markdown") or "")
        alltext = md + "\n" + "\n".join(c.get("text", "") for c in result.get("content", []) if c.get("type") == "text")
        diff_read = bool(args.get("since")) and prev is not None and "full read follows" not in alltext \
            and "no change since" not in alltext
        if args.get("since") and prev is not None and "no change since" in alltext:
            els = dict(prev.elements)
        elif diff_read:
            # diff read: rows not listed keep their index; listed rows override
            merged = dict(prev.elements)
            merged.update(els)
            els = merged
        o = Observation(
            snapshot_id=snap, pid=pid, window_id=wid, t=now or time.time(), with_screenshot=with_shot,
            elements=els, window_bounds=sc.get("window_bounds"),
            shot_w=sc.get("screenshot_width"), shot_h=sc.get("screenshot_height"),
            loading_cue=bool(_LOADING.search(sc.get("tree_markdown") or "")) or any(
                _LOADING.search(e.role + " " + e.label) for e in els.values()),
            truncated=bool(sc.get("truncated")),
            text=(prev.text if (prev is not None and "no change since" in alltext) else alltext)[:12000],
        )
        self.obs_by_snapshot[snap] = o
        self.latest_by_window[self.wkey(pid, wid)] = o
        if pid is not None and wid is not None:
            self.window_of_pid[int(pid)] = int(wid)
        return o

    def latest_for(self, pid=None, window_id=None) -> Optional[Observation]:
        if window_id is None and pid is not None:
            window_id = self.window_of_pid.get(int(pid))
        o = self.latest_by_window.get(self.wkey(pid, window_id))
        if o:
            return o
        # fall back to any window of that pid
        for k, v in self.latest_by_window.items():
            if pid is not None and k.startswith(f"{pid}:"):
                return v
        return None

    # ---- actions ------------------------------------------------------
    def record_action(self, rec: ActionRecord) -> None:
        self.actions.append(rec)
        if len(self.actions) > 500:
            self.actions = self.actions[-500:]

    def actions_since(self, t: float, window_key: Optional[str] = None) -> int:
        return sum(1 for a in self.actions if a.t > t and (window_key is None or a.window_key in (None, window_key)))

    def last_action(self) -> Optional[ActionRecord]:
        return self.actions[-1] if self.actions else None

    def repeat_count(self, key: str, since: float = 0.0) -> int:
        """Consecutive identical actions with no observation in between (an agent that
        looked again and still chose the same action is not looping)."""
        n = 0
        for a in reversed(self.actions):
            if a.t <= since:
                break
            if a.key == key:
                n += 1
            else:
                break
        return n
