"""Server-side batching ("codemode inside the gate"), so every MCP client gets it, not only Pi.

Two tools share one engine:
  run_actions  JSON list of steps (zero dependencies, any client, any model).
  gui_script   JavaScript in a QuickJS sandbox with Pi-codemode-style globals (tools.<name>(args), text(),
               console.log, return, store/load). Optional: needs the `quickjs` Python package. The script
               API follows Pi's codemode (earendil-works/pi, MIT); the implementation here is our own.

Gate semantics inside a batch (see docs/design-codemode-in-gate.md):
  * every GUI step goes through Gate.gated() exactly like a single call; a refusal stops the batch;
  * no auto-observe between steps, one observation at the end (or where the batch stopped);
  * blind-batch guard: stop on a driver error, an unexpected new window/dialog, a failed `expect`, or a
    pixel (x/y) step after the first executed step (coordinates from before the batch are stale by then);
  * at most MAX_STEPS GUI actions per batch.
"""
from __future__ import annotations

import json
import re
import time
from typing import Optional

from .features import GATED_TOOLS, OBS_TOOLS
from .jsonrpc import text_of

MAX_STEPS = 15
NOT_IN_BATCH = {"run_steps", "run_script", "kill_app"}

RUN_ACTIONS_TOOL = {
    "name": "run_actions",
    "description": (
        "Run several GUI actions in ONE call when you can already see every target (e.g. open a menu, pick an "
        "item, type a name, press Enter). Each step is checked by the gate like a single call. The batch stops "
        "at the first refused or failed step, when a new window/dialog opens unexpectedly, or when an `expect` "
        "fails; you get what ran plus one fresh read of the window at the end. x/y (pixel) targets are allowed "
        "only in the first step. Name things that appear only after an earlier step (menu items, dialog buttons) with "
        "args.target = {\"label\": \"Save As\", \"role\": \"menu item\"}: the gate finds it on screen when the step runs. "
        "A popup menu opening is expected; any other new window stops the batch unless that step says "
        "expect.new_window. Max 15 steps."),
    "inputSchema": {"type": "object", "properties": {
        "pid": {"type": "integer", "description": "Default pid for steps that omit it."},
        "window_id": {"type": "integer", "description": "Default window_id for steps that omit it."},
        "steps": {"type": "array", "description": "In order. Each: {tool, args, expect?}.", "items": {
            "type": "object", "properties": {
                "tool": {"type": "string", "description": "A GUI action tool: click, type_text, press_key, hotkey, "
                                                          "scroll, set_value, double_click, right_click, drag, invoke_menu"},
                "args": {"type": "object", "description": "That tool's arguments."},
                "expect": {"type": "object", "description": "Optional check after this step: "
                           "{\"title_contains\": \"Save\"} and/or {\"new_window\": true} when this step should open one."},
            }, "required": ["tool", "args"]}},
    }, "required": ["steps"]},
}

GUI_SCRIPT_TOOL = {
    "name": "gui_script",
    "description": (
        "Run a short JavaScript program that calls the GUI tools, for loops or decisions between steps "
        "(read the tree, find rows, click several of them). The input is raw JavaScript, the body of an async "
        "function: call tools as `await tools.click({...})` / `tools.get_window_state({...})` (results are the "
        "tool's {text, structuredContent}); a refused or failed GUI call throws and ends the script. Use "
        "text(v) / return v / console.log() for output; only that output comes back, plus one fresh read of the "
        "window at the end. store(k,v)/load(k) keep small JSON across calls. No file system, network or timers. "
        "Same gate rules as run_actions (max 15 GUI actions, pixel x/y only before the first executed action)."),
    "inputSchema": {"type": "object", "properties": {
        "script": {"type": "string", "description": "JavaScript source. Optional first line: "
                   "// @options: {\"timeout_ms\": 60000, \"max_output_chars\": 20000}"},
    }, "required": ["script"]},
}


