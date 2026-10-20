#!/usr/bin/env python3
"""A SCRIPTED stand-in for an LLM agent (no model inside), used to exercise the
gate end-to-end on a real desktop. It behaves like a thrifty computer-use
agent: it reads the window (with a screenshot by default), then performs a few
actions from that read before reading again, and it obeys "[gate]" messages.

Every tool call it makes counts as one model turn; every screenshot it receives
counts as image input. Model latency is not slept, only accounted as virtual
time (--turn-s, --image-s), so runs are fast and comparable.

    python3 sim_agent.py --server "gui-gate --policy gate" --seed 3
"""
import argparse
import json
import os
import re
import shlex
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROW = re.compile(r'\[(\d+)\]\s+([a-z ]+?)\s+"((?:[^"\\]|\\.)*)"')


class MCP:
    def __init__(self, cmd):
        self.p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self.i = 0
        self.rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                "clientInfo": {"name": "sim-agent", "version": "0"}})
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        self.p.stdin.flush()

    def rpc(self, m, params):
        self.i += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.i, "method": m, "params": params}) + "\n")
        self.p.stdin.flush()
        while True:
            d = json.loads(self.p.stdout.readline())
            if d.get("id") == self.i:
                return d.get("result") or {"isError": True, "content": [{"type": "text", "text": str(d.get("error"))}]}

    def call(self, name, **a):
        return self.rpc("tools/call", {"name": name, "arguments": a})

    def close(self):
        self.p.stdin.close()
        try:
            self.p.wait(timeout=5)
        except Exception:
            self.p.kill()


def txt(r):
    return "\n".join(c.get("text", "") for c in r.get("content", []) if c.get("type") == "text")


