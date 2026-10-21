# drive gui-gate (MCP stdio) to get list_windows + get_window_state
import json, subprocess, sys, os
p = subprocess.Popen([os.path.expanduser("~/.local/bin/gui-gate"), "--policy", "direct", "--human", "deny"],
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
i = 0
def rpc(m, params=None, notify=False):
    global i
    msg = {"jsonrpc": "2.0", "method": m}
    if params is not None: msg["params"] = params
    if not notify: i += 1; msg["id"] = i
    p.stdin.write(json.dumps(msg) + "\n"); p.stdin.flush()
    if notify: return None
    while True:
        r = json.loads(p.stdout.readline())
        if r.get("id") == i: return r
rpc("initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "x", "version": "1"}})
rpc("notifications/initialized", notify=True)
def call(name, args):
    r = rpc("tools/call", {"name": name, "arguments": args})
    return r.get("result", r)
if __name__ == "__main__":
    tl = rpc("tools/list")["result"]["tools"]
    for t in tl:
        if t["name"] in ("list_windows", "get_window_state"): print(json.dumps(t)[:1500])
    lw = call("list_windows", {})
    print(json.dumps(lw)[:1500])
