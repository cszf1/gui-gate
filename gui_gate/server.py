"""gui-gate: Cua Driver with a pre-action value gate, served as ONE MCP server.

    agent (Pi, opencode, Claude Code, ...)  --stdio MCP-->  gui-gate  --stdio-->  cua-driver mcp

Every driver tool is exposed unchanged. Action tools pass through the gate,
which either executes them, probes cheaply first, waits, repairs a stale
token, or answers with a short "[gate] ..." instruction (observe again,
double-check, ask the user). Observation tools pass straight through and
update what the gate knows.
"""
from __future__ import annotations

import argparse
import json
import queue
import re
import os
import secrets
import sys
import threading
import time
from typing import Optional

from . import __version__
from .decide import Decision, GateConfig, choose, hard_rules, p_direct, policy_decision
from .features import GATED_TOOLS, OBS_TOOLS, ActionCtx, build_ctx, features
from .human import Human
from .jsonrpc import ChildMCP, StdioWriter, find_driver, log_err, text_of
from .log import EventLog
from .state import ActionRecord, GateState

GATE_ARGS = ("gate_ack", "gate_confirm")

CORE_TOOLS = {"list_apps", "list_windows", "get_window_state", "launch_app", "bring_to_front", "click",
              "double_click", "right_click", "type_text", "press_key", "hotkey", "scroll", "set_value", "drag",
              "invoke_menu", "gate_report", "request_human"}
# parameters an agent driving a GUI task practically never needs (dropped from schemas in core mode;
# calls that still pass them go through unchanged)
RARE_PARAMS = {"session", "capture_mode", "screenshot_out_file", "verbose", "full_output", "display_only", "cursor_id",
               "delivery_mode", "scope", "from_zoom", "capture_id", "timeout_ms", "max_image_dimension", "target"}
AUTO_OBS_TOOLS = {"click", "double_click", "right_click", "press_key", "hotkey", "scroll", "invoke_menu", "drag"}
_BG_UNAVAILABLE = re.compile(r"background delivery is not available|no focus-free input backend|background_unavailable", re.I)
_NO_FRAME = re.compile(r"no screenshot frame", re.I)
_LOADING_RX = re.compile(r"(loading|busy|please wait|加载|正在|请稍候)", re.I)
MAX_TREE_LINES = 120

GATE_TOOLS = [
    {
        "name": "request_human",
        "description": "Ask the person at this computer. kind=approve: a yes/no on what you are about to do. "
                       "kind=answer: a short free-text answer (e.g. which option they want). kind=takeover: ask them to "
                       "do a step themselves (log in, solve a captcha, anything you cannot or should not do); it returns "
                       "when they are done. Use only when a person is really needed.",
        "inputSchema": {"type": "object", "properties": {
            "question": {"type": "string", "description": "What you need from them, in one or two plain sentences."},
            "kind": {"type": "string", "enum": ["approve", "answer", "takeover"]},
        }, "required": ["question"]},
    },
    {
        "name": "gate_report",
        "description": "Value-gate statistics for this session: how many actions ran directly, how many "
                       "observations/screenshots were requested or avoided, interventions by type, and gate overhead.",
        "inputSchema": {"type": "object", "properties": {}, "required": []},
        "annotations": {"readOnlyHint": True},
    },
    {
        "name": "gate_config",
        "description": "Read or change the value gate at runtime. policy: gate | direct | light | full. "
                       "lambda: cost weight (higher = fewer observations). collect: shadow-label every action. "
                       "decider: formula | model (formula, then the main model settles the grey zone). risk_mode: auto | model | human.",
        "inputSchema": {"type": "object", "properties": {
            "policy": {"type": "string", "enum": ["gate", "direct", "light", "full"]},
            "lambda": {"type": "number", "minimum": 0},
            "collect": {"type": "boolean"},
            "risk_confirm": {"type": "boolean"},
            "decider": {"type": "string", "enum": ["formula", "model"]},
            "risk_mode": {"type": "string", "enum": ["auto", "model", "human"]},
        }, "required": []},
    },
]

INSTRUCTIONS_NOTE = (
    "\n\n[gui-gate] Actions go through a value gate before they run. Usually they execute at once. "
    "If a call returns a message starting with \"[gate]\", nothing was executed: do exactly what it says "
    "(re-read cheaply, re-read with a screenshot, double-check the target or think a risky step over and "
    "retry with gate_ack:true, or ask the user and retry with gate_confirm). To save time the gate omits the screenshot "
    "from get_window_state when the element tree is enough; pass include_screenshot:true when you really need pixels."
)


def _short(text: str, limit: int) -> str:
    """Trim a long tool/parameter description at a sentence boundary (each turn resends them all)."""
    if len(text) <= limit:
        return text
    cut = text[:limit]
    k = max(cut.rfind(". "), cut.rfind(".\n"), cut.rfind("。"))
    return (cut[:k + 1] if k > limit * 0.4 else cut.rstrip() + " ...")


def _refusal(dec: Decision, text: str, extra: Optional[dict] = None) -> dict:
    sc = {"gate": {"decision": dec.option, "tier": dec.tier, "p_direct": round(dec.p_direct, 3),
                   "reason": dec.reason, "executed": False}}
    if extra:
        sc["gate"].update(extra)
    return {"isError": True, "content": [{"type": "text", "text": text}], "structuredContent": sc}


_OPENER = ("menu", "menu bar item", "menubar", "page tab", "tab", "combo box")


def _fast_path(ctx, f) -> Optional[str]:
    """Cases where any extra look costs more than a miss: (1) opening a menu or tab by its tree row
    (a wrong one is closed with Escape, and the auto-observe shows what opened); (2) a pixel click
    planned on a screenshot the model got right before it, with nothing done since."""
    t = ctx.target
    role = ((t.element.role if t.element else t.role) or "").lower()
    if t.kind in ("token", "index", "name") and role in _OPENER and ctx.tool in ("click",):
        return f"opens a {role}: harmless if wrong (Escape), the result is shown right after"
    if (t.kind == "pixel" and f.get("obs_had_shot", 0) >= 1 and f.get("acts_since_obs", 1) == 0
            and f.get("window_changed", 0) < 0.5 and f.get("last_err", 0) < 0.5 and f.get("repeat", 0) < 0.5):
        return "pixel target planned on a fresh screenshot, nothing changed since"
    return None