def quickjs_available() -> bool:
    try:
        import quickjs  # noqa: F401
        return True
    except Exception:
        return False


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

    def resolve(self, a: dict) -> Optional[str]:
        """`target: {label, role?}` -> a fresh element_token, read at the moment the step runs (so steps can name
        things that only appear after earlier steps, such as menu items). Newest windows are searched first.
        Returns an error string, or None when a["element_token"] was set."""
        tgt = a.pop("target")
        if not isinstance(tgt, dict) or not tgt.get("label"):
            return "target needs {\"label\": ..., \"role\": optional}"
        pid = a.get("pid")
        if pid is None:
            return "target needs a pid"
        want, role = _norm_label(str(tgt["label"])), str(tgt.get("role") or "").lower()
        wins = []
        try:
            r = self.g._tool("list_windows", {"pid": int(pid)}, timeout=10)
            ws = [w for w in (r.get("structuredContent") or {}).get("windows") or [] if w.get("is_on_screen", True)]
            ws.sort(key=lambda w: -(w.get("z_index") or 0))
            wins = [w.get("window_id") for w in ws]
        except Exception:
            pass
        # windows opened during the batch (popups, menus) first, then the batch's own window, then the rest
        base = a.get("window_id")
        fresh = [w for w in wins if str(w) not in (self.seen or set()) and w != base]
        order = fresh + ([base] if base else []) + [w for w in wins if w not in fresh and w != base]
        seen_names = []
        for wid in order[:4]:
            o = self._read(pid, wid)
            if o is None:
                continue
            hits = [e for e in o.elements.values() if _norm_label(e.label) == want and (not role or role in (e.role or ""))]
            seen_names += [e.label for e in o.elements.values() if e.label][:12]
            if len(hits) == 1:
                a["element_token"] = f"{o.snapshot_id}:{hits[0].index}"
                a["window_id"] = o.window_id or wid
                a.pop("x", None), a.pop("y", None)
                return None
            if len(hits) > 1:
                return f"\"{tgt['label']}\" matches {len(hits)} elements; add a role or use an element_token"
        return f"no element named \"{tgt['label']}\" is on screen (seen: {', '.join(seen_names[:10])})"

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


def run_actions(gate, args: dict) -> dict:
    steps = args.get("steps")
    if not isinstance(steps, list) or not steps:
        return {"isError": True, "content": [{"type": "text", "text": "run_actions needs a non-empty `steps` list"}]}
    b = Batch(gate, args.get("pid"), args.get("window_id"), args.get("stop_on_new_window", True) is not False)
    gate._batch = True
    try:
        for i, s in enumerate(steps):
            if not isinstance(s, dict):
                b.stopped = f"step {i + 1} is not an object"
                break
            tool = s.get("tool")
            if not tool:  # also accept {"click": {...}}
                keys = [k for k in s if k not in ("expect",)]
                tool = keys[0] if keys else ""
                sargs = s.get(tool) if isinstance(s.get(tool), dict) else {}
            else:
                sargs = s.get("args") or {}
            b.step(str(tool), sargs, s.get("expect"), last=(i == len(steps) - 1))
            if b.stopped:
                break
    finally:
        gate._batch = False
    head = (f"[gate] run_actions: {b.executed} of {len(steps)} steps ran."
            + (f" STOPPED: {b.stopped}. Steps after it did NOT run." if b.stopped else " All done."))
    out = {"content": [{"type": "text", "text": head + "\n" + "\n".join(b.lines)}],
           "structuredContent": {"batch": {"executed": b.executed, "total": len(steps), "stopped": b.stopped}}}
    if b.stopped:
        out["isError"] = True
    return b.finish(out)


