"""Server-side batching ("codemode inside the gate"), so every MCP client gets it, not only Pi.

run_actions takes a small JSON program instead of JavaScript: action steps plus four control steps
(find / for_each / if / read). Same idea as Pi's codemode (several tool calls per model turn, only a
compact result comes back) without a JS engine, so it needs nothing beyond the Python standard library
and cannot run arbitrary code: it can only call the GUI tools, each through the gate.

Gate semantics inside a batch (see docs/design-codemode-in-gate.md):
  * every GUI step goes through Gate.gated() exactly like a single call; a refusal stops the batch;
  * no auto-observe between steps, one observation at the end (or where the batch stopped);
  * blind-batch guard: stop on a driver error, an unexpected new window/dialog, a failed `expect`, or a
    pixel (x/y) step after the first executed step (coordinates from before the batch are stale by then);
  * at most MAX_STEPS GUI actions per batch.
"""
from __future__ import annotations

import re
import time
from typing import Optional

from .features import GATED_TOOLS
from .jsonrpc import text_of

MAX_STEPS = 20      # GUI actions per batch
MAX_NODES = 200     # interpreted steps per batch (loops included)
MAX_DEPTH = 3       # nesting of for_each / if
NOT_IN_BATCH = {"run_steps", "run_script", "kill_app"}

_SEL = {"type": "object", "description": "Which elements: {label} exact, {label_contains}, {label_regex}, "
         "and/or {role}; searched in windows opened during the batch first, then the batch's window.",
         "properties": {"label": {"type": "string"}, "label_contains": {"type": "string"},
                        "label_regex": {"type": "string"}, "role": {"type": "string"}}}

RUN_ACTIONS_TOOL = {
    "name": "run_actions",
    "description": (
        "Batch mode: run several GUI steps in ONE call (single-step tools remain available). Each GUI step is "
        "checked by the gate like a single call. Steps, in order:\n"
        "  {tool, args, expect?}  a GUI action (click, type_text, press_key, hotkey, scroll, set_value, ...). "
        "Name things that appear only after an earlier step with args.target = {label, role?}; the gate finds "
        "them on screen when the step runs. x/y only in the first GUI step.\n"
        "  {find: SELECTOR, as: \"rows\", max?: 20}  read the screen now and keep the matching elements.\n"
        "  {for_each: \"rows\", do: [steps]}  repeat steps per element; inside, args.target = \"$item\" points at it "
        "and \"$item.label\" in any string is its label.\n"
        "  {if: {exists: SELECTOR} | {not_exists: SELECTOR}, then: [steps], else?: [steps]}\n"
        "  {read: SELECTOR, as?: \"name\"}  report the matching elements' labels/values in the result.\n"
        "The batch stops at the first refused or failed step, when a dialog opens unexpectedly (a popup menu is "
        "fine), or when an expect fails ({title_contains} or {new_window: true}). You get what ran plus one fresh "
        "read of the window at the end. Max 20 GUI actions."),
    "inputSchema": {"type": "object", "properties": {
        "pid": {"type": "integer", "description": "Default pid for steps that omit it."},
        "window_id": {"type": "integer", "description": "Default window_id for steps that omit it."},
        "steps": {"type": "array", "description": "The program, see the tool description.",
                  "items": {"type": "object"}},
    }, "required": ["steps"]},
}


def _titles(gate, pid) -> list[str]:
    try:
        r = gate._tool("list_windows", {"pid": int(pid)}, timeout=10)
    except Exception:
        return []
    ws = (r.get("structuredContent") or {}).get("windows") or []
    return [str(w.get("title") or w.get("name") or "") for w in ws]


def _ids(sig: Optional[str]) -> set:
    return {x.split("@")[0] for x in (sig or "").split(";") if x}


_MENU_ROLES = {"menu", "popup menu", "menu item", "check menu item", "radio menu item", "menu bar", "list",
               "list item", "combo box", "tear off menu item", "separator"}


def _norm_label(s: str) -> str:
    s = (s or "").replace("_", "").replace("&", "").replace("\u2026", "").replace("...", "")
    return re.sub(r"\s+", " ", s).strip().lower()


def _menu_like(obs) -> bool:
    """A popup menu / dropdown list rather than a dialog: every actionable element is a menu or list entry."""
    roles = [e.role for e in obs.elements.values()] if obs is not None else []
    return bool(roles) and all(r in _MENU_ROLES for r in roles)


