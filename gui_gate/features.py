"""Turn one pending action into runtime-only features.

Everything here is derivable from what the agent and the driver already
exchanged (plus cheap gate-side probes). No generator ground truth.
"""
from __future__ import annotations

import math
import re

from . import sidefx
import time
from dataclasses import dataclass, field
from typing import Optional

from .state import Element, GateState, Observation

TOKEN_RE = re.compile(r"^(s[0-9a-f]{8}):(\d+)$")

# Tools the gate evaluates before they reach the driver.
GATED_TOOLS = {
    "click", "double_click", "right_click", "type_text", "set_value", "press_key",
    "hotkey", "scroll", "drag", "run_steps", "mouse_button_down", "invoke_menu",
    "browser_click", "browser_type", "browser_pointer", "browser_select",
    "browser_navigate", "browser_download", "browser_set_input_files",
    "kill_app", "run_script",
}
OBS_TOOLS = {"get_window_state", "get_desktop_state", "zoom", "list_windows", "get_browser_state"}
POINTER_TOOLS = {"click", "double_click", "right_click", "drag", "mouse_button_down", "scroll"}
KEY_TOOLS = {"type_text", "press_key", "hotkey", "set_value"}

# Two risk levels, judged from what the action touches (no model involved).
#   high   irreversible or costs money / leaks data -> a person decides
#   medium consequential but usually recoverable   -> the agent must re-think and re-send with gate_ack
RISK_HIGH = [
    "delete", "erase", "wipe", "pay", "pay now", "purchase", "buy now", "place order", "checkout", "confirm payment",
    "transfer", "uninstall", "format disk", "format drive", "format partition", "format volume", "factory reset", "empty trash", "permanently", "delete account",
    "close account", "remove account", "reset password",
    "删除", "清空", "支付", "付款", "购买", "立即购买", "下单", "转账", "卸载", "格式化", "永久", "注销账号", "恢复出厂",
]
RISK_MED = [
    "remove", "send", "submit", "publish", "post", "sign out", "log out", "logout", "overwrite", "replace all",
    "discard", "don't save", "revoke", "authorize", "allow", "grant", "accept", "shut down", "shutdown", "restart",
    "reboot", "install", "share", "invite", "move to trash", "order now",
    "移除", "发送", "提交", "发布", "退出登录", "覆盖", "不保存", "撤销", "授权", "允许", "关机", "重启", "安装", "分享", "邀请",
]
RISK_HOTKEYS_HIGH = [{"shift", "delete"}, {"ctrl", "alt", "delete"}]
RISK_HOTKEYS_MED = [{"ctrl", "enter"}, {"ctrl", "return"}, {"alt", "f4"}, {"ctrl", "w"}, {"ctrl", "q"},
                    {"ctrl", "shift", "w"}, {"super", "l"}]
# typed text that is dangerous once it is submitted (terminals, consoles, SQL boxes)
_SHELL_HIGH = re.compile(r"(\brm\s+-[a-z]*[rf]|\bmkfs\b|\bdd\s+if=|\bshutdown\b|\breboot\b|:\(\)\s*\{|\bchmod\s+-R\s+777"
                         r"|\bdrop\s+(table|database)\b|\btruncate\s+table\b|\bdelete\s+from\b|git\s+push\s+.*--force"
                         r"|\bcurl\b[^|]*\|\s*(ba)?sh|\bsudo\b|>\s*/dev/sd|\bformat\s+[a-z]:)", re.I)
_SECRET_FIELD = re.compile(r"(password|passcode|passwd|\bpin\b|密码|口令|cvv|cvc|security code|card number|卡号|验证码"
                           r"|one-time code|otp|2fa|verification code)", re.I)
_CAPTCHA = re.compile(r"(captcha|recaptcha|hcaptcha|i'?m not a robot|verify you are human|人机验证|滑块验证|安全验证)", re.I)
_OPENER_ROLE = re.compile(r"^(menu|menu ?bar|menubar|menu bar item|tab|page tab|tab list|combo ?box|heading|label|static text|text)$")
_ENTER_KEYS = {"return", "enter", "kp_enter"}

RISK_WORDS = RISK_HIGH + RISK_MED  # back-compat


def _rx(words):
    return [(w, re.compile(r"\b" + re.escape(w) + r"\b") if w.isascii() else None) for w in words]


_HIGH_RES, _MED_RES = _rx(RISK_HIGH), _rx(RISK_MED)


def _match(s: str, res) -> Optional[str]:
    for w, rx in res:
        if (rx.search(s) if rx else w in s):
            return w
    return None


def risky_text(s: str) -> Optional[str]:
    s = (s or "").lower()
    if not s:
        return None
    return _match(s, _HIGH_RES) or _match(s, _MED_RES)