# ----------------------------------------------------------------- gui_script (QuickJS)
_PRELUDE = r"""
var __out = [], __con = [], __res, __err, __done = false;
function __fmt(v) { return typeof v === 'string' ? v : JSON.stringify(v); }
function text(v) { __out.push(__fmt(v)); }
var console = { log: function () { __con.push(Array.prototype.map.call(arguments, __fmt).join(' ')); } };
console.info = console.warn = console.error = console.debug = console.log;
var __EXIT = { exit: true };
function exit() { throw __EXIT; }
var tools = new Proxy({}, { get: function (_, n) { return function (a) {
  var r = JSON.parse(__call(String(n), JSON.stringify(a === undefined ? {} : a)));
  if (r && r.__error) throw new Error(r.__error);
  return r; }; } });
var __store = JSON.parse(__store_in);
function store(k, v) { if (v === undefined) delete __store[k]; else __store[k] = v; }
function load(k) { return __store[k]; }
var ALL_TOOLS = JSON.parse(__tools_in);
"""

_OPT_RX = re.compile(r"^\s*//\s*@options:\s*(\{.*\})\s*$")


def _js_str(s: str) -> str:
    return json.dumps(s)


def run_js(src: str, store: dict, names: list, host_call) -> dict:
    """Run `src` in QuickJS. Called inside the child process (see run_js_child). Returns
    {out, con, res, err, store}. host_call(name, args_json) -> result_json."""
    import quickjs
    ctx = quickjs.Context()
    try:
        ctx.set_memory_limit(128 * 1024 * 1024)
    except Exception:
        pass
    ctx.add_callable("__call", host_call)
    err = None
    out_items, con, new_store = [], [], store
    try:
        ctx.eval(f"var __store_in = {_js_str(json.dumps(store))}; var __tools_in = {_js_str(json.dumps(names))};")
        ctx.eval(_PRELUDE)
        ctx.eval("(async function () {\n" + src + "\n})().then(function (v) { __res = v; __done = true; },"
                 " function (e) { __done = true; if (e === __EXIT) return; __err = String(e) + ((e && e.stack) ? '\\n' + e.stack : ''); });")
        while ctx.execute_pending_job():
            pass
        err = ctx.eval("__err === undefined ? '' : __err") or None
        if err is None and not ctx.eval("__done"):
            err = "the script waited on something that can never finish (there are no timers)"
        out_items = json.loads(ctx.eval("JSON.stringify(__out)") or "[]")
        res = ctx.eval("JSON.stringify(__res === undefined ? null : __res)")
        if res and res != "null":
            v = json.loads(res)
            out_items.append(v if isinstance(v, str) else json.dumps(v, ensure_ascii=False))
        con = json.loads(ctx.eval("JSON.stringify(__con)") or "[]")
        if err is None:
            new_store = json.loads(ctx.eval("JSON.stringify(__store)") or "{}")
    except Exception as e:  # syntax errors, out of memory
        err = err or f"{type(e).__name__}: {e}"
    return {"out": out_items, "con": con, "err": err, "store": new_store}


def run_js_child() -> None:
    """Entry point of the sandbox child process: `python -m gui_gate.batch`. Protocol (JSON lines):
    parent -> {"src","store","names"}; child -> {"call": name, "args": json} / parent -> result json;
    child -> {"done": {...}}. The parent kills the child on timeout (QuickJS refuses a time limit
    while Python callables are registered)."""
    import sys
    rd, wr = sys.stdin, sys.stdout
    job = json.loads(rd.readline())

    def host_call(name, ajson):
        wr.write(json.dumps({"call": name, "args": ajson}) + "\n")
        wr.flush()
        line = rd.readline()
        return line.rstrip("\n") if line else json.dumps({"__error": "gate went away"})

    done = run_js(job["src"], job.get("store") or {}, job.get("names") or [], host_call)
    wr.write(json.dumps({"done": done}) + "\n")
    wr.flush()


