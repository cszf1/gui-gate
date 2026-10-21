"""Authorization check (design-v0.4 T2): does the user's own request cover this risky action?

Borrowed from the user-intent rule of Claude Code's auto mode: only what the user explicitly asked for is
authorized; anything the agent chose on its own is not. Deterministic, no model.

Two ways an action becomes authorized:
  * working-document exception: replacing the very file that is open in this app (saving a document onto
    itself) - the GUI analogue of auto mode letting in-project edits through;
  * explicit wording: the task names the hazard (overwrite / replace / delete / send / log out ...) and, when
    the action has a concrete file target, also names that file.

Also: a hazard class per risk reason, used to pick a safe alternative when the action is denied
(deny-and-continue) and to remember denied effects (no re-asking through another route).
"""
from __future__ import annotations

import os
import re
from typing import Optional

# hazard class -> words in the user's request that explicitly authorize it
AUTH_WORDS = {
    "overwrite": ["overwrite", "replace", "replacing", "overwriting", "覆盖", "替换", "替代"],
    "delete": ["delete", "remove", "erase", "wipe", "clear", "empty", "删除", "移除", "清空", "清除"],
    "send": ["send", "submit", "publish", "post", "share", "发送", "提交", "发布", "分享"],
    "logout": ["log out", "sign out", "logout", "退出登录", "注销"],
    "purchase": ["buy", "purchase", "pay", "order", "checkout", "购买", "支付", "下单", "付款"],
    "power": ["shut down", "shutdown", "restart", "reboot", "关机", "重启"],
    "install": ["install", "uninstall", "安装", "卸载"],
    "permissions": ["chmod", "permission", "permissions", "权限"],
}

_FILE_IN_REASON = re.compile(r"(?:replace the existing file|overwrite existing file\(s\):)\s*([^\s,()]+)")
_QUOTED_FILE = re.compile(r"[\"'“‘]([^\"'“”‘’\n]{1,200}\.[A-Za-z0-9]{1,6})[\"'”’]")
_NAME_IN_DIALOG = re.compile(r"(?:file named|named|文件)\s*[\"'“‘]?([^\"'“”‘’\n]{1,200}?\.[A-Za-z0-9]{1,6})", re.I)


# hazards a request typed into gate_task by the agent itself cannot authorize (only --task / GUI_GATE_TASK can):
# the agent could misquote the user, and these are hard to undo or reach other people
OPERATOR_ONLY = {"send", "purchase", "permissions", "power", "install", "logout", "command"}


def hazard_of(reason: Optional[str]) -> str:
    r = (reason or "").lower()
    if any(k in r for k in ("replac", "overwrite", "覆盖", "clobber")):
        return "overwrite"
    if "chmod" in r:
        return "permissions"
    if any(k in r for k in ("delete", "rm ", "erase", "wipe", "remove", "trash", "删除", "清空", "find -exec rm")):
        return "delete"
    if any(k in r for k in ("log out", "sign out", "logout", "退出登录")):
        return "logout"
    if any(k in r for k in ("pay", "purchase", "buy", "order", "checkout", "支付", "购买", "下单")):
        return "purchase"
    if any(k in r for k in ("send", "submit", "publish", "post", "share", "发送", "提交", "发布")):
        return "send"
    if any(k in r for k in ("shut", "reboot", "restart", "关机", "重启")):
        return "power"
    if "install" in r:
        return "install"
    if "typed a dangerous command" in r or "enter would run" in r:
        return "command"
    return "other"


def target_file(reason: Optional[str], dialog_text: str = "") -> Optional[str]:
    """The file a risky action would replace, from the risk reason or the dialog's own words."""
    m = _FILE_IN_REASON.search(reason or "")
    if m:
        return m.group(1).rstrip(".")
    for rx in (_NAME_IN_DIALOG, _QUOTED_FILE):
        m = rx.search(dialog_text or "")
        if m:
            return m.group(1).strip()
    return None


def _norm(s: str) -> str:
    return re.sub(r"\s+", " ", (s or "").lower()).strip()


