"""A tiny stand-in for `cua-driver mcp`, used only by the unit tests.

It mimics the parts of Cua Driver's contract the gate relies on: snapshot ids,
element tokens that go stale after the next read, verify_state, list_windows,
click results. `fake_mutate` lets a test change the UI between calls.
"""
import base64
import json
import os
import struct
import sys
import zlib

PID, WID = int(os.environ.get("FAKE_PID") or 4242), 777
state = {
    "snap": 0,
    "elements": [("button", "Increment"), ("button", "Delete file"), ("text", "count=0"), ("button", "OK"), ("button", "OK")],
    "windows": [{"window_id": WID, "pid": PID, "x": 0, "y": 0, "width": 500, "height": 300, "is_on_screen": True,
                 "title": "Fixture"}],
    "win_elems": {},   # window_id -> elements of a popup window (menus, dialogs)
    "snap_win": {},    # snapshot id -> window_id it was read from
    "clicked": [],
    "fail_next": 0,
    "count": 0,
    "keys": [],
}
TOOLS = [{"name": n, "inputSchema": {"type": "object", "properties": {"pid": {"type": "integer"},
         **({"delivery_mode": {"type": "string"}} if n in ("click", "type_text", "press_key", "hotkey") else {})}}}
         for n in ["fake_state", "get_window_state", "click", "type_text", "press_key", "hotkey", "list_windows", "verify_state",
                   "run_steps", "fake_mutate", "launch_app", "fake_crash", "bring_to_front"]]


def snap_id(n):
    return f"s{n:08x}"