def _nearest(old, cands):
    """Several rows share the role+label: keep the one where the old row was (same place on screen,
    else the nearest row index), when that choice is clear."""
    def ctr(e):
        fr = e.frame or {}
        return (fr.get("x", 0) + fr.get("w", 0) / 2, fr.get("y", 0) + fr.get("h", 0) / 2) if fr else None
    oc = ctr(old)
    if oc is not None and all(ctr(c) is not None for c in cands):
        d = sorted(((abs(ctr(c)[0] - oc[0]) + abs(ctr(c)[1] - oc[1]), i) for i, c in enumerate(cands)))
        if d[0][0] <= 8 and (len(d) == 1 or d[1][0] > 30):
            return [cands[d[0][1]]]
        return cands
    d = sorted(((abs(c.index - old.index), i) for i, c in enumerate(cands)))
    if len(d) == 1 or d[1][0] > d[0][0] + 5:
        return [cands[d[0][1]]]
    return cands


class Gate:
    def __init__(self, child_cmd: list[str], cfg: GateConfig, log_path: Optional[str]):
        self.cfg = cfg
        self.out = StdioWriter(sys.stdout)
        self.state = GateState()
        self.log = EventLog(log_path)
        self.lock = threading.Lock()
        self.confirm_codes: dict[str, str] = {}
        self.pending_methods: dict = {}  # agent request id -> method (to rewrite some responses)
        self.stats = {"actions": 0, "executed": 0, "by_option": {}, "obs_calls": 0, "screenshots": 0,
                      "screenshot_px": 0, "tree_reads": 0, "model_turns_requested": 0,
                      "gate_ms": 0.0, "driver_ms": 0.0, "exec_errors": 0, "heals": 0, "checks": 0,
                      "screenshots_skipped": 0, "model_reviews": 0, "model_acks": 0, "auto_obs": 0,
                      "human_asked": 0, "human_allowed": 0}
        self.started = time.time()
        self.aid = 0
        self._want_shot: set = set()  # pids whose next read must carry a screenshot
        self._seen_pids: set = set()
        self._delivery_tools: set = {"click", "double_click", "right_click", "drag", "type_text", "press_key",
                                     "hotkey", "set_value", "scroll"}
        self._fg_sticky = False
        # windows the agent has to target by pixels (it clicked by x/y, or the element tree is partial,
        # e.g. a spreadsheet grid): their reads keep the screenshot, since a read without one also
        # drops the driver's pixel frame and the next x/y click fails
        self._pixel_pids: set = set()
        self._probe_el = None
        self.child_cmd = child_cmd
        self.init_params: Optional[dict] = None
        self.client_elicit = False
        self._client_pending: dict[str, queue.Queue] = {}
        self._cids = 0
        self.restarts = 0
        self.human = Human(cfg.human_channel, cfg.human_timeout, elicit=self._elicit,
                           client_can_elicit=lambda: self.client_elicit)
        self.child = self._spawn()

    def _spawn(self) -> ChildMCP:
        return ChildMCP(self.child_cmd, self._from_child, stderr=sys.stderr if os.environ.get("GUI_GATE_DEBUG") else None)

    def _respawn(self) -> None:
        """The driver died: start a new one and replay the handshake (agent never notices)."""
        try:
            self.child.close()
        except Exception:
            pass
        self.child = self._spawn()
        self.restarts += 1
        params = self.init_params or {"protocolVersion": "2025-06-18", "capabilities": {},
                                      "clientInfo": {"name": "gui-gate", "version": __version__}}
        self.child.call("initialize", params, timeout=30)
        self.child.forward({"jsonrpc": "2.0", "method": "notifications/initialized"})
        self.log.write({"ev": "driver_restart", "n": self.restarts})
        log_err("driver restarted")

    def _tool(self, name: str, args: dict, timeout: float = 60.0) -> dict:
        # input delivery: background (no focus steal) when the desktop supports it, else foreground.
        # cfg.delivery auto: try background, and on "no focus-free input backend" retry in foreground
        # and stay in foreground for the rest of the session (sticky).
        if name in self._delivery_tools and "delivery_mode" not in args:
            mode = self.cfg.delivery
            if mode == "foreground" or (mode == "auto" and self._fg_sticky):
                args = dict(args, delivery_mode="foreground")
        r = self._tool_raw(name, args, timeout)
        if (self.cfg.delivery == "auto" and r.get("isError") and name in self._delivery_tools
                and args.get("delivery_mode") != "foreground" and _BG_UNAVAILABLE.search(text_of(r))):
            if not self._fg_sticky:
                self._fg_sticky = True
                self.log.write({"ev": "delivery_fallback", "tool": name, "to": "foreground"})
            self.stats["delivery_fallbacks"] = self.stats.get("delivery_fallbacks", 0) + 1  # (caller may hold self.lock)
            r = self._tool_raw(name, dict(args, delivery_mode="foreground"), timeout)
        return r

    def _tool_raw(self, name: str, args: dict, timeout: float = 60.0) -> dict:
        if self.child.p.poll() is not None:
            self._respawn()
        try:
            r = self.child.tool(name, args, timeout=timeout)
        except (BrokenPipeError, OSError):
            r = {"isError": True, "content": [{"type": "text", "text": "driver exited"}]}
        if r.get("isError") and (self.child.p.poll() is not None or text_of(r) in ("driver exited",)):
            self._respawn()
            try:
                r = self.child.tool(name, args, timeout=timeout)
            except (BrokenPipeError, OSError):
                r = {"isError": True, "content": [{"type": "text", "text": "driver exited twice; giving up on this call"}]}
        return r

    def _elicit(self, params: dict, timeout: float) -> Optional[dict]:
        self._cids += 1
        cid = f"gate-c-{self._cids}"
        q: queue.Queue = queue.Queue(maxsize=1)
        self._client_pending[cid] = q
        self.out.send({"jsonrpc": "2.0", "id": cid, "method": "elicitation/create", "params": params})
        try:
            m = q.get(timeout=timeout)
        except queue.Empty:
            self._client_pending.pop(cid, None)
            return None
        return m.get("result")

    # ------------------------------------------------------------- plumbing
    def _from_child(self, msg: dict) -> None:
        mid = msg.get("id")
        if mid is not None and "method" not in msg:
            meth = self.pending_methods.pop(mid, None)
            if meth == "tools/list" and "result" in msg:
                self._augment_tools(msg["result"])
            elif meth == "initialize" and "result" in msg:
                r = msg["result"]
                r["instructions"] = (r.get("instructions") or "") + INSTRUCTIONS_NOTE
                si = r.get("serverInfo") or {}
                si["name"] = f"gui-gate (cua-driver {si.get('version', '?')})"
                si["version"] = __version__
                r["serverInfo"] = si
        self.out.send(msg)

    def _augment_tools(self, result: dict) -> None:
        tools = result.get("tools") or []
        for t in tools:
            sch = t.setdefault("inputSchema", {"type": "object", "properties": {}})
            # some OpenAI-compatible backends reject schemas without an explicit "required" array
            if not isinstance(sch.get("required"), list):
                sch["required"] = []
            sch.setdefault("properties", {})
            if "delivery_mode" in (sch.get("properties") or {}):
                self._delivery_tools.add(t.get("name"))
            if self.cfg.toolset == "core":
                t["description"] = _short(t.get("description") or "", 600)
                props0 = sch.get("properties") or {}
                for k in list(props0):
                    if k in RARE_PARAMS and k not in (sch.get("required") or []):
                        props0.pop(k)
                for pv in props0.values():
                    if isinstance(pv, dict) and isinstance(pv.get("description"), str):
                        pv["description"] = _short(pv["description"], 140)
            if t.get("name") in GATED_TOOLS:
                props = t.setdefault("inputSchema", {}).setdefault("properties", {})
                props["gate_ack"] = {"type": "boolean", "description": "Set true after the gate asked you to "
                                     "double-check this target and you are sure. Never set it pre-emptively."}
                props["gate_confirm"] = {"type": "string", "description": "Confirmation code the gate gave "
                                         "for a high-risk action, only after the user approved it."}
        allt = tools + GATE_TOOLS
        if self.cfg.toolset == "core":
            allt = [t for t in allt if t.get("name") in CORE_TOOLS]
        result["tools"] = allt

    def serve(self) -> None:
        for line in sys.stdin:
            line = line.strip()
            if not line:
                continue
            try:
                msg = json.loads(line)
            except json.JSONDecodeError:
                continue
            method = msg.get("method")
            mid = msg.get("id")
            if method is None and isinstance(mid, str) and mid in self._client_pending:
                self._client_pending.pop(mid).put(msg)
                continue
            if method == "initialize":
                self.init_params = msg.get("params") or {}
                self.client_elicit = "elicitation" in (self.init_params.get("capabilities") or {})
            if method == "tools/call" and "id" in msg:
                threading.Thread(target=self._handle_call, args=(msg,), daemon=True).start()
                continue
            if method in ("tools/list", "initialize") and "id" in msg:
                self.pending_methods[msg["id"]] = method
            if self.child.p.poll() is not None and method != "initialize":
                self._respawn()
            try:
                self.child.forward(msg)
            except (BrokenPipeError, OSError):
                self._respawn()
                self.child.forward(msg)
        self.child.close()
        self.log.close()

    def _handle_call(self, msg: dict) -> None:
        params = msg.get("params") or {}
        name = params.get("name", "")
        args = dict(params.get("arguments") or {})
        try:
            if name == "gate_report":
                result = self.report()
            elif name == "gate_config":
                result = self.config(args)
            elif name == "request_human":
                result = self.request_human(args)
            elif name in GATED_TOOLS:
                with self.lock:
                    result = self.gated(name, args)
            else:
                result = self.passthrough(name, args)
            self.out.send({"jsonrpc": "2.0", "id": msg["id"], "result": result})
        except Exception as e:  # never crash the agent's session
            log_err("error handling", name, repr(e))
            self.out.send({"jsonrpc": "2.0", "id": msg["id"],
                           "result": {"isError": True, "content": [{"type": "text", "text": f"[gate] internal error: {e!r}"}]}})

    # --------------------------------------------------------- pass-through
    def passthrough(self, name: str, args: dict) -> dict:
        t0 = time.time()
        trimmed = False
        if name == "get_window_state" and self.cfg.policy == "gate" and "include_screenshot" not in args:
            pid = args.get("pid")
            if pid in self._seen_pids and pid not in self._want_shot and pid not in self._pixel_pids:
                args = dict(args, include_screenshot=False)
                trimmed = True
        res = self._tool(name, args, timeout=120)
        if name == "get_window_state" and "PARTIAL TREE" in text_of(res) and args.get("pid") is not None:
            self._pixel_pids.add(args.get("pid"))
        if trimmed:
            md = (res.get("structuredContent") or {}).get("tree_markdown") or text_of(res)
            if md.count("- [") < 2:  # tree too thin to act on: give the pixels after all
                args = dict(args, include_screenshot=True)
                res = self._tool(name, args, timeout=120)
                trimmed = False
            else:
                with self.lock:
                    self.stats["screenshots_skipped"] += 1
                res.setdefault("content", []).append({"type": "text", "text":
                    "[gate] screenshot omitted (the element tree is enough here); call again with "
                    "include_screenshot:true if you need pixels."})
        if name == "get_window_state":
            pid = args.get("pid")
            self._seen_pids.add(pid)
            if args.get("include_screenshot", True):
                self._want_shot.discard(pid)
        ms = (time.time() - t0) * 1000
        if name in OBS_TOOLS:
            with self.lock:
                self.stats["obs_calls"] += 1
                shot = [c for c in res.get("content", []) if c.get("type") == "image"]
                sc = res.get("structuredContent") or {}
                px = 0
                if shot:
                    self.stats["screenshots"] += 1
                    px = int(sc.get("screenshot_width") or 0) * int(sc.get("screenshot_height") or 0)
                    self.stats["screenshot_px"] += px
                if name == "get_window_state":
                    self.stats["tree_reads"] += 1
                    o = self.state.record_observation(args, res)
                    if o is not None and o.pid is not None:
                        o.window_count = self._window_sig(o.pid)
                self.log.write({"ev": "obs", "tool": name, "ms": round(ms, 1), "screenshot": bool(shot),
                                "px": px, "snapshot": sc.get("snapshot_id"), "pid": sc.get("pid", args.get("pid")),
                                "trimmed": trimmed})
        return res

    def _window_sig(self, pid) -> Optional[str]:
        r = self._tool("list_windows", {"pid": int(pid)}, timeout=10)
        ws = (r.get("structuredContent") or {}).get("windows")
        if ws is None:
            return None
        return ";".join(sorted(f"{w.get('window_id')}@{w.get('x')},{w.get('y')},{w.get('width')}x{w.get('height')}"
                               for w in ws if w.get("is_on_screen", True)))

    # ---------------------------------------------------------------- probes
    def _probe_window_changed(self, ctx: ActionCtx):
        """Returns (changed_any, window_added) since the agent's last read, or (None, None)."""
        obs = self.state.latest_for(ctx.target.pid, ctx.target.window_id) if ctx.target.pid else None
        if obs is None or obs.window_count is None or obs.pid is None:
            return None, None
        now = self._window_sig(obs.pid)
        if now is None:
            return None, None
        ids = lambda sig: {x.split("@")[0] for x in sig.split(";") if x}
        return now != obs.window_count, bool(ids(now) - ids(obs.window_count))

    def _check_available(self, ctx: ActionCtx) -> bool:
        t = ctx.target
        el = t.element
        role = (el.role if el else t.role)
        label = (el.label if el else t.label)
        wid = t.window_id or (t.obs.window_id if t.obs else None)
        return bool(t.kind in ("token", "index", "name") and label and t.pid and wid and role)

    def _check(self, ctx: ActionCtx, args: Optional[dict] = None) -> Optional[bool]:
        """Cheap probe, no model involved.

        * element-token targets: refresh the tree WITHOUT a screenshot (~30 ms) and
          re-bind the token to the unique element with the same role+label. This
          catches relabels/reorders that an existence check cannot see. Sets
          self._probe_disabled when the element exists but is disabled (busy UI).
        * name targets: verify_state existence check (~20 ms, no snapshot change).
        """
        t = ctx.target
        el = t.element
        self.stats["checks"] += 1
        self._probe_disabled = False
        self._probe_el = None
        if t.kind in ("token", "index") and el is not None and el.label and args is not None:
            r = self._refresh(ctx)
            if r is None:
                return False
            newtok, newel = r
            if newtok is None:
                return False
            if t.kind == "token":
                args["element_token"] = newtok
            else:
                args["element_index"] = int(newtok.split(":")[1])
            self._probe_disabled = newel.enabled is False
            self._probe_el = newel
            return newel.enabled is not False
        sel = {"label_contains": (el.label if el else t.label)[:200]}
        role = el.role if el else t.role
        if role:
            sel["role"] = role
        pred = {"element": {"selector": sel, "exists": True}}
        wid = t.window_id or (t.obs.window_id if t.obs else None)
        r = self._tool("verify_state", {"pid": int(t.pid), "window_id": int(wid), "expect": [pred],
                                             "timeout_ms": 0, "stable_samples": 1}, timeout=10)
        status = (r.get("structuredContent") or {}).get("status")
        if status == "satisfied":
            return True
        if status in ("unsatisfied", "unknown") or r.get("isError"):
            return False
        return None

    def _refresh(self, ctx: ActionCtx):
        """Tree-only re-read; returns (new_token or None, element) or None on failure."""
        t = ctx.target
        old = t.element
        wid = t.window_id or (t.obs.window_id if t.obs else None)
        a = {"pid": int(t.pid), "include_screenshot": False, "tree_format": "elements", "max_elements": 600}
        if wid:
            a["window_id"] = int(wid)
        r = self._tool("get_window_state", a, timeout=20)
        o = self.state.record_observation(a, r)
        self.stats["tree_reads"] += 1
        if o is None:
            return None
        o.gate_read = True
        o.window_count = self._window_sig(o.pid) if o.pid is not None else None
        cands = [e for e in o.elements.values() if e.label == old.label and e.role == old.role]
        if len(cands) > 1:
            cands = _nearest(old, cands)
        if len(cands) != 1:
            return (None, None)
        return (f"{o.snapshot_id}:{cands[0].index}", cands[0])

    def _heal(self, ctx: ActionCtx) -> Optional[str]:
        """Re-read the tree (no screenshot) and re-bind a stale token. Returns new token."""
        if ctx.target.element is None or not ctx.target.element.label or ctx.target.pid is None:
            return None
        r = self._refresh(ctx)
        if not r or r[0] is None:
            return None
        self.stats["heals"] += 1
        return r[0]

    # ---------------------------------------------------------------- gating
    def gated(self, name: str, args: dict) -> dict:
        tg0 = time.time()
        ack = bool(args.pop("gate_ack", False))
        code = args.pop("gate_confirm", None)
        ctx = build_ctx(name, args, self.state)
        confirmed = bool(code) and self.confirm_codes.pop(str(code), None) == ctx.signature
        self.stats["actions"] += 1
        self.aid += 1

        wc, wadd = None, None
        if ctx.target.kind == "pixel" or self.state.actions:
            wc, wadd = self._probe_window_changed(ctx)
        f = features(ctx, self.state, window_changed=wc, window_added=wadd)
        if name == "run_steps":
            f["token_stale"] = 0.0  # the driver re-binds stale tokens inside a batch itself
        check_res = None
        shadow = None
        if self.cfg.collect and self._check_available(ctx):
            # shadow label: would the target still resolve right now? (probe on a copy, args untouched)
            # (existence probe only: it must not replace the agent's snapshot)
            shadow = self._check(ctx)

        dec = policy_decision(ctx, f, self.cfg)
        trail = []
        if dec is None:
            dec = hard_rules(ctx, f, self.cfg, confirmed, ack)
            if dec is not None and dec.option == "ask_human" and confirmed:
                dec = None
            if dec is not None and ack and dec.option == "reason":
                dec = None
            if dec is None:
                dec = choose(ctx, f, self.cfg, self._check_available(ctx), check_done=False)
                if ack and dec.option == "reason":
                    dec = Decision("direct", dec.p_direct, "agent acknowledged after double-check", "model", dec.scores)
        if dec.option in ("look_light", "look_full", "reason", "check") and dec.tier not in ("rule", "policy") \
                and f.get("token_stale", 0) < 0.5 and not ctx.risk_level:
            fast = _fast_path(ctx, f)
            if fast:
                dec = Decision("direct", dec.p_direct, fast, "rule", dec.scores)
        # cheap options are resolved here and re-decided (each at most once)
        waited = checked = False
        for _ in range(5):
            trail.append(dec.option)
            if dec.option == "check" and checked:
                dec = Decision("look_light", dec.p_direct, "target still not confirmed after a fresh tree probe", "grey")
                break
            if dec.option == "wait" and waited:
                dec = Decision("look_light", dec.p_direct, "UI still busy after waiting", "grey")
                break
            if dec.option == "check":
                checked = True
                check_res = self._check(ctx, args)
                ctx = build_ctx(name, args, self.state)
                f = features(ctx, self.state, window_changed=wc, window_added=wadd, check=check_res)
                if getattr(self, "_probe_disabled", False):
                    f["loading_cue"] = 1.0
                dec = choose(ctx, f, self.cfg, False, True)
                if ack and dec.option == "reason":
                    dec = Decision("direct", dec.p_direct, "agent acknowledged after double-check", "model", dec.scores)
                continue
            if dec.option == "wait":
                waited = True
                slept = 0.0
                while slept < 1.5:
                    time.sleep(0.25)
                    slept += 0.25
                    if self._check_available(ctx) and self._check(ctx):
                        break
                self.state.actions and setattr(self.state.actions[-1], "unverifiable", False)
                wc, wadd = self._probe_window_changed(ctx)
                f = features(ctx, self.state, window_changed=wc, window_added=wadd)
                f["loading_cue"] = 0.0
                f["last_popup"] = 0.0
                dec = choose(ctx, f, self.cfg, self._check_available(ctx) and not checked, checked)
                if dec.option == "wait":
                    dec = Decision("direct", dec.p_direct, "settled after wait", "model", dec.scores)
                continue
            if dec.option == "heal":
                newtok = self._heal(ctx)
                if newtok:
                    args["element_token"] = newtok
                    ctx = build_ctx(name, args, self.state)
                    f = features(ctx, self.state, window_changed=wc, window_added=wadd)
                    dec = Decision("direct", p_direct(f, self.cfg), f"token re-bound to {newtok}", "rule")
                    ctx.args = args
                    trail.append("healed")
                else:
                    dec = Decision("look_light", 0.0, "stale token and no unique element with the same role+label",
                                   "rule")
                continue
            break
        if dec.option in ("check", "wait", "heal"):
            dec = Decision("look_light", dec.p_direct, "could not settle the target cheaply", "grey")

        # decider=model: what the formula could not settle cheaply goes to the main model itself
        judge = None
        if (self.cfg.policy == "gate" and self.cfg.decider == "model" and dec.tier not in ("rule", "policy")
                and dec.option in ("look_light", "look_full", "reason")):
            if ack:
                self.stats["model_acks"] += 1
                trail.append("model_ack")
                dec = Decision("direct", dec.p_direct, "main model confirmed after gate review", "model_ack", dec.scores)
            else:
                trail.append("model_review")
                judge = {"asked": dec.option}
                dec = Decision("model_review", dec.p_direct, dec.reason, "model_review", dec.scores)
        opt = dec.option
        self.stats["by_option"][opt] = self.stats["by_option"].get(opt, 0) + 1
        gate_ms = (time.time() - tg0) * 1000
        event = {"ev": "action", "aid": self.aid, "tool": name, "policy": self.cfg.policy, "model": self.cfg.model_name,
                 "lam": self.cfg.lam, "target": {"kind": ctx.target.kind, "role": (ctx.target.element.role if ctx.target.element else ctx.target.role)[:40],
                 "label": (ctx.target.element.label if ctx.target.element else ctx.target.label)[:80]},
                 "risk": ctx.risk, "sig": ctx.signature, "features": f, "p_direct": round(dec.p_direct, 4),
                 "decision": opt, "tier": dec.tier, "trail": trail, "reason": dec.reason, "check": check_res,
                 "shadow_check": shadow, "decider": self.cfg.decider, "judge": judge, "window_changed": wc, "window_added": wadd, "ack": ack, "confirmed": confirmed,
                 "risk_level": ctx.risk_level, "handoff": ctx.handoff}

        # a person decides / acts, when the gate can reach one itself
        if opt in ("ask_human", "handoff") and self.human.resolve_channel() != "chat":
            what0 = (ctx.target.element.label if ctx.target.element else ctx.target.label) or name
            if opt == "ask_human":
                q = (f"The agent wants to {name.replace('_', ' ')} \"{what0}\".\nWhy the gate stopped it: {dec.reason}.\n"
                     f"Allow it?")
            else:
                q = (f"The agent needs you: {dec.reason}.\nPlease do this step yourself in the app window, "
                     f"then press Allow. Press Deny to stop the agent here.")
            self.stats["human_asked"] += 1
            h = self.human.ask(q, title="gui-gate: approve action" if opt == "ask_human" else "gui-gate: your turn")
            event["human"] = h
            if h["answer"] == "allow" and opt == "ask_human":
                self.stats["human_allowed"] += 1
                dec = Decision("direct", dec.p_direct, "approved by a person", "human", dec.scores)
                opt = "direct"
                event.update({"decision": "direct", "tier": "human"})
            elif h["answer"] == "allow":
                self.stats["human_allowed"] += 1
                event.update({"executed": False, "gate_ms": round((time.time() - tg0) * 1000, 1)})
                self.log.write(event)
                res = {"content": [{"type": "text", "text": f"[gate] A person did this step themselves ({dec.reason}). "
                                    f"Do not repeat it; continue from the window's current state."}]}
                if ctx.target.pid is not None:
                    self._auto_observe(ctx.target.pid, ctx.target.window_id, res, force=True)
                return res
            else:
                verdict = "declined" if h["answer"] == "deny" else f"did not answer within {int(self.cfg.human_timeout)} s"
                event.update({"executed": False, "gate_ms": round((time.time() - tg0) * 1000, 1)})
                self.log.write(event)
                return _refusal(dec, f"[gate] Not executed: the person {verdict} ({dec.reason}). Do not retry this "
                                     f"action. Stop and tell the user what you wanted to do and why.",
                                {"human": h["answer"]})

        if opt == "direct":
            t0 = time.time()
            res = self._tool(name, args, timeout=120)
            ms = (time.time() - t0) * 1000
            self.stats["driver_ms"] += ms
            self.stats["executed"] += 1
            sc = res.get("structuredContent") or {}
            txt = text_of(res)
            stale = "stale" in txt.lower() and bool(res.get("isError"))
            rec = ActionRecord(time.time(), name, ctx.signature,
                               GateState.wkey(ctx.target.pid, ctx.target.window_id) if ctx.target.pid else None,
                               is_error=bool(res.get("isError")), unverifiable=sc.get("effect") == "unverifiable",
                               popup=bool(sc.get("popup")) or "popup" in txt.lower(), stale_error=stale)
            if rec.is_error:
                self.stats["exec_errors"] += 1
            prev = self.state.last_action()
            if prev is not None and prev.key == rec.key and not prev.is_error and rec.t - prev.t < 60:
                # the agent sent the same action again: the previous one most likely did not do what it wanted
                self.log.write({"ev": "retro_repeat", "aid": getattr(prev, "aid", None)})
            rec.aid = self.aid
            self.state.record_action(rec)
            pid0 = ctx.target.pid
            if pid0 is not None and "x" in args and "y" in args:
                self._pixel_pids.add(pid0)
            if pid0 is not None and rec.is_error and _NO_FRAME.search(txt):
                # the driver lost its pixel frame: hand back a fresh screenshot + tree in this same result
                self._pixel_pids.add(pid0)
                res.setdefault("content", []).append({"type": "text", "text":
                    "[gate] Here is a fresh screenshot of the window; re-aim your x/y on it and retry."})
                self._auto_observe(pid0, ctx.target.window_id, res, force=True)
                self.stats["frame_recoveries"] = self.stats.get("frame_recoveries", 0) + 1
            if pid0 is not None and not rec.is_error:
                if name in ("type_text", "set_value"):
                    self.state.last_typed[int(pid0)] = self.state.last_typed.get(int(pid0), "") + str(args.get("text") or args.get("value") or "")
                elif name in ("press_key", "hotkey"):
                    keys = {str(k).lower() for k in (args.get("keys") or [])} | {str(args.get("key") or "").lower()}
                    if keys & {"return", "enter", "kp_enter"}:
                        self.state.last_typed[int(pid0)] = ""
            event.update({"executed": True, "exec_ms": round(ms, 1), "is_error": rec.is_error,
                          "stale_error": stale, "effect": sc.get("effect"), "popup": rec.popup,
                          "gate_ms": round(gate_ms, 1)})
            self.stats["gate_ms"] += gate_ms
            self.log.write(event)
            if "healed" in trail:
                res.setdefault("content", []).append({"type": "text", "text":
                    f"[gate] your token was stale; re-bound to {args['element_token']} from a fresh tree read. "
                    f"Older tokens for this window are now stale."})
            if (not rec.is_error and name in AUTO_OBS_TOOLS and pid0 is not None and self.cfg.policy == "gate"
                    and self.cfg.auto_observe):
                self._auto_observe(pid0, ctx.target.window_id, res)
            return res

        # an intervention: nothing executed
        self.stats["gate_ms"] += gate_ms
        t = ctx.target
        loc = []
        if t.pid is not None:
            loc.append(f"pid={t.pid}")
        if t.window_id:
            loc.append(f"window_id={t.window_id}")
        loc = ", ".join(loc)
        what = f'"{(t.element.label if t.element else t.label)}"' if (t.element or t.label) else "this target"
        extra = {}
        if opt == "look_light":
            self.stats["model_turns_requested"] += 1
            text = (f"[gate] Not executed: {dec.reason}. Re-read the window without a screenshot "
                    f"(get_window_state {loc}, include_screenshot:false, since:\"latest\") and retry with the fresh token.")
        elif opt == "look_full":
            self.stats["model_turns_requested"] += 1
            if t.pid is not None:
                self._want_shot.add(t.pid)
            text = (f"[gate] Not executed: {dec.reason}. Re-read the window WITH a screenshot "
                    f"(get_window_state {loc}, include_screenshot:true) and check the target before retrying.")
        elif opt == "model_review":
            self.stats["model_turns_requested"] += 1
            self.stats["model_reviews"] += 1
            text = (f"[gate] Not executed: the gate is unsure this will hit {what} ({dec.reason}). You decide: "
                    f"if you are confident the target is still current and correct, call again with the same "
                    f"arguments plus gate_ack:true. Otherwise re-read the window first (get_window_state {loc}; "
                    f"add include_screenshot:true only if you need pixels).")
        elif opt == "reason":
            self.stats["model_turns_requested"] += 1
            text = (f"[gate] Not executed: {dec.reason}. Double-check that {what} is really the element you mean "
                    f"(another element may share the label, or the last attempt had no visible effect). "
                    f"If you are sure, call again with gate_ack:true.")
        elif opt == "risk_review":
            self.stats["model_turns_requested"] += 1
            text = (f"[gate] Not executed: {dec.reason} on {what}. Stop and think before doing this: did the user "
                    f"explicitly ask for exactly this, and can it be undone? If the user did not clearly ask for it, "
                    f"ask them first. Only if you are sure, call the same tool again with the same arguments plus "
                    f"gate_ack:true.")
        elif opt == "handoff":
            self.stats["model_turns_requested"] += 1
            text = (f"[gate] Not executed: {dec.reason}. A person must do this step, not you. Ask the user to do it "
                    f"themselves, wait until they say it is done, then re-read the window and continue.")
        elif opt == "ask_human":
            self.stats["model_turns_requested"] += 1
            c = secrets.token_hex(3)
            self.confirm_codes[c] = ctx.signature
            extra["confirm_code"] = c
            text = (f"[gate] Not executed: {dec.reason} on {what}. Ask the user to approve this exact action. "
                    f"Only after they approve, call the same tool again with the same arguments plus gate_confirm:\"{c}\".")
        else:
            text = f"[gate] Not executed: {dec.reason}."
        event.update({"executed": False, "gate_ms": round(gate_ms, 1)})
        self.log.write(event)
        return _refusal(dec, text, extra)

    # --------------------------------------------------------------- report
    def _auto_observe(self, pid, window_id, res: dict, force: bool = False) -> None:
        """Right after an action: read the window again WITHOUT a screenshot and hand the
        fresh tree back in the same tool result, so the agent needs no separate read turn.
        Waits briefly while the UI says it is loading; reads a newly opened dialog instead."""
        t0 = time.time()
        try:
            pid = int(pid)
            prev = self.state.latest_for(pid, window_id)
            prev_ids = {x.split("@")[0] for x in (prev.window_count or "").split(";") if x} if prev else set()
            time.sleep(0.15)
            md, sc, a, note = "", {}, None, ""
            for attempt in range(4):
                sig = self._window_sig(pid) or ""
                now_ids = [x.split("@")[0] for x in sig.split(";") if x]
                new = [w for w in now_ids if w not in prev_ids] if prev_ids else []
                wid = int(new[-1]) if new else (int(window_id) if window_id else self.state.window_of_pid.get(pid))
                note = f"A new window/dialog opened (window_id={wid}); this is its tree.\n" if new else ""
                shot = pid in self._pixel_pids
                a = {"pid": pid, "include_screenshot": shot}
                if wid:
                    a["window_id"] = wid
                # (a driver diff read, since:"latest", was tried in 0.3.3 and dropped: its answer is not in
                # tree_markdown, and it could not be shown to report changes in unindexed text rows)
                r = self._tool("get_window_state", a, timeout=20)
                sc = r.get("structuredContent") or {}
                md = str(sc.get("tree_markdown") or "")
                if r.get("isError") or not sc.get("snapshot_id"):
                    return
                if _LOADING_RX.search(md) and attempt < 3:
                    a.pop("since", None)
                    time.sleep(0.4)
                    continue
                break
            if "PARTIAL TREE" in text_of(r) and not shot:
                # a grid/canvas the tree cannot show: the agent will aim by pixels, so give it the picture
                self._pixel_pids.add(pid)
                shot = True
                a = dict(a, include_screenshot=True)
                a.pop("since", None)
                r2 = self._tool("get_window_state", a, timeout=20)
                if not r2.get("isError") and (r2.get("structuredContent") or {}).get("snapshot_id"):
                    r, sc = r2, r2.get("structuredContent") or {}
                    md = str(sc.get("tree_markdown") or md)
            o = self.state.record_observation(a, r)
            if o is not None:
                o.window_count = sig
            self._seen_pids.add(pid)
            if a.get("since") or not md.strip():
                # a diff read puts its answer (changed rows / "no change since ...") in the text, not the tree
                body = text_of(r).strip()
                md = "\n".join(x for x in body.splitlines() if not x.startswith(("window_id=", "element_token for row")))
            lines = md.splitlines()
            more = ""
            if len(lines) > MAX_TREE_LINES:
                more = f"\n... ({len(lines) - MAX_TREE_LINES} more rows; call get_window_state for the full tree)"
                lines = lines[:MAX_TREE_LINES]
            busy = " The UI still looks busy." if _LOADING_RX.search(md) else ""
            res.setdefault("content", []).append({"type": "text", "text":
                f"[gate] Window right after the action (snapshot {sc.get('snapshot_id')}; tokens are "
                f"{sc.get('snapshot_id')}:N, older tokens are stale).{busy}\n{note}" + "\n".join(lines) + more})
            if shot:
                res["content"].extend(c for c in (r.get("content") or []) if c.get("type") == "image")
            self.stats["auto_obs"] += 1
            self.log.write({"ev": "obs", "tool": "get_window_state", "auto": True, "ms": round((time.time() - t0) * 1000, 1),
                            "screenshot": shot, "px": 0, "snapshot": sc.get("snapshot_id"), "pid": pid,
                            "trimmed": not shot, "new_window": bool(note)})
        except Exception as e:  # never break the action result
            log_err("auto-observe failed", repr(e))

    def request_human(self, args: dict) -> dict:
        q = str(args.get("question") or "").strip() or "The agent needs your help."
        kind = args.get("kind") or "answer"
        ch = self.human.resolve_channel()
        self.stats["human_asked"] += 1
        if ch == "chat":
            return {"content": [{"type": "text", "text": "[gate] There is no direct line to a person from here. Ask "
                                 "your user in your reply and wait for their answer."}]}
        text = q if kind != "takeover" else q + "\n\nDo it in the app yourself, then press Allow when you are done."
        h = self.human.ask(text, want_text=(kind == "answer"),
                           title={"approve": "gui-gate: approve?", "takeover": "gui-gate: your turn"}.get(kind, "gui-gate: question"))
        self.log.write({"ev": "request_human", "kind": kind, "question": q[:200], "human": h})
        ans = h["answer"]
        if ans == "timeout":
            msg = f"No answer from a person within {int(self.cfg.human_timeout)} s. Do not guess; stop and report."
        elif kind == "takeover":
            msg = "The person says the step is done. Re-read the window and continue." if ans == "allow" else \
                  "The person declined to take over. Stop and report."
        elif kind == "approve":
            msg = "Approved." if ans == "allow" else "Not approved. Do not do it."
        else:
            msg = f"The person answered: {ans}" if ans not in ("deny",) else "The person cancelled. Stop and report."
        return {"content": [{"type": "text", "text": f"[human via {h['channel']}] {msg}"}],
                "structuredContent": {"answer": ans, "channel": h["channel"], "ms": h["ms"]}}

    def report(self) -> dict:
        s = dict(self.stats)
        s["by_option"] = dict(s["by_option"])
        s["session_s"] = round(time.time() - self.started, 1)
        s["policy"] = self.cfg.policy
        s["lambda"] = self.cfg.lam
        s["model"] = self.cfg.model_name
        s["approx_image_tokens"] = int(s["screenshot_px"] / 750)
        s["gate_ms_per_action"] = round(s["gate_ms"] / s["actions"], 1) if s["actions"] else 0
        s["log"] = self.log.path
        s["decider"] = self.cfg.decider
        lines = [
            f"policy={s['policy']} lambda={s['lambda']} model={s['model']}",
            f"actions={s['actions']} executed={s['executed']} errors={s['exec_errors']}",
            f"decisions={s['by_option']}",
            f"observations={s['obs_calls']} (screenshots={s['screenshots']}, ~{s['approx_image_tokens']} image tokens)",
            f"extra model turns asked by gate={s['model_turns_requested']} | cheap checks={s['checks']} heals={s['heals']}",
            f"gate overhead={s['gate_ms_per_action']} ms/action",
            f"human: asked={s['human_asked']} allowed={s['human_allowed']} channel={self.human.resolve_channel()} | "
            f"auto observations={s['auto_obs']} | driver restarts={self.restarts}",
            f"decider={s['decider']} reviews by main model={s['model_reviews']} (acked {s['model_acks']}) | screenshots skipped={s['screenshots_skipped']}",
        ]
        return {"content": [{"type": "text", "text": "\n".join(lines)}], "structuredContent": s}

    def config(self, args: dict) -> dict:
        if "policy" in args:
            self.cfg.policy = args["policy"]
        if "lambda" in args:
            self.cfg.lam = float(args["lambda"])
        if "collect" in args:
            self.cfg.collect = bool(args["collect"])
        if "risk_confirm" in args:
            self.cfg.risk_confirm = bool(args["risk_confirm"])
        if args.get("decider") in ("formula", "model"):
            self.cfg.decider = args["decider"]
        if args.get("risk_mode") in ("auto", "model", "human"):
            self.cfg.risk_mode = args["risk_mode"]
        self.log.write({"ev": "config", "policy": self.cfg.policy, "lam": self.cfg.lam, "collect": self.cfg.collect})
        d = {"policy": self.cfg.policy, "lambda": self.cfg.lam, "collect": self.cfg.collect,
             "risk_confirm": self.cfg.risk_confirm, "model": self.cfg.model_name,
             "decider": self.cfg.decider, "risk_mode": self.cfg.risk_mode}
        return {"content": [{"type": "text", "text": json.dumps(d)}], "structuredContent": d}