def risk_of_label(s: str) -> tuple[Optional[str], Optional[str]]:
    s = (s or "").lower().strip()
    if not s:
        return None, None
    w = _match(s, _HIGH_RES)
    if w:
        return "high", w
    w = _match(s, _MED_RES)
    if w:
        return "medium", w
    return None, None


@dataclass
class Target:
    kind: str  # token | index | pixel | name | keyboard | none
    pid: Optional[int] = None
    window_id: Optional[int] = None
    snapshot: Optional[str] = None
    index: Optional[int] = None
    x: Optional[float] = None
    y: Optional[float] = None
    role: str = ""
    label: str = ""
    element: Optional[Element] = None
    obs: Optional[Observation] = None


@dataclass
class ActionCtx:
    tool: str
    args: dict
    target: Target
    signature: str
    risk: Optional[str]
    steps: list = field(default_factory=list)  # for run_steps: list[(tool,args,Target)]
    risk_level: Optional[str] = None  # high | medium | None
    handoff: Optional[str] = None     # why a person (not the model) should act: secret entry, captcha


def _signature(tool: str, t: Target, args: dict) -> str:
    if t.kind in ("token", "index"):
        lab = t.element.label if t.element else ""
        role = t.element.role if t.element else ""
        return f"{tool}|{t.pid}|{role}|{lab}"
    if t.kind == "pixel":
        return f"{tool}|{t.window_id}|{round(t.x or 0, -1)}|{round(t.y or 0, -1)}"
    if t.kind == "name":
        return f"{tool}|{t.role}|{t.label}"
    keys = args.get("keys") or args.get("key") or args.get("text") or args.get("value") or ""
    return f"{tool}|{t.pid}|{keys}"


def resolve_target(tool: str, args: dict, st: GateState) -> Target:
    tok = args.get("element_token")
    pid = args.get("pid")
    wid = args.get("window_id")
    tgt = args.get("target") if isinstance(args.get("target"), dict) else None
    if tgt and tgt.get("kind") == "window":
        pid = pid or tgt.get("pid")
        wid = wid or tgt.get("window_id")
    if isinstance(tok, str) and TOKEN_RE.match(tok):
        snap, idx = TOKEN_RE.match(tok).groups()
        obs = st.obs_by_snapshot.get(snap)
        el = obs.elements.get(int(idx)) if obs else None
        return Target("token", pid or (obs.pid if obs else None), wid or (obs.window_id if obs else None),
                      snap, int(idx), role=el.role if el else "", label=el.label if el else "",
                      element=el, obs=obs)
    if args.get("element_index") is not None:
        obs = st.latest_for(pid, wid)
        idx = int(args["element_index"])
        el = obs.elements.get(idx) if obs else None
        return Target("index", pid, wid or (obs.window_id if obs else None), obs.snapshot_id if obs else None,
                      idx, role=el.role if el else "", label=el.label if el else "", element=el, obs=obs)
    if args.get("x") is not None or args.get("from_x") is not None:
        obs = st.latest_for(pid, wid)
        return Target("pixel", pid or (obs.pid if obs else None), wid, x=args.get("x", args.get("from_x")),
                      y=args.get("y", args.get("from_y")), obs=obs)
    if args.get("role") or args.get("name"):
        obs = st.latest_for(pid, wid)
        return Target("name", pid, wid, role=str(args.get("role") or ""), label=str(args.get("name") or ""), obs=obs)
    if tool in KEY_TOOLS:
        obs = st.latest_for(pid, wid)
        return Target("keyboard", pid, wid, obs=obs)
    return Target("none", pid, wid, obs=st.latest_for(pid, wid))


def build_ctx(tool: str, args: dict, st: GateState) -> ActionCtx:
    steps = []
    if tool == "run_steps":
        for s in args.get("steps") or []:
            if not isinstance(s, dict):
                continue
            if "tool" in s:
                stool, sargs = s.get("tool"), dict(s.get("args") or {})
            else:
                keys = [k for k in s if k not in ("wait_for", "expect", "timeout_ms")]
                if not keys:
                    continue
                stool, sargs = keys[0], dict(s.get(keys[0]) or {})
            for k in ("pid", "window_id"):
                if k not in sargs and k in args:
                    sargs[k] = args[k]
            steps.append((stool, sargs, resolve_target(stool, sargs, st)))
        t = steps[0][2] if steps else Target("none")
    else:
        t = resolve_target(tool, args, st)
    risk, level, handoff = assess_risk(tool, args, t, steps, st)
    return ActionCtx(tool, args, t, _signature(tool, t, args), risk, steps, level, handoff)


