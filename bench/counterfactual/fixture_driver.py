"""A stand-in for `cua-driver mcp` that serves the serialisable fixture (fixture_env.FixtureEnv) over MCP stdio,
so counterfactual branches run through the REAL gui-gate binary offline (no desktop, no model).

Modelled on tests/fake_driver.py: snapshot ids, element tokens that go stale after the next read, list_windows,
click results. Extra tools for the harness (the gate passes them through; none of them is a GUI action):
  fixture_dump     -> the full env state (JSON) and its hash
  fixture_advance  -> advance the virtual clock by `s` seconds (model think time, waits)
  fixture_probe    -> {index, caption}: does that element still show that caption (the gate's cheap probe)
FIXTURE_STATE=path.json starts from a saved state (a checkpoint); otherwise FIXTURE_SEED / FIXTURE_LENGTH.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from fixture_env import FixtureEnv  # noqa: E402

PID, WID, DLG = 5151, 901, 902
if os.environ.get("FIXTURE_STATE"):
    env = FixtureEnv.load(json.load(open(os.environ["FIXTURE_STATE"])))
else:
    env = FixtureEnv(seed=int(os.environ.get("FIXTURE_SEED") or 0), length=int(os.environ.get("FIXTURE_LENGTH") or 10))
snap = {"n": 0, "rows": {}}
TOOLS = [{"name": n, "inputSchema": {"type": "object", "properties": {"pid": {"type": "integer"}}}}
         for n in ["get_window_state", "click", "list_windows", "fixture_dump", "fixture_advance", "fixture_probe", "bring_to_front"]]


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
        snap["n"] += 1
        sid = f"s{snap['n']:08x}"
        full = a.get("include_screenshot", True)
        o = env.observe(popup_visible=full)
        rows = [("button", lab) for lab in o["labels"]]
        if o["popup"]:
            rows.append(("button", "OK"))  # the Notice dialog's button, index 8
        snap["rows"][sid] = rows
        md = f'- frame = "GateFixture"\n  - label "Sequence: {", ".join(o["seq"])}"\n  - label "Progress {o["k"]}/{len(o["seq"])}"\n'
        md += "\n".join(f'    - [{i}] {r} "{lab}" [actions=[click]]' for i, (r, lab) in enumerate(rows)) + "\n"
        if o["popup"]:
            md += '- dialog = "Notice"\n'
        sc = {"snapshot_id": sid, "pid": PID, "window_id": WID, "tree_markdown": md, "truncated": False,
              "screenshot_width": 560 if full else None, "screenshot_height": 360 if full else None}
        return text(f"window_id={WID} pid={PID} snapshot_id={sid}\n{md}", sc=sc, img=full)
    if name == "click":
        tok = a.get("element_token") or ""
        if ":" not in tok:
            return text("missing target", err=True)
        sid, idx = tok.split(":")
        if sid != f"s{snap['n']:08x}":
            return text("element_token is stale or unknown; call get_window_state again to refresh.", err=True, sc={"refusal": "stale"})
        idx = int(idx)
        rows = snap["rows"].get(sid, [])
        target = "ok" if idx >= 8 else idx
        ev = env.click(target)
        lab = rows[idx][1] if idx < len(rows) else "?"
        return text(f'Clicked element [{idx}] button "{lab}".', sc={"effect": "unverifiable", "route": "accessibility", "fixture": ev})
    if name == "list_windows":
        ws = [{"window_id": WID, "pid": PID, "x": 0, "y": 0, "width": 560, "height": 360, "is_on_screen": True, "title": "GateFixture"}]
        if env.st["popup"]:
            ws.append({"window_id": DLG, "pid": PID, "x": 100, "y": 100, "width": 200, "height": 100, "is_on_screen": True, "title": "Notice"})
        return text(f"Found {len(ws)} windows", sc={"windows": ws})
    if name == "fixture_dump":
        return text(json.dumps({"state": env.dump(), "hash": env.state_hash()}))
    if name == "fixture_advance":
        env.advance(float(a.get("s") or 0))
        return text(f"t={env.st['t']}")
    if name == "fixture_probe":
        return text(json.dumps({"pass": env.probe(int(a["index"]), a["caption"])}))
    return text(f"{name} ok", sc={})


def main():
    for line in sys.stdin:
        m = json.loads(line)
        if "id" not in m:
            continue
        meth = m.get("method")
        if meth == "initialize":
            res = {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}}, "serverInfo": {"name": "fixture-driver", "version": "0"}}
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