def gui_script(gate, args: dict) -> dict:
    import select
    import subprocess
    import sys
    src = str(args.get("script") or args.get("code") or "")
    if not src.strip():
        return {"isError": True, "content": [{"type": "text", "text": "gui_script needs `script`"}]}
    lines = src.splitlines()
    opts = {}
    if lines and _OPT_RX.match(lines[0]):
        try:
            opts = json.loads(_OPT_RX.match(lines[0]).group(1))
        except json.JSONDecodeError:
            return {"isError": True, "content": [{"type": "text", "text": "bad // @options line (must be JSON)"}]}
        src = "\n".join(lines[1:])
    timeout_s = min(float(opts.get("timeout_ms", 60000)) / 1000.0, 300.0)
    max_chars = int(opts.get("max_output_chars", 20000))
    b = Batch(gate)
    t0 = time.time()
    t_end = t0 + timeout_s

    def host_call(name, ajson):
        try:
            a = json.loads(ajson) if ajson else {}
        except json.JSONDecodeError:
            return json.dumps({"__error": f"{name}: arguments are not JSON"})
        if b.stopped:
            return json.dumps({"__error": f"batch already stopped: {b.stopped}"})
        if name in OBS_TOOLS:
            r = gate.passthrough(name, a)
        elif name in GATED_TOOLS:
            r = b.step(name, a)
            if b.stopped:
                return json.dumps({"__error": f"{b.stopped}: {text_of(r)[:600]}"})
        else:
            return json.dumps({"__error": f"unknown or unavailable tool `{name}`"})
        if r.get("isError"):
            return json.dumps({"__error": text_of(r)[:800]})
        sc = dict(r.get("structuredContent") or {})
        return json.dumps({"text": text_of(r), "structuredContent": sc})

    names = sorted((GATED_TOOLS - NOT_IN_BATCH) | OBS_TOOLS)
    pkg_root = __import__("os").path.dirname(__import__("os").path.dirname(__import__("os").path.abspath(__file__)))
    env = dict(__import__("os").environ)
    env["PYTHONPATH"] = pkg_root + ((":" + env["PYTHONPATH"]) if env.get("PYTHONPATH") else "")
    p = subprocess.Popen([sys.executable, "-m", "gui_gate.batch"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.DEVNULL, text=True, env=env)
    done, err = None, None
    gate._batch = True
    try:
        p.stdin.write(json.dumps({"src": src, "store": gate._script_store, "names": names}) + "\n")
        p.stdin.flush()
        while True:
            left = t_end - time.time()
            if left <= 0:
                err = f"script timeout after {timeout_s:.0f} s"
                break
            ready, _, _ = select.select([p.stdout], [], [], min(left, 1.0))
            if not ready:
                if p.poll() is not None:
                    err = "the script sandbox exited unexpectedly"
                    break
                continue
            line = p.stdout.readline()
            if not line:
                err = "the script sandbox exited unexpectedly"
                break
            msg = json.loads(line)
            if "done" in msg:
                done = msg["done"]
                break
            p.stdin.write(host_call(msg.get("call", ""), msg.get("args") or "{}") + "\n")
            p.stdin.flush()
    except (BrokenPipeError, OSError, json.JSONDecodeError) as e:
        err = err or f"script sandbox error: {e!r}"
    finally:
        gate._batch = False
        if p.poll() is None:
            p.kill()
        try:
            p.wait(timeout=2)
        except Exception:
            pass
    out_items, con = [], []
    if done is not None:
        out_items, con, err = done.get("out") or [], done.get("con") or [], done.get("err")
        if not err:
            gate._script_store = done.get("store") or {}
    body = "\n".join(out_items)
    if con:
        body += "\n<console_output>\n" + "\n".join(con) + "\n</console_output>"
    if len(body) > max_chars:
        body = body[: max_chars // 2] + f"\n... ({len(body) - max_chars} chars cut) ...\n" + body[-max_chars // 2:]
    head = (f"Script {'failed' if err else 'completed'} in {time.time() - t0:.1f} s; {b.executed} GUI action(s) ran."
            + (f" Batch stopped: {b.stopped}." if b.stopped else ""))
    text = head + ("\n" + body if body else "") + (f"\nScript error: {err}" if err else "")
    if b.lines:
        text += "\n[gate] actions: " + " | ".join(b.lines)
    out = {"content": [{"type": "text", "text": text}],
           "structuredContent": {"batch": {"executed": b.executed, "stopped": b.stopped, "error": err}}}
    if err:
        out["isError"] = True
    return b.finish(out)


if __name__ == "__main__":
    run_js_child()