def _newest_obs_blob(st: GateState, pid) -> str:
    """Text of the newest read of any window of this process (a dialog that just opened, usually)."""
    if pid is None:
        return ""
    cut = time.time() - 30  # an old dialog read says nothing about the window now
    obs = [o for k, o in st.latest_by_window.items() if k.split(":")[0] == str(pid) and o.t >= cut]
    if not obs:
        return ""
    return _obs_blob(max(obs, key=lambda x: x.t))


def _newest_obs_text(st: GateState, pid) -> str:
    cut = time.time() - 30
    obs = [o for k, o in st.latest_by_window.items() if k.split(":")[0] == str(pid) and o.t >= cut]
    return max(obs, key=lambda x: x.t).text if obs else ""


def _obs_blob(o) -> str:
    return (o.text or "")[:8000] + " " + " ".join(e.label for e in o.elements.values())[:4000]


def assess_risk(tool: str, args: dict, t: Target, steps: list, st: GateState):
    """(reason, level, handoff) from what the action touches. Rules only, plus file-system checks
    (sidefx) for commands typed into a terminal."""
    best = [None, None]  # reason, level

    def bump(level, reason):
        if level == "high" or best[1] is None:
            if best[1] != "high":
                best[0], best[1] = reason, level

    items = [(tool, args, t)] + [(a, b, c) for a, b, c in steps]
    handoff = None
    for tl, a, tg in items:
        label = (tg.element.label if tg.element else tg.label) or ""
        role = (tg.element.role if tg.element else tg.role) or ""
        if tl in ("click", "double_click", "browser_click", "invoke_menu", "set_value") or tg.kind in ("token", "index", "name"):
            lv, w = risk_of_label(label)
            rl = role.lower()
            if lv and _OPENER_ROLE.search(rl):
                lv = None  # opening a menu, tab or list shows choices; it changes nothing
            elif lv == "high" and "menu" in rl:
                lv = "medium"  # an app command from a menu (Delete Rows, ...) is usually undoable
            if lv and tl not in ("type_text",):
                bump(lv, f'"{label[:40]}" ({w})')
        text = str(a.get("text") or a.get("value") or "")
        if tl in ("type_text", "set_value", "browser_type") and text:
            if _SECRET_FIELD.search(role + " " + label) or "password" in role.lower():
                handoff = handoff or f'typing into a secret field "{label[:40] or role}"'
            if _SHELL_HIGH.search(text):
                bump("medium", f"typed a dangerous command ({_SHELL_HIGH.search(text).group(0)[:30]})")
        if tl in ("press_key", "hotkey"):
            ks = {str(k).lower() for k in (a.get("keys") or [])}
            if a.get("key"):
                ks |= {x.strip().lower() for x in str(a["key"]).split("+")}
            if any(h <= ks for h in RISK_HOTKEYS_HIGH):
                bump("high", "+".join(sorted(ks)))
            elif any(h <= ks for h in RISK_HOTKEYS_MED):
                bump("medium", "+".join(sorted(ks)))
            if ks & _ENTER_KEYS and tg.pid is not None:
                typed = st.last_typed.get(int(tg.pid), "") if hasattr(st, "last_typed") else ""
                if typed and _SHELL_HIGH.search(typed):
                    bump("high", f"Enter would run: {typed[:60]}")
                _shell_check(typed, tg.pid, bump)
            if tg.pid is not None and sidefx.overwrite_dialog(_newest_obs_blob(st, tg.pid), "", ks):
                bump("high", "confirms replacing an existing file")
        if tl in ("type_text", "set_value") and tg.pid is not None and "\n" in text:
            # a newline submits the line: check what it runs
            prev = st.last_typed.get(int(tg.pid), "") if hasattr(st, "last_typed") else ""
            _shell_check(prev + text.split("\n")[0], tg.pid, bump)
        if tl in ("click", "double_click") and label and tg.pid is not None:
            blob = _obs_blob(tg.obs) if tg.obs else _newest_obs_blob(st, tg.pid)
            if sidefx.overwrite_dialog(blob, label, set()):
                bump("high", f'"{label[:30]}" confirms replacing an existing file')
        if tl in ("click", "double_click", "press_key") and tg.pid is not None:
            lab = label.strip().lower().replace("_", "").replace("&", "").rstrip(".")
            enter = tl == "press_key" and bool({str(k).lower() for k in (a.get("keys") or [])} | {str(a.get("key") or "").lower()}
                                                & {"return", "enter", "kp_enter"})
            if lab in sidefx.SAVE_LABELS or enter:
                src = (tg.obs.text if (tg.obs and tl != "press_key") else "") or _newest_obs_text(st, tg.pid)
                hit = sidefx.save_dialog_target(src) if ("dialog" in src[:400] or "Save" in src or "Export" in src) else None
                if hit:
                    bump("high", f"saving here would replace the existing file {hit}")
        if tl in ("click", "double_click") and not label and tg.pid is not None and tg.kind == "pixel":
            if sidefx.OVERWRITE_CTX.search(_newest_obs_blob(st, tg.pid)):
                bump("medium", "pixel click inside a dialog that asks to replace an existing file")
    if tool == "kill_app":
        bump("medium", "kill_app")
    if tool in ("browser_download", "browser_set_input_files"):
        bump("medium", tool)
    if tool == "run_script":
        code = str(args.get("script") or args.get("code") or "")
        if _SHELL_HIGH.search(code) or risky_text(code):
            bump("high", "run_script with destructive content")
    # a captcha / human-verification screen: the model must not try to solve it
    obs = t.obs
    if obs is not None and handoff is None and tool not in ("press_key",):
        blob = " ".join(e.label for e in obs.elements.values())[:4000]
        if _CAPTCHA.search(blob):
            handoff = "the window shows a human-verification (captcha) step"
    return best[0], best[1], handoff