class Agent:
    def __init__(self, mcp, pid, wid, a):
        self.m, self.pid, self.wid, self.a = mcp, pid, wid, a
        self.turns = 0
        self.screens = 0
        self.snap = None
        self.rows = {}
        self.seq = []
        self.progress = 0
        self.since_read = 0
        self.notes = []

    def tool(self, name, **args):
        self.turns += 1
        r = self.m.call(name, **args)
        if any(c.get("type") == "image" for c in r.get("content", [])):
            self.screens += 1
        return r

    def read(self, shot=True):
        r = self.tool("get_window_state", pid=self.pid, window_id=self.wid, include_screenshot=shot)
        sc = r.get("structuredContent") or {}
        self.snap = sc.get("snapshot_id")
        md = sc.get("tree_markdown") or txt(r)
        self.rows = {}
        for line in md.splitlines():
            m = ROW.search(line)
            if m:
                self.rows.setdefault(m.group(3), []).append(int(m.group(1)))
        m = re.search(r"Sequence: ([A-Za-z, ]+)", md)
        if m:
            self.seq = [s.strip() for s in m.group(1).split(",")]
        m = re.search(r"Progress (\d+)/(\d+)", md)
        if m:
            self.progress = int(m.group(1))
        self.since_read = 0
        self.handle_dialogs()

    def handle_dialogs(self):
        r = self.tool("list_windows", pid=self.pid)
        for w in (r.get("structuredContent") or {}).get("windows", []):
            if w.get("window_id") != self.wid and w.get("is_on_screen", True):
                d = self.tool("get_window_state", pid=self.pid, window_id=w["window_id"], include_screenshot=self.a.shot)
                sc = d.get("structuredContent") or {}
                md = sc.get("tree_markdown") or ""
                for line in md.splitlines():
                    mm = ROW.search(line)
                    if mm and mm.group(3) in ("OK", "_OK"):
                        self.tool("click", element_token=f"{sc['snapshot_id']}:{mm.group(1)}")
                        self.notes.append("closed dialog")
                        break
                # back to the main window with a fresh read
                return self.read(self.a.shot)

    def run(self):
        self.read(True)
        budget = self.a.budget_mult * max(1, len(self.seq))
        ack = False
        while self.turns < budget:
            if self.progress >= len(self.seq):
                break
            if self.since_read >= max(1, self.a.reread_every) or (self.a.reread_every == 0 and self.since_read > 0):
                self.read(self.a.shot)
                continue
            target = self.seq[self.progress]
            idx = self.rows.get(target)
            if not idx:
                self.read(self.a.shot)
                continue
            args = {"element_token": f"{self.snap}:{idx[0]}"}
            if ack:
                args["gate_ack"] = True
                ack = False
            r = self.tool("click", **args)
            t = txt(r)
            if t.startswith("[gate]"):
                if "without a screenshot" in t:
                    self.read(False)
                elif "WITH a screenshot" in t:
                    self.read(True)
                elif "gate_ack" in t:
                    self.read(True)
                    ack = True  # looked again, still sure
                elif "list_windows" in t or "dialog" in t:
                    self.read(self.a.shot)
                else:
                    self.read(True)
                continue
            if r.get("isError"):
                self.read(True)
                continue
            # (if the gate re-bound a token, the agent's own label->index map is older than the
            #  gate's snapshot; it keeps using its own tokens and the gate re-binds them again)
            self.progress += 1  # the agent assumes success until its next read says otherwise
            self.since_read += 1
            if self.progress >= len(self.seq):
                self.read(self.a.shot)  # verify completion
        return self


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--server", default="gui-gate --policy gate")
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--length", type=int, default=10)
    ap.add_argument("--reread-every", type=int, default=3)
    ap.add_argument("--shot", type=int, default=1, help="agent habit: screenshots on its own reads")
    ap.add_argument("--budget-mult", type=int, default=4)
    ap.add_argument("--turn-s", type=float, default=1.5, help="virtual model latency per tool call")
    ap.add_argument("--image-s", type=float, default=1.0, help="extra virtual latency per screenshot")
    ap.add_argument("--fixture-log", default="/tmp/fixture.jsonl")
    ap.add_argument("--hazards", default="", help="extra fixture args, e.g. '--p-relabel 0.4'")
    ap.add_argument("--out", default="")
    ap.add_argument("--tag", default="")
    a = ap.parse_args()
    a.shot = bool(a.shot)

    if os.path.exists(a.fixture_log):
        os.remove(a.fixture_log)
    app = subprocess.Popen([sys.executable, os.path.join(HERE, "fixture_app.py"), "--seed", str(a.seed),
                            "--length", str(a.length), "--log", a.fixture_log, *shlex.split(a.hazards)])
    mcp = MCP(shlex.split(a.server))
    t0 = time.time()
    try:
        pid = wid = None
        for _ in range(50):
            r = mcp.call("list_windows", pid=app.pid)
            ws = [w for w in (r.get("structuredContent") or {}).get("windows", []) if w.get("title") == "GateFixture"]
            if ws:
                pid, wid = ws[0]["pid"], ws[0]["window_id"]
                break
            time.sleep(0.2)
        if wid is None:
            raise SystemExit("fixture window not found")
        time.sleep(0.5)
        t1 = time.time()
        ag = Agent(mcp, pid, wid, a).run()
        wall = time.time() - t1
        rep = mcp.call("gate_report")
        gate = rep.get("structuredContent") if not rep.get("isError") else None
    finally:
        mcp.close()
        app.terminate()
    evs = [json.loads(l) for l in open(a.fixture_log)] if os.path.exists(a.fixture_log) else []
    clicks = [e for e in evs if e["ev"] == "click"]
    res = {
        "seed": a.seed, "server": a.server, "tag": a.tag, "reread_every": a.reread_every, "shot": a.shot,
        "length": a.length,
        "done": any(e["ev"] == "done" for e in evs),
        "correct_clicks": sum(1 for e in clicks if e["correct"]),
        "wrong_clicks": sum(1 for e in clicks if not e["correct"]),
        "ignored_clicks": sum(1 for e in evs if e["ev"] == "ignored"),
        "hazards": {k: sum(1 for e in evs if e["ev"] == k) for k in ("relabel", "loading", "popup_open")},
        "turns": ag.turns, "screenshots": ag.screens,
        "wall_s": round(wall, 2),
        "virtual_s": round(wall + ag.turns * a.turn_s + ag.screens * a.image_s, 2),
        "gate": {k: gate.get(k) for k in ("by_option", "checks", "heals", "gate_ms_per_action", "model_turns_requested")} if gate else None,
    }
    line = json.dumps(res)
    print(line)
    if a.out:
        with open(a.out, "a") as fh:
            fh.write(line + "\n")


if __name__ == "__main__":
    main()
