#!/usr/bin/env python3
"""Summarise sim_agent result lines (eval.jsonl, eval_extra.jsonl) per condition."""
import json
import sys
from collections import defaultdict


def cond(d):
    if d.get("tag"):
        return d["tag"]
    s = d["server"]
    if s.startswith("cua-driver"):
        return "raw-driver (agent re-reads every 3)"
    pol = s.split("--policy")[1].split()[0]
    if "--weights" in s:
        return "gate (trained)"
    return f"policy {pol}"


rows = defaultdict(list)
for fn in sys.argv[1:]:
    for line in open(fn):
        d = json.loads(line)
        rows[cond(d)].append(d)

print("| condition | runs | task done | wrong clicks | clicks on busy/blocked UI | model turns | screenshots | virtual time (s) |")
print("|---|---|---|---|---|---|---|---|")
for c, ds in rows.items():
    n = len(ds)
    avg = lambda k: sum(x[k] for x in ds) / n
    print(f"| {c} | {n} | {sum(x['done'] for x in ds)}/{n} | {sum(x['wrong_clicks'] for x in ds)} | "
          f"{sum(x['ignored_clicks'] for x in ds)} | {avg('turns'):.1f} | {avg('screenshots'):.1f} | {avg('virtual_s'):.1f} |")