def is_open_document(path: Optional[str], window_titles: list[str]) -> bool:
    """True when `path` (or its basename) is the document shown in one of the app's window titles,
    e.g. "report.xlsx - LibreOffice Calc"."""
    if not path:
        return False
    base = os.path.basename(path).lower()
    if not base or "." not in base:
        return False
    for t in window_titles or []:
        tl = (t or "").lower()
        # the title starts with the document name ("name.ext - App" / "name.ext — App")
        head = re.split(r"\s+[-–—]\s+", tl, maxsplit=1)[0].strip()
        if head == base or head.endswith("/" + base):
            return True
    return False


def authorized(reason: Optional[str], task: str, label: str = "", dialog_text: str = "",
               window_titles: Optional[list] = None) -> tuple[bool, str]:
    """(authorized?, why). Never authorizes when there is no task text, except the working document."""
    hz = hazard_of(reason)
    tf = target_file(reason, dialog_text)
    if hz == "overwrite" and is_open_document(tf, window_titles or []):
        return True, f"replaces the document that is open in this app ({os.path.basename(tf)})"
    t = _norm(task)
    if not t:
        return False, "no user request known to the gate"
    words = AUTH_WORDS.get(hz)
    if not words:
        return False, "this kind of action is never authorized by wording alone"
    hit = next((w for w in words if (re.search(r"\b" + re.escape(w) + r"\b", t) if w.isascii() else w in t)), None)
    if not hit:
        return False, f"the request does not ask to {hz}"
    if tf:
        base = os.path.basename(tf).lower()
        stem = os.path.splitext(base)[0]
        if base not in t and not (stem and len(stem) >= 3 and stem in t):
            return False, f"the request says \"{hit}\" but does not name {os.path.basename(tf)}"
    return True, f"the request explicitly says \"{hit}\""


# deny-and-continue: what the agent can do instead, without a person
ALTERNATIVES = {
    "overwrite": ("do not replace it: answer No/Cancel in the dialog, then save or export under a new name that "
                  "does not exist yet (for example add _1 before the extension) in the same folder, finish the task "
                  "with that file, and mention both paths in your final answer"),
    "delete": ("do not delete it: leave it in place (or move it to a clearly named backup folder only if the task "
               "needs the space) and continue with the rest of the task"),
    "permissions": ("narrow the command so it touches only what the task names (for example `find ... -type f` "
                    "instead of a recursive change on directories), list the matches first, then run it"),
    "command": ("run a read-only version first (list what it would touch, e.g. `ls` / `find ... -print`), then a "
                "narrower command that touches only what the task names"),
    "send": "stop before the final send/submit: leave it as a draft and say what is ready in your final answer",
    "purchase": "stop before paying or ordering and say what is ready in your final answer",
    "logout": "do not log out; finish the task in the current session",
    "power": "do not shut down or restart; finish the rest of the task",
    "install": "do not install or uninstall anything; finish the task with what is available",
    "other": "find a way to finish the task that avoids this action",
}

# cp/mv onto an existing file: the alternative is concrete
ALTERNATIVES_CP = ("do not overwrite it: copy to the other destinations, and for the conflicting one use a name "
                   "that does not exist yet (e.g. `cp -n`, or `cp file dir/file.copy`), then report it")


def alternative(reason: Optional[str]) -> str:
    r = reason or ""
    if r.startswith("`cp`") or r.startswith("`mv`") or "would overwrite existing file" in r:
        return ALTERNATIVES_CP
    return ALTERNATIVES.get(hazard_of(r), ALTERNATIVES["other"])


def effect_key(reason: Optional[str], dialog_text: str = "") -> str:
    """What the action would do, independent of the route (dialog button, Enter key, terminal)."""
    hz = hazard_of(reason)
    tf = target_file(reason, dialog_text)
    if tf:
        return f"{hz}:{os.path.basename(tf).lower()}"
    return f"{hz}:{_norm(reason)[:80]}"