class Batch:
    """One batch: counts GUI actions, enforces the guards, remembers where to observe at the end."""

    def __init__(self, gate, default_pid=None, default_wid=None, stop_on_new_window=True):
        self.g = gate
        self.pid, self.wid = default_pid, default_wid
        self.stop_on_new_window = stop_on_new_window
        self.executed = 0
        self.lines: list[str] = []
        self.stopped: Optional[str] = None
        self.last_pid, self.last_wid = default_pid, default_wid
        self.seen: Optional[set] = None
        self.bid = f"b{int(time.time() * 1000) % 10_000_000}"
        self.menu_open = False  # the last step opened a popup menu/list (not a surprise)

    def _read(self, pid, wid):
        a = {"pid": int(pid), "include_screenshot": False, "tree_format": "elements", "max_elements": 800}
        if wid:
            a["window_id"] = int(wid)
        r = self.g._tool("get_window_state", a, timeout=20)
        o = self.g.state.record_observation(a, r)
        self.g.stats["tree_reads"] += 1
        if o is not None:
            o.gate_read = True
            o.window_count = self.g._window_sig(o.pid) if o.pid is not None else None
        return o

    def _windows(self, pid, base):
        """Windows to search: ones opened during the batch (popups, menus) first, then the batch's own, then the rest."""
        wins = []
        try:
            r = self.g._tool("list_windows", {"pid": int(pid)}, timeout=10)
            ws = [w for w in (r.get("structuredContent") or {}).get("windows") or [] if w.get("is_on_screen", True)]
            ws.sort(key=lambda w: -(w.get("z_index") or 0))
            wins = [w.get("window_id") for w in ws]
        except Exception:
            pass
        fresh = [w for w in wins if str(w) not in (self.seen or set()) and w != base]
        return (fresh + ([base] if base else []) + [w for w in wins if w not in fresh and w != base])[:4]

    def select(self, pid, base, sel: dict, first_window_only: bool = True):
        """Elements matching a selector, read fresh now. Returns (obs, [elements], seen_labels)."""
        if not isinstance(sel, dict) or not any(k in sel for k in ("label", "label_contains", "label_regex", "role")):
            raise ValueError("a selector needs label, label_contains, label_regex or role")
        want = _norm_label(str(sel["label"])) if sel.get("label") else None
        sub = str(sel.get("label_contains") or "").lower()
        rx = re.compile(str(sel["label_regex"]), re.I) if sel.get("label_regex") else None
        role = str(sel.get("role") or "").lower()
        seen = []
        for wid in self._windows(pid, base):
            o = self._read(pid, wid)
            if o is None:
                continue
            els = sorted(o.elements.values(), key=lambda e: e.index)
            seen += [e.label for e in els if e.label][:12]
            hits = [e for e in els
                    if (want is None or _norm_label(e.label) == want)
                    and (not sub or sub in (e.label or "").lower())
                    and (rx is None or rx.search(e.label or ""))
                    and (not role or role in (e.role or "").lower())]
            if hits:
                return o, hits, seen
        return None, [], seen

    def resolve(self, a: dict) -> Optional[str]:
        """`target: {label, role?}` -> a fresh element_token, read at the moment the step runs (so steps can name
        things that only appear after earlier steps, such as menu items). Returns an error string or None."""
        tgt = a.pop("target")
        if isinstance(tgt, dict) and tgt.get("_token"):  # an element from find / for_each
            a["element_token"] = tgt["_token"]
            if tgt.get("_wid"):
                a["window_id"] = tgt["_wid"]
            a.pop("x", None), a.pop("y", None)
            return None
        if not isinstance(tgt, dict) or not tgt.get("label"):
            return "target needs {\"label\": ..., \"role\": optional}"
        if a.get("pid") is None:
            return "target needs a pid"
        try:
            o, hits, seen = self.select(a["pid"], a.get("window_id"), {"label": tgt["label"], "role": tgt.get("role")})
        except (ValueError, re.error) as e:
            return str(e)
        if len(hits) == 1:
            a["element_token"] = f"{o.snapshot_id}:{hits[0].index}"
            a["window_id"] = o.window_id or a.get("window_id")
            a.pop("x", None), a.pop("y", None)
            return None
        if len(hits) > 1:
            return f"\"{tgt['label']}\" matches {len(hits)} elements; add a role or use an element_token"
        return f"no element named \"{tgt['label']}\" is on screen (seen: {', '.join(seen[:10])})"

    def fill(self, args: dict) -> dict:
        a = dict(args or {})
        if self.pid is not None and "pid" not in a:
            a["pid"] = self.pid
        if self.wid is not None and "window_id" not in a and "element_token" not in a:
            a["window_id"] = self.wid
        return a

    def step(self, tool: str, args: dict, expect: Optional[dict] = None, last: bool = False) -> dict:
        """Run one GUI step through the gate. Returns the tool result; sets self.stopped when the batch must end."""
        g = self.g
        n = self.executed + 1
        if tool not in GATED_TOOLS or tool in NOT_IN_BATCH:
            self.stopped = f"step {n}: `{tool}` cannot be used inside a batch"
            return {"isError": True, "content": [{"type": "text", "text": self.stopped}]}
        if self.executed >= MAX_STEPS:
            self.stopped = f"the batch limit of {MAX_STEPS} GUI actions was reached"
            return {"isError": True, "content": [{"type": "text", "text": self.stopped}]}
        a = self.fill(args)
        if self.seen is None and a.get("pid") is not None:
            self.seen = _ids(g._window_sig(a["pid"]))  # windows that were there before the batch
        if "target" in a:
            err = self.resolve(a)
            if err:
                self.stopped = f"step {n}: {err}"
                return {"isError": True, "content": [{"type": "text", "text": self.stopped}]}
        elif self.menu_open and "element_token" in a:
            self.stopped = (f"step {n}: a menu opened in the previous step, and an element_token from before the "
                            f"batch cannot point into it; name the item with target:{{label}}")
            return {"isError": True, "content": [{"type": "text", "text": self.stopped}]}
        if self.executed > 0 and "x" in a and "y" in a and "element_token" not in a:
            self.stopped = (f"step {n}: pixel x/y after the batch already changed the screen; those coordinates "
                            f"are stale. Re-read the window and aim again")
            return {"isError": True, "content": [{"type": "text", "text": self.stopped}]}
        pid = a.get("pid")
        before = g._window_sig(pid) if (pid is not None and self.stop_on_new_window) else None
        g._batch_ctx = {"bid": self.bid, "step": n}
        try:
            res = g.gated(tool, a)
        finally:
            g._batch_ctx = None
        sc = res.get("structuredContent") or {}
        refused = bool((sc.get("gate") or {}).get("executed") is False)
        first = (text_of(res).strip().splitlines() or [""])[0][:200]
        if res.get("isError"):
            self.lines.append(f"{n}. {tool}: {'REFUSED' if refused else 'FAILED'} - {first}")
            self.stopped = f"step {n} ({tool}) was {'refused by the gate' if refused else 'not completed'}"
            return res
        self.executed += 1
        self.last_pid = pid if pid is not None else self.last_pid
        if a.get("window_id"):
            self.last_wid = a.get("window_id")
        self.lines.append(f"{n}. {tool}: ok - {first}")
        expect = expect or {}
        if pid is not None and before is not None:
            after = g._window_sig(pid)
            new = _ids(after) - _ids(before)
            self.menu_open = False
            if new and not expect.get("new_window") and not last:
                nw = sorted(new)[-1]
                o = self._read(pid, int(nw)) if nw.isdigit() else None
                if _menu_like(o):  # a menu or dropdown opened: expected, the next step names its item
                    self.menu_open = True
                    return res
                self.stopped = (f"step {n} opened a new window/dialog (window_id={sorted(new)[-1]}) that the batch "
                                f"did not expect; read it before going on")
                self.last_wid = int(sorted(new)[-1]) if sorted(new)[-1].isdigit() else self.last_wid
                return res
            if expect.get("new_window") and not new:
                self.stopped = f"step {n} was expected to open a window/dialog, but none appeared"
                return res
        tc = expect.get("title_contains")
        if tc and pid is not None:
            titles = _titles(g, pid)
            if not any(str(tc).lower() in t.lower() for t in titles):
                self.stopped = f"after step {n} no window title contains \"{tc}\" (titles: {titles[:4]})"
                return res
        return res

    def finish(self, out: dict) -> dict:
        g = self.g
        if self.executed and self.last_pid is not None:
            g._auto_observe(self.last_pid, self.last_wid, out, force=True)
        g.stats["batches"] = g.stats.get("batches", 0) + 1
        g.stats["batch_actions"] = g.stats.get("batch_actions", 0) + self.executed
        g.log.write({"ev": "batch", "bid": self.bid, "executed": self.executed, "stopped": self.stopped,
                     "lines": self.lines[-MAX_STEPS:]})
        return out


