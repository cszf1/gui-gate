#!/usr/bin/env python3
"""Run `pi --mode json ...`, tee its event stream to a file, and stop it after
--max-turns assistant turns or --timeout seconds. Prints a JSON status line."""
import argparse, json, os, signal, subprocess, sys, time

ap = argparse.ArgumentParser()
ap.add_argument("--out", required=True)
ap.add_argument("--max-turns", type=int, default=30)
ap.add_argument("--timeout", type=float, default=480)
ap.add_argument("cmd", nargs=argparse.REMAINDER)
a = ap.parse_args()
cmd = a.cmd[1:] if a.cmd and a.cmd[0] == "--" else a.cmd
t0 = time.time()
p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stdin=subprocess.DEVNULL, stderr=open(a.out + ".err", "w"),
                     start_new_session=True, text=True, bufsize=1)
turns, stop, final = 0, None, ""
import threading
def killer():
    while p.poll() is None:
        if time.time() - t0 > a.timeout:
            globals()["stop"] = "timeout"
            os.killpg(p.pid, signal.SIGTERM); time.sleep(3)
            if p.poll() is None: os.killpg(p.pid, signal.SIGKILL)
            return
        time.sleep(1)
threading.Thread(target=killer, daemon=True).start()
# sidecar with arrival times (s since start) of the non-streaming events: model latency per turn =
# assistant message_end - message_start, tool time = tool_execution_end - tool_execution_start
tf = open(a.out + ".times", "w")
with open(a.out, "w") as f:
    for line in p.stdout:
        f.write(line)
        try:
            e = json.loads(line)
        except Exception:
            continue
        et = e.get("type")
        if et in ("message_start", "message_end", "tool_execution_start", "tool_execution_end", "turn_start", "agent_end"):
            tf.write(json.dumps({"t": round(time.time() - t0, 3), "type": et, "role": (e.get("message") or {}).get("role"),
                                 "tool": e.get("toolName")}) + "\n")
            tf.flush()
        if e.get("type") == "message_end" and (e.get("message") or {}).get("role") == "assistant":
            turns += 1
            txt = "".join(c.get("text", "") for c in e["message"].get("content") or [] if c.get("type") == "text")
            if txt.strip():
                final = txt
            if turns >= a.max_turns and stop is None:
                stop = "max_turns"
                os.killpg(p.pid, signal.SIGTERM)
p.wait()
open(a.out + ".final.txt", "w").write(final)
print(json.dumps({"turns": turns, "stop": stop or "agent", "rc": p.returncode, "wall_s": round(time.time() - t0, 1)}))
