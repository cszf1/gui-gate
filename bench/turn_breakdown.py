#!/usr/bin/env python3
"""Where a Pi turn's time goes, per run: model (time to first token / full response), reasoning and output
length, tokens sent, images in context, tool execution, gate overhead and reviewer calls.

  python bench/turn_breakdown.py OUTDIR [TAG ...]    -> OUTDIR/turn_breakdown.md (+ .json)
Needs pi_monitor's .times sidecar (arrival time of turn/message/tool events).
"""
import base64
import json
import os
import struct
import sys


def jl(p):
    try:
        with open(p, encoding="utf-8", errors="replace") as f:
            for line in f:
                try:
                    yield json.loads(line)
                except json.JSONDecodeError:
                    pass
    except OSError:
        return


def img_dims(b64):
    try:
        raw = base64.b64decode(b64[:200] + "=" * (-len(b64[:200]) % 4))
        if raw[:8] == b"\x89PNG\r\n\x1a\n":
            return struct.unpack(">II", raw[16:24])
        if raw[:2] == b"\xff\xd8":
            raw = base64.b64decode(b64)
            i = 2
            while i < len(raw):
                if raw[i] != 0xFF:
                    i += 1
                    continue
                m = raw[i + 1]
                if m in (0xC0, 0xC1, 0xC2):
                    h, w = struct.unpack(">HH", raw[i + 5:i + 9])
                    return w, h
                i += 2 + struct.unpack(">H", raw[i + 2:i + 4])[0]
    except Exception:
        pass
    return None


def run(out, tag):
    times = list(jl(os.path.join(out, "pi", tag + ".jsonl.times")))
    turns, cur = [], None
    tool_t0 = None
    for e in times:
        t = e["type"]
        if t == "turn_start":
            cur = {"t0": e["t"], "tools": []}
            turns.append(cur)
        elif t == "message_start" and e.get("role") == "assistant" and cur:
            cur["ttft"] = e["t"] - cur["t0"]
        elif t == "message_end" and e.get("role") == "assistant" and cur:
            cur["model"] = e["t"] - cur["t0"]
        elif t == "tool_execution_start":
            tool_t0 = e["t"]
        elif t == "tool_execution_end" and cur and tool_t0 is not None:
            cur["tools"].append((e.get("tool", "").replace("mcp__gui__", ""), e["t"] - tool_t0))
    # Pi stream: usage + reasoning per assistant message, images per tool result
    msgs, imgs_ctx, new_imgs = [], 0, []
    pending = []
    for e in jl(os.path.join(out, "pi", tag + ".jsonl")):
        if e.get("type") != "message_end":
            continue
        m = e.get("message") or {}
        if m.get("role") == "assistant":
            u = m.get("usage") or {}
            th = sum(len(c.get("thinking", "")) for c in m.get("content") or [] if c.get("type") == "thinking")
            msgs.append({"in": u.get("input", 0), "cached": u.get("cacheRead", 0), "out": u.get("output", 0),
                         "reasoning": u.get("reasoning"), "think_chars": th, "imgs_in_ctx": imgs_ctx,
                         "new_imgs": pending})
            pending = []
        elif m.get("role") == "toolResult":
            for c in m.get("content") or []:
                if c.get("type") == "image":
                    imgs_ctx += 1
                    d = c.get("data") or ""
                    pending.append({"kb": round(len(d) * 3 / 4 / 1024), "mime": c.get("mimeType"), "dims": img_dims(d)})
    # gate log: per-call execution + gate decision time + reviewer
    gate = {"actions": 0, "exec_ms": 0.0, "gate_ms": 0.0, "obs": 0, "obs_ms": 0.0, "reviews": [], "shots": 0, "px": []}
    for e in jl(os.path.join(out, "gate", tag + ".jsonl")):
        if e.get("ev") == "action":
            gate["actions"] += 1
            gate["exec_ms"] += e.get("exec_ms") or 0
            gate["gate_ms"] += e.get("gate_ms") or 0
            for j in [e.get("judge"), (e.get("risk_gate") or {}).get("reviewer") if isinstance(e.get("risk_gate"), dict) else None]:
                if isinstance(j, dict):
                    gate["reviews"].append(j.get("ms"))
        elif e.get("ev") == "obs":
            gate["obs"] += 1
            gate["obs_ms"] += e.get("ms") or 0
            if e.get("screenshot"):
                gate["shots"] += 1
                gate["px"].append(e.get("px"))
    for i, t in enumerate(turns):
        if i < len(msgs):
            t.update(msgs[i])
    return turns, gate


def main():
    out = sys.argv[1]
    tags = sys.argv[2:] or sorted(f[:-len(".jsonl.times")] for f in os.listdir(os.path.join(out, "pi")) if f.endswith(".jsonl.times"))
    md = ["# Per-turn time breakdown (bench/turn_breakdown.py)", ""]
    dump = {}
    for tag in tags:
        turns, g = run(out, tag)
        dump[tag] = {"turns": turns, "gate": g}
        md += [f"## {tag}", "", "| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | "
               "think chars | images in context | new images |", "|" + "---|" * 10]
        for i, t in enumerate(turns, 1):
            tools = ", ".join(f"{n} {s:.2f}" for n, s in t["tools"])
            ni = "; ".join(f"{x['kb']} KB {x['mime']} {x['dims'][0]}x{x['dims'][1]}" if x.get("dims") else f"{x['kb']} KB"
                           for x in t.get("new_imgs", []))
            md.append(f"| {i} | {t.get('ttft', 0):.1f} | {t.get('model', 0):.1f} | {tools} | {t.get('in', 0)}+{t.get('cached', 0)} | "
                      f"{t.get('out', '')} | {t.get('reasoning', '')} | {t.get('think_chars', '')} | {t.get('imgs_in_ctx', '')} | {ni} |")
        mt = sum(t.get("model", 0) for t in turns)
        tt = sum(s for t in turns for _, s in t["tools"])
        rv = [x for x in g["reviews"] if x is not None]
        md += ["", f"model total {mt:.1f} s, tool total {tt:.1f} s over {len(turns)} turns; gate: {g['actions']} actions "
               f"exec {g['exec_ms'] / 1000:.1f} s, gate decision {g['gate_ms'] / 1000:.2f} s, {g['obs']} observations "
               f"{g['obs_ms'] / 1000:.1f} s ({g['shots']} with screenshot), reviewer calls {len(g['reviews'])} "
               f"({sum(rv) / 1000:.1f} s)", ""]
    open(os.path.join(out, "turn_breakdown.md"), "w").write("\n".join(md) + "\n")
    json.dump(dump, open(os.path.join(out, "turn_breakdown.json"), "w"), indent=1, default=str)
    print("\n".join(md))


if __name__ == "__main__":
    main()
