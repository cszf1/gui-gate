#!/usr/bin/env python3
"""CI / manual residue check: run one MCP session against the installed gate the way a client would, start a
background job (unix), close stdin, and assert that the gate, its driver and every job are gone afterwards.

    python3 scripts/smoke_session.py [PATH_TO_GUI_GATE]
"""
import json
import os
import subprocess
import sys
import time


def procs():
    """(pid, command line) of running processes (ps on unix, tasklist on Windows)."""
    if os.name == "nt":
        out = subprocess.run(["tasklist", "/FO", "CSV", "/NH"], capture_output=True, text=True).stdout
        return [(l.split(",")[1].strip('"'), l.split(",")[0].strip('"')) for l in out.splitlines() if l.count(",") >= 2]
    out = subprocess.run(["ps", "-A", "-o", "pid=,command="], capture_output=True, text=True).stdout
    return [(l.strip().split(None, 1)[0], l.strip().split(None, 1)[1] if len(l.strip().split(None, 1)) > 1 else "")
            for l in out.splitlines() if l.strip()]


def ours(mark):
    """gui-gate / cua-driver processes (by program name) and anything carrying the job mark."""
    me = str(os.getpid())
    names = {"gui-gate", "gui-gate.exe", "cua-driver", "cua-driver.exe"}
    out = []
    for p, c in procs():
        prog = os.path.basename((c.split() or [""])[0].replace("\\", "/"))
        if p != me and (prog in names or mark in c) and "smoke_session" not in c:
            out.append((p, c))
    return out


def main():
    gate = sys.argv[1] if len(sys.argv) > 1 else "gui-gate"
    mark = f"77{os.getpid()}"
    env = dict(os.environ, GUI_GATE_JOBS="1")
    p = subprocess.Popen([gate, "--human", "deny"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                         text=True, env=env)
    n = [0]

    def rpc(method, params):
        n[0] += 1
        p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": n[0], "method": method, "params": params}) + "\n")
        p.stdin.flush()
        while True:
            line = p.stdout.readline()
            if not line:
                raise SystemExit(f"gate closed its output during {method}")
            m = json.loads(line)
            if m.get("id") == n[0]:
                return m

    rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "smoke", "version": "0"}})
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
    p.stdin.flush()
    tools = [t["name"] for t in rpc("tools/list", {})["result"]["tools"]]
    print(f"{len(tools)} tools")
    if os.name != "nt":
        r = rpc("tools/call", {"name": "run_command", "arguments": {"command": f"(sleep {mark}) & sleep {mark}", "mode": "start"}})
        print(r["result"]["content"][0]["text"][:100])
        time.sleep(1)
        if not [x for x in ours(mark) if f"sleep {mark}" in x[1]]:
            raise SystemExit("the background job did not start")
    p.stdin.close()
    p.wait(timeout=20)
    time.sleep(3)
    left = ours(mark)
    if left:
        raise SystemExit(f"left running after the session: {left}")
    print("no gui-gate / cua-driver / job process left")


if __name__ == "__main__":
    main()