def real_png(w=500, h=300):
    """A real RGBA PNG (stdlib only): flat panels, text-like stripes and a gradient, like a window capture."""
    rows = []
    for y in range(h):
        row = bytearray(b"\x00")
        for x in range(w):
            v = 40 if (y // 6) % 4 == 0 and (x // 3) % 5 and x % 300 < 180 else (230 if y < 40 else 250)
            row += bytes((v, v * (w - x // 2) // w, (x + y) % 256, 255))
        rows.append(bytes(row))
    chunk = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(b"".join(rows), 9)) + chunk(b"IEND", b""))


# FAKE_PNG=1: screenshots are a real 500x300 PNG instead of the placeholder "AAAA" (not decodable)
_PNG = base64.b64encode(real_png()).decode() if os.environ.get("FAKE_PNG") else None


def text(t, err=False, sc=None, img=False):
    r = {"content": [{"type": "text", "text": t}]}
    if img:
        r["content"].append({"type": "image", "data": _PNG or "AAAA", "mimeType": "image/png"})
    if err:
        r["isError"] = True
    if sc is not None:
        r["structuredContent"] = sc
    return r


def call(name, a):
    if name == "click":
        state.setdefault("modes", []).append(a.get("delivery_mode"))
    if state.get("fg_fail_next") and name in ("click", "press_key", "type_text"):
        state["fg_fail_next"] -= 1
        return text(f"foreground_unavailable: window 0x1 did not become the active, focused window within 800ms "
                    f"(active=0x2, focus_within_target=true); no input was sent.", err=True)
    if name == "bring_to_front":
        state.setdefault("front", []).append(a.get("pid"))
    # FAKE_NO_BG=1: like a desktop without a focus-free input backend (no /dev/uinput)
    if (os.environ.get("FAKE_NO_BG") and name in ("type_text", "press_key", "hotkey")
            and a.get("delivery_mode") != "foreground"):
        return text("Background delivery is not available: the requested target has no focus-free input backend",
                    err=True)
    if name == "get_window_state":
        state["snap"] += 1
        sid = snap_id(state["snap"])
        w = a.get("window_id") or WID
        if w not in state["win_elems"]:
            w = WID
        els = state["win_elems"][w] if w != WID else state["elements"]
        state["snap_win"][sid] = w
        rows = "\n".join(f'    - [{i}] {r} "{l}" [actions=[click]]' for i, (r, l) in enumerate(els))
        md = f'- frame = "Fixture"\n{rows}\n'
        shot = a.get("include_screenshot", True)
        state["last_shot"] = shot
        sc = {"snapshot_id": sid, "pid": PID, "window_id": w, "tree_markdown": md, "truncated": False,
              "screenshot_width": 500 if shot else None, "screenshot_height": 300 if shot else None}
        if shot:
            sc["capture_id"] = "cap_" + sid
        if a.get("tree_format") == "elements":
            sc["elements"] = [{"element_index": i, "element_token": f"{sid}:{i}", "role": r, "label": l,
                               "enabled": True, "frame": {"x": 10, "y": 10 + 40 * i, "w": 200, "h": 34}}
                              for i, (r, l) in enumerate(els)]
            # like cua-driver: the rows are only in structuredContent.elements
            del sc["tree_markdown"]
            return text(f"window_id={w} pid={PID} snapshot_id={sid}\n\n(tree is in structuredContent.elements; pass "
                        'tree_format "markdown" for the compact text rendering)', sc=sc, img=shot)
        return text(f"window_id={w} pid={PID} snapshot_id={sid}\n{md}", sc=sc, img=shot)
    if name == "click":
        tok = a.get("element_token")
        if tok:
            sid, idx = tok.split(":")
            if sid != snap_id(state["snap"]):
                return text("element_token is stale or unknown; call get_window_state again to refresh.", err=True,
                            sc={"refusal": "stale"})
            if a.get("delivery_mode") == "background" and state.pop("bg_fail_once", False):
                return text("click: element has no accessibility action to perform focus-free", err=True)
            if state.get("occlude_next") and a.get("delivery_mode") != "background":
                # a pointer click is checked against the point owner; the focus-free path (AT-SPI action) is not
                occ = state.pop("occlude_next")
                orole, olab = occ[0], occ[1]
                if len(occ) > 2 and occ[2] == "noaction":
                    state["bg_fail_once"] = True
                return text(f'click: element [{idx}] (pid {PID}) has a snapshot frame at (161, 71), but that screen point is '
                            f'owned by {orole} "{olab}" rather than the observed element; a real pointer click there would '
                            'hit the wrong control. Re-snapshot with get_window_state, or click by pixel from a screenshot.',
                            err=True)
            if state["fail_next"]:
                state["fail_next"] -= 1
                return text("click failed: target_occluded", err=True, sc={"code": "target_occluded"})
            w = state["snap_win"].get(sid, WID)
            role, lab = (state["win_elems"][w] if w != WID else state["elements"])[int(idx)]
            state["clicked"].append(lab)
            if lab == "Open menu":
                state["windows"].append({"window_id": WID + 6, "pid": PID, "x": 0, "y": 20, "width": 120,
                                         "height": 200, "is_on_screen": True, "title": "", "z_index": 9})
                state["win_elems"][WID + 6] = [("menu item", "Save As"), ("menu item", "Export")]
            if lab == "Open dialog":
                state["windows"].append({"window_id": WID + 5, "pid": PID, "x": 60, "y": 60, "width": 200,
                                         "height": 100, "is_on_screen": True, "title": "Dialog"})
            if lab == "Increment":
                state["count"] += 1
                state["elements"][2] = ("text", f"count={state['count']}")
            return text(f'Clicked element [{idx}] {role} "{lab}".', sc={"effect": "unverifiable", "route": "accessibility"})
        if "x" in a:
            if os.environ.get("FAKE_NO_FRAME") and not state.get("last_shot", True):
                return text(f"No screenshot frame for window {WID}: its latest read had no screenshot", err=True)
            state["clicked"].append(f"xy:{a['x']},{a['y']}:{a.get('coordinate_frame', 'window')}")
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
    if name == "press_key":
        state["keys"].append("+".join(list(a.get("modifiers") or []) + [a.get("key", "")]))
    if name == "fake_state":
        return text(json.dumps({"keys": state["keys"], "clicked": state["clicked"], "front": state.get("front", []),
                                "modes": state.get("modes", [])}))
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
        if "fg_fail_next" in a:
            state["fg_fail_next"] = a["fg_fail_next"]
        if "occlude_next" in a:
            state["occlude_next"] = a["occlude_next"]
        return text("mutated")
    return text(f"{name} ok delivery={a.get('delivery_mode', 'background')}", sc={})


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
