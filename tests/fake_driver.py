"""A tiny stand-in for `cua-driver mcp`, used only by the unit tests.

It mimics the parts of Cua Driver's contract the gate relies on: snapshot ids,
element tokens that go stale after the next read, verify_state, list_windows,
click results. `fake_mutate` lets a test change the UI between calls.
"""
import json
import os
import sys

PID, WID = 4242, 777
state = {
    "snap": 0,
    "elements": [("button", "Increment"), ("button", "Delete file"), ("text", "count=0"), ("button", "OK"), ("button", "OK")],
    "windows": [{"window_id": WID, "pid": PID, "x": 0, "y": 0, "width": 500, "height": 300, "is_on_screen": True}],
    "fail_next": 0,
    "count": 0,
}
TOOLS = [{"name": n, "inputSchema": {"type": "object", "properties": {"pid": {"type": "integer"}}}}
         for n in ["get_window_state", "click", "type_text", "press_key", "hotkey", "list_windows", "verify_state",
                   "run_steps", "fake_mutate", "launch_app", "fake_crash"]]


def snap_id(n):
    return f"s{n:08x}"


def text(t, err=False, sc=None, img=False):
    r = {"content": [{"type": "text", "text": t}]}
    if img:
        r["content"].append({"type": "image", "data": "AAAA", "mimeType": "image/png"})
    if err:
        r["isError"] = True
    if sc is not None:
        r["structuredContent"] = sc
    return r


def call(name, a):
    if name == "get_window_state":
        state["snap"] += 1
        sid = snap_id(state["snap"])
        rows = "\n".join(f'    - [{i}] {r} "{l}" [actions=[click]]' for i, (r, l) in enumerate(state["elements"]))
        md = f'- frame = "Fixture"\n{rows}\n'
        shot = a.get("include_screenshot", True)
        sc = {"snapshot_id": sid, "pid": PID, "window_id": WID, "tree_markdown": md, "truncated": False,
              "screenshot_width": 500 if shot else None, "screenshot_height": 300 if shot else None}
        if shot:
            sc["capture_id"] = "cap_" + sid
        if a.get("tree_format") == "elements":
            sc["elements"] = [{"element_index": i, "element_token": f"{sid}:{i}", "role": r, "label": l,
                               "enabled": True, "frame": {"x": 10, "y": 10 + 40 * i, "w": 200, "h": 34}}
                              for i, (r, l) in enumerate(state["elements"])]
        return text(f"window_id={WID} pid={PID} snapshot_id={sid}\n{md}", sc=sc, img=shot)
    if name == "click":
        tok = a.get("element_token")
        if tok:
            sid, idx = tok.split(":")
            if sid != snap_id(state["snap"]):
                return text("element_token is stale or unknown; call get_window_state again to refresh.", err=True,
                            sc={"refusal": "stale"})
            if state["fail_next"]:
                state["fail_next"] -= 1
                return text("click failed: target_occluded", err=True, sc={"code": "target_occluded"})
            role, lab = state["elements"][int(idx)]
            if lab == "Increment":
                state["count"] += 1
                state["elements"][2] = ("text", f"count={state['count']}")
            return text(f'Clicked element [{idx}] {role} "{lab}".', sc={"effect": "unverifiable", "route": "accessibility"})
        if "x" in a:
            return text(f"Clicked at {a['x']},{a['y']}", sc={"effect": "unverifiable", "route": "pixel"})
        return text("missing target", err=True)
    if name == "list_windows":
        ws = [w for w in state["windows"] if "pid" not in a or w["pid"] == a["pid"]]
        return text(f"Found {len(ws)} windows", sc={"windows": ws})
    if name == "verify_state":
        sel = a["expect"][0]["element"]["selector"]
        ok = any((not sel.get("role") or r == sel["role"]) and sel.get("label_contains", "") in l
                 for r, l in state["elements"])
        return text("verify", sc={"status": "satisfied" if ok else "unsatisfied"})
    if name == "fake_crash":
        os._exit(3)
    if name == "fake_mutate":
        if "elements" in a:
            state["elements"] = [tuple(x) for x in a["elements"]]
        if a.get("popup"):
            state["windows"].append({"window_id": WID + 1, "pid": PID, "x": 50, "y": 50, "width": 200,
                                     "height": 100, "is_on_screen": True})
        if "fail_next" in a:
            state["fail_next"] = a["fail_next"]
        return text("mutated")
    return text(f"{name} ok", sc={})


def main():
    for line in sys.stdin:
        m = json.loads(line)
        if "id" not in m:
            continue
        meth = m.get("method")
        if meth == "initialize":
            res = {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}},
                   "serverInfo": {"name": "fake-driver", "version": "0.0"}, "instructions": "fake"}
        elif meth == "tools/list":
            res = {"tools": TOOLS}
        elif meth == "tools/call":
            res = call(m["params"]["name"], m["params"].get("arguments") or {})
        else:
            res = {}
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": m["id"], "result": res}) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
