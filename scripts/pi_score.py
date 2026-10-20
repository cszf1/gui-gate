#!/usr/bin/env python3
"""Score pi_eval.sh runs: task outcome from the fixture's ground-truth log,
cost from Pi's JSON event stream and the gate log. Prints a table + JSON."""
import json
import os
import sys
from collections import defaultdict

out = sys.argv[1]


def jl(p):
    try:
        for line in open(p, encoding="utf-8"):
            try:
                yield json.loads(line)
            except json.JSONDecodeError:
                pass
    except OSError:
        return


rows = []
for r in jl(os.path.join(out, "runs.jsonl")):
    tag = r["tag"]
    fx = list(jl(os.path.join(out, f"fixture-{tag}.jsonl")))
    seq = next((e.get("sequence") for e in fx if e.get("ev") == "start"), []) or []
    clicks = [e for e in fx if e.get("ev") == "click"]
    r["done"] = any(e.get("ev") == "done" for e in fx)
    r["progress"] = max([e.get("k", -1) + 1 for e in clicks if e.get("correct")] or [0])
    r["need"] = len(seq)
    r["wrong"] = sum(1 for e in clicks if not e.get("correct"))
    r["ignored"] = sum(1 for e in fx if e.get("ev") == "ignored")
    turns = tin = tout = calls = 0
    err = None
    for e in jl(os.path.join(out, f"pi-{tag}.jsonl")):
        if e.get("type") == "message_end" and (e.get("message") or {}).get("role") == "assistant":
            m = e["message"]
            turns += 1
            u = m.get("usage") or {}
            tin += (u.get("input") or 0) + (u.get("cacheRead") or 0)
            tout += u.get("output") or 0
            calls += sum(1 for c in m.get("content") or [] if c.get("type") == "toolCall")
            if m.get("stopReason") == "error":
                err = (m.get("errorMessage") or "")[:80]
    r.update(turns=turns, tool_calls=calls, tok_in=tin, tok_out=tout, error=err)
    g = defaultdict(float)
    for e in jl(os.path.join(out, f"gate-{tag}.jsonl")):
        if e.get("ev") == "obs":
            g["obs"] += 1
            g["shots"] += 1 if e.get("screenshot") else 0
            g["skipped"] += 1 if e.get("trimmed") else 0
        elif e.get("ev") == "action":
            g["actions"] += 1
            g["gate_ms"] += e.get("gate_ms") or 0
            if not e.get("executed"):
                g["interventions"] += 1
            if e.get("decision") == "model_review":
                g["review"] += 1
            if e.get("tier") == "model_ack":
                g["acked"] += 1
    r.update({k: round(v, 1) for k, v in g.items()})
    rows.append(r)

conds = []
for r in rows:
    if r["cond"] not in conds:
        conds.append(r["cond"])
cols = ["done", "progress", "wrong", "ignored", "wall_s", "turns", "tool_calls", "shots", "skipped", "tok_in", "tok_out",
        "interventions", "review", "acked", "gate_ms"]
print("cond      n  " + "  ".join(f"{c:>10}" for c in cols))
summary = {}
for c in conds:
    rs = [r for r in rows if r["cond"] == c]
    n = len(rs)
    agg = {}
    for k in cols:
        vals = [float(r.get(k) or 0) for r in rs]
        agg[k] = round(sum(vals) / n, 2) if k != "done" else sum(1 for r in rs if r.get("done"))
    summary[c] = {"n": n, **agg}
    print(f"{c:<9}{n:>2}  " + "  ".join(f"{agg[k]:>10}" for k in cols))
print()
for r in rows:
    print(json.dumps(r, ensure_ascii=False))
print("SUMMARY_JSON " + json.dumps(summary))