def main(argv=None) -> None:
    ap = argparse.ArgumentParser(prog="gui-gate", description="Cua Driver behind a pre-action value gate (MCP stdio server)")
    ap.add_argument("--policy", choices=["gate", "direct", "light", "full"])
    ap.add_argument("--lambda", dest="lam", type=float)
    ap.add_argument("--collect", action="store_true", default=None, help="shadow-label every action for training")
    ap.add_argument("--no-risk-confirm", dest="risk_confirm", action="store_false", default=None)
    ap.add_argument("--decider", choices=["formula", "model"], help="who settles the grey zone (model = the main model)")
    ap.add_argument("--risk-mode", choices=["auto", "model", "human"],
                    help="auto (default): high risk -> person, medium -> main model; model: all -> model; human: all -> person")
    ap.add_argument("--human", dest="human_channel", choices=["auto", "elicit", "desktop", "chat", "deny"],
                    help="how to reach a person (default auto: MCP elicitation, else desktop dialog, else via the agent)")
    ap.add_argument("--human-timeout", type=float, help="seconds to wait for a person (default 120)")
    ap.add_argument("--delivery", choices=["auto", "background", "foreground"],
                    help="input delivery: auto (default; background, falls back to foreground when the desktop "
                         "has no focus-free backend), background, or foreground")
    ap.add_argument("--toolset", choices=["core", "all"], help="core (default): only the tools a GUI task needs")
    ap.add_argument("--no-auto-observe", dest="auto_observe", action="store_false", default=None,
                    help="do not return the fresh window tree after each action")
    ap.add_argument("--weights", help="trained weights JSON (from gui-gate-train)")
    ap.add_argument("--log", help="JSONL log path (default ~/.gui-gate/logs/<time>.jsonl; 'none' disables)")
    ap.add_argument("--driver", help="path to cua-driver (default: PATH or ~/.local/bin)")
    ap.add_argument("--driver-arg", action="append", default=[], help="extra args for `cua-driver mcp`")
    ap.add_argument("--child", help=argparse.SUPPRESS)  # test hook: full command of a fake MCP server
    ap.add_argument("--version", action="version", version=__version__)
    a = ap.parse_args(argv)
    cfg = GateConfig.from_env(policy=a.policy, lam=a.lam, collect=a.collect, risk_confirm=a.risk_confirm,
                              decider=a.decider, risk_mode=a.risk_mode, human_channel=a.human_channel,
                              human_timeout=a.human_timeout, toolset=a.toolset, auto_observe=a.auto_observe,
                              delivery=a.delivery)
    if a.weights:
        cfg.load_weights(a.weights)
    if a.child:
        cmd = a.child.split()
    else:
        drv = a.driver or find_driver()
        if not drv:
            log_err("cua-driver not found. Install it with: curl -fsSL https://cua.ai/driver/install.sh | bash")
            sys.exit(2)
        cmd = [drv, "mcp", *a.driver_arg]
    log_path = None if (a.log or "").lower() == "none" else (a.log or os.environ.get("GUI_GATE_LOG") or "auto")
    Gate(cmd, cfg, log_path).serve()


if __name__ == "__main__":
    main()