def _line_of(o, idx: int) -> str:
    m = re.search(r"\[" + str(idx) + r"\] ([^\n]{0,200})", getattr(o, "text", "") or "")
    return m.group(1).strip() if m else ""


def _subst(v, item: Optional[dict]):
    """$item -> the element (as a target), "$item.label" inside strings -> its label."""
    if item is None:
        return v
    if v == "$item":
        return dict(item)
    if isinstance(v, str):
        return v.replace("$item.label", item.get("label", ""))
    if isinstance(v, dict):
        return {k: _subst(x, item) for k, x in v.items()}
    if isinstance(v, list):
        return [_subst(x, item) for x in v]
    return v


class Program:
    """Interprets the run_actions JSON program on top of a Batch."""

    def __init__(self, b: Batch):
        self.b = b
        self.vars: dict[str, list] = {}
        self.nodes = 0
        self.out: list[str] = []

    def _stop(self, why: str):
        self.b.stopped = why

    def _sel(self, sel, item):
        b = self.b
        pid = b.pid if b.pid is not None else b.last_pid
        if pid is None:
            raise ValueError("find/if/read need a pid (give one at the top level)")
        if b.seen is None:
            b.seen = _ids(b.g._window_sig(pid))
        return b.select(pid, b.last_wid or b.wid, _subst(sel, item))

    def run(self, steps, depth: int = 0, item: Optional[dict] = None, top: bool = False):
        b = self.b
        if depth > MAX_DEPTH:
            return self._stop(f"nesting deeper than {MAX_DEPTH}")
        for i, s in enumerate(steps or []):
            if b.stopped:
                return
            self.nodes += 1
            if self.nodes > MAX_NODES:
                return self._stop(f"more than {MAX_NODES} steps were interpreted (loop too long)")
            if not isinstance(s, dict):
                return self._stop(f"a step is not an object: {str(s)[:60]}")
            try:
                if "find" in s:
                    o, hits, _ = self._sel(s["find"], item)
                    hits = hits[: int(s.get("max", 20))]
                    name = str(s.get("as") or "found")
                    self.vars[name] = [{"_token": f"{o.snapshot_id}:{e.index}", "_wid": o.window_id,
                                        "label": e.label, "role": e.role} for e in hits]
                    b.lines.append(f"find -> {name}: {len(hits)} match(es)"
                                   + (f" ({', '.join(e.label for e in hits[:5])})" if hits else ""))
                elif "for_each" in s:
                    items = self.vars.get(str(s["for_each"]))
                    if items is None:
                        return self._stop(f"for_each: no list named \"{s['for_each']}\" (use find ... as first)")
                    for it in items:
                        self.run(s.get("do") or [], depth + 1, it)
                        if b.stopped:
                            return
                elif "if" in s:
                    cond = s["if"] or {}
                    key = "exists" if "exists" in cond else "not_exists" if "not_exists" in cond else None
                    if key is None:
                        return self._stop("if needs {exists: selector} or {not_exists: selector}")
                    _, hits, _ = self._sel(cond[key], item)
                    ok = bool(hits) if key == "exists" else not hits
                    b.lines.append(f"if {key} -> {'then' if ok else 'else'}")
                    self.run(s.get("then") if ok else s.get("else"), depth + 1, item)
                elif "read" in s:
                    o, hits, _ = self._sel(s["read"], item)
                    vals = [(_line_of(o, e.index) or f'{e.role} "{e.label}"') for e in hits[:20]]
                    name = s.get("as") or "read"
                    self.out.append(f"{name}: " + (" | ".join(vals) if vals else "(nothing matched)"))
                else:
                    tool = s.get("tool")
                    if not tool:  # also accept {"click": {...}}
                        keys = [k for k in s if k != "expect"]
                        tool = keys[0] if keys else ""
                        sargs = s.get(tool) if isinstance(s.get(tool), dict) else {}
                    else:
                        sargs = s.get("args") or {}
                    b.step(str(tool), _subst(sargs, item), s.get("expect"),
                           last=(top and i == len(steps) - 1))
            except (ValueError, re.error, TypeError) as e:
                return self._stop(f"bad step {str(s)[:80]}: {e}")


def run_actions(gate, args: dict) -> dict:
    steps = args.get("steps")
    if not isinstance(steps, list) or not steps:
        return {"isError": True, "content": [{"type": "text", "text": "run_actions needs a non-empty `steps` list"}]}
    b = Batch(gate, args.get("pid"), args.get("window_id"), args.get("stop_on_new_window", True) is not False)
    prog = Program(b)
    gate._batch = True
    try:
        prog.run(steps, top=True)
    finally:
        gate._batch = False
    head = (f"[gate] run_actions: {b.executed} GUI action(s) ran."
            + (f" STOPPED: {b.stopped}. Steps after it did NOT run." if b.stopped else " All done."))
    body = "\n".join(b.lines + prog.out)
    out = {"content": [{"type": "text", "text": head + "\n" + body}],
           "structuredContent": {"batch": {"executed": b.executed, "stopped": b.stopped, "read": prog.out}}}
    if b.stopped:
        out["isError"] = True
    return b.finish(out)
