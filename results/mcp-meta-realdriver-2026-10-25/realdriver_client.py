#!/usr/bin/env python3
"""Real-driver protocol test: gui-gate + real cua-driver over stdio, MODERN (MCP 2026-07-28 per-request _meta,
no initialize) and LEGACY (initialize 2025-06-18) clients. Only read-only tools are called, so the reviewer
(a dummy local URL) is never hit.

usage: realdriver_client.py GATE_BIN DRIVER_BIN OUT.json {modern|legacy} [tool]
"""
import json, os, subprocess, sys, tempfile, threading, time

MODERN = "2026-07-28"
META = {"io.modelcontextprotocol/protocolVersion": MODERN,
        "io.modelcontextprotocol/clientCapabilities": {},
        "io.modelcontextprotocol/clientInfo": {"name": "realdriver-client", "version": "1"}}


def strip(o):
    if isinstance(o, dict):
        if o.get("type") == "image" and "data" in o:
            return dict(o, data=f"<base64 {len(o['data'])} chars stripped>")
        return {k: strip(v) for k, v in o.items()}
    if isinstance(o, list):
        return [strip(v) for v in o]
    if isinstance(o, str) and len(o) > 4000:
        return o[:4000] + f"...<{len(o) - 4000} more chars>"
    return o


def main():
    gate, driver, out, mode = sys.argv[1:5]
    tool = sys.argv[5] if len(sys.argv) > 5 else "get_screen_size"
    log = tempfile.NamedTemporaryFile(suffix=".jsonl", delete=False).name
    env = dict(os.environ, GUI_GATE_DRIVER=driver, GUI_GATE_NO_UPDATE_CHECK="1",
               GUI_GATE_REVIEWER_URL="http://127.0.0.1:9/v1/chat/completions",
               GUI_GATE_REVIEWER_MODEL="dummy-reviewer", GUI_GATE_REVIEWER_KEY="dummy-key",
               GUI_GATE_REVIEWER_TIMEOUT="2", GUI_GATE_REVIEWER_ENV="/nonexistent/reviewer.env")
    p = subprocess.Popen([gate, "--log", log], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.PIPE, text=True, env=env)
    err = []
    threading.Thread(target=lambda: err.extend(p.stderr), daemon=True).start()
    transcript, i = [], 0

    def send(method, params=None, notify=False, meta=False):
        nonlocal i
        msg = {"jsonrpc": "2.0", "method": method, "params": dict(params or {})}
        if meta:
            msg["params"]["_meta"] = META
        if not notify:
            i += 1
            msg["id"] = i
        p.stdin.write(json.dumps(msg) + "\n"); p.stdin.flush()
        rec = {"request": msg}
        if not notify:
            t0 = time.time()
            while True:
                line = p.stdout.readline()
                if not line:
                    rec["response"] = None; rec["note"] = "gate closed stdout"; break
                r = json.loads(line)
                if r.get("id") == i and "method" not in r:
                    rec["response"] = r; break
                rec.setdefault("other_messages", []).append(r)
            rec["seconds"] = round(time.time() - t0, 3)
        transcript.append(strip(rec))
        return rec.get("response")

    modern = mode == "modern"
    if not modern:
        send("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": {"name": "realdriver-client", "version": "1"}})
        send("notifications/initialized", notify=True)
    else:
        send("server/discover", {}, meta=True)
    tl = send("tools/list", {}, meta=modern)
    names = [t["name"] for t in (tl or {}).get("result", {}).get("tools", [])]
    if tool not in names:
        tool = next((t for t in ("get_screen_size", "list_apps", "list_windows", "get_cursor_position") if t in names), tool)
    call = send("tools/call", {"name": tool, "arguments": {}}, meta=modern)
    p.stdin.close()
    try:
        p.wait(timeout=10)
    except subprocess.TimeoutExpired:
        p.kill()
    ok = bool(call and "result" in call and not call["result"].get("isError"))
    summary = {"gate": gate, "gate_version": subprocess.run([gate, "--version"], capture_output=True, text=True).stdout.strip(),
               "driver": driver, "driver_version": subprocess.run([driver, "--version"], capture_output=True, text=True).stdout.strip(),
               "mode": mode, "tool": tool, "tools_listed": len(names), "tools_call_ok": ok,
               "tools_call_error": (call or {}).get("error") or ((call or {}).get("result") if not ok else None),
               "stderr_tail": "".join(err)[-2000:]}
    json.dump(strip({"summary": summary, "transcript": transcript}), open(out, "w"), indent=2, ensure_ascii=False)
    print(json.dumps(strip(summary), ensure_ascii=False)[:1500])


main()