def _shell_check(cmd: str, pid, bump) -> None:
    if not cmd or not sidefx.is_terminal(pid):
        return
    r = sidefx.shell_effects(cmd, sidefx.shell_cwd(pid))
    if r:
        bump(r[0], r[1])


FEATURE_NAMES = [
    "bias", "is_pixel", "is_token", "is_name", "is_keyboard", "no_obs", "token_stale",
    "log_obs_age", "acts_since_obs", "obs_had_shot", "last_err", "last_unverif",
    "last_popup", "repeat", "edge_close", "small_target", "label_empty", "label_dup",
    "loading_cue", "window_changed", "window_added", "elem_unknown", "check_pass", "check_fail",
]


def features(ctx: ActionCtx, st: GateState, window_changed: Optional[bool] = None, window_added: Optional[bool] = None,
             check: Optional[bool] = None, now: Optional[float] = None) -> dict[str, float]:
    now = now or time.time()
    t = ctx.target
    obs = t.obs
    wk = GateState.wkey(t.pid, t.window_id) if t.pid is not None else None
    latest = st.latest_for(t.pid, t.window_id) if t.pid is not None else None
    f = {k: 0.0 for k in FEATURE_NAMES}
    f["bias"] = 1.0
    f["is_pixel"] = float(t.kind == "pixel")
    f["is_token"] = float(t.kind in ("token", "index"))
    f["is_name"] = float(t.kind == "name")
    f["is_keyboard"] = float(t.kind == "keyboard")
    ref = latest or obs
    f["no_obs"] = float(ref is None)
    if t.kind in ("token", "index") and t.snapshot and latest is not None:
        f["token_stale"] = float(latest.snapshot_id != t.snapshot)
    if ref is not None:
        age = max(0.0, now - ref.t)
        f["log_obs_age"] = math.log1p(min(age, 300.0))
        f["acts_since_obs"] = float(min(st.actions_since(ref.t, wk), 5))
        f["obs_had_shot"] = float(ref.with_screenshot)
        f["loading_cue"] = float(ref.loading_cue)
    la = st.last_action()
    if la is not None:
        f["last_err"] = float(la.is_error)
        f["last_unverif"] = float(la.unverifiable)
        f["last_popup"] = float(la.popup)
    f["repeat"] = float(min(st.repeat_count(ctx.signature, since=ref.t if ref is not None else 0.0), 3))
    if t.kind == "pixel" and ref is not None and ref.shot_w and ref.shot_h and t.x is not None:
        m = min(t.x, t.y, ref.shot_w - t.x, ref.shot_h - t.y)
        f["edge_close"] = float(m < 0.03 * min(ref.shot_w, ref.shot_h))
    el = t.element
    if el is not None and el.frame:
        w, h = el.frame.get("w", 0), el.frame.get("h", 0)
        f["small_target"] = float(w * h < 18 * 18)
    if t.kind in ("token", "index", "name"):
        lab = el.label if el else t.label
        f["label_empty"] = float(not lab)
        f["elem_unknown"] = float(el is None and t.kind != "name")
        if ref is not None and lab:
            dup = sum(1 for e in ref.elements.values() if e.label == lab and (not el or e.role == el.role))
            f["label_dup"] = float(dup > 1)
    if window_changed is not None:
        f["window_changed"] = float(window_changed)
    if window_added is not None:
        f["window_added"] = float(window_added)
    if check is True:
        f["check_pass"] = 1.0
    elif check is False:
        f["check_fail"] = 1.0
    return f
