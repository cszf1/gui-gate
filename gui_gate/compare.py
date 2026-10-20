"""Summarise gate logs per policy (for baseline comparisons).

    gui-gate-compare LOG_DIR_OR_FILES...
"""
from __future__ import annotations

import argparse
import glob
import json
import os
from collections import defaultdict


def main(argv=None):
    ap = argparse.ArgumentParser(prog="gui-gate-compare")
    ap.add_argument("paths", nargs="+")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args(argv)
    files = []
    for p in a.paths:
        files += sorted(glob.glob(os.path.join(p, "*.jsonl"))) if os.path.isdir(p) else [p]
    agg = defaultdict(lambda: defaultdict(float))
    sess_policy = {}
    retro = set()
    evs = []
    for fn in files:
        for line in open(fn, encoding="utf-8"):
            try:
                e = json.loads(line)
            except json.JSONDecodeError:
                continue
            evs.append(e)
            if e.get("ev") == "action":
                sess_policy.setdefault(e["session"], e.get("policy"))
            if e.get("ev") == "retro_repeat":
                retro.add((e["session"], e.get("aid")))
    for e in evs:
        pol = sess_policy.get(e.get("session"), "?")
        g = agg[pol]
        g["sessions_set"] = 0
        if e.get("ev") == "obs":
            g["observations"] += 1
            g["screenshots"] += 1 if e.get("screenshot") else 0
            g["image_tokens"] += (e.get("px") or 0) / 750
            g["driver_obs_ms"] += e.get("ms") or 0
        elif e.get("ev") == "action":
            g["actions"] += 1
            g["gate_ms"] += e.get("gate_ms") or 0
            if e.get("executed"):
                g["executed"] += 1
                g["exec_ms"] += e.get("exec_ms") or 0
                bad = e.get("is_error") or e.get("stale_error") or (e["session"], e.get("aid")) in retro
                g["failed"] += 1 if bad else 0
            else:
                g["interventions"] += 1
                g["int_" + e.get("decision", "?")] += 1
    out = {}
    for pol, g in agg.items():
        sess = {s for s, p in sess_policy.items() if p == pol}
        d = {k: round(v, 1) for k, v in g.items() if k != "sessions_set"}
        d["sessions"] = len(sess)
        if g["executed"]:
            d["fail_rate_executed"] = round(g["failed"] / g["executed"], 3)
        if g["actions"]:
            d["gate_ms_per_action"] = round(g["gate_ms"] / g["actions"], 1)
        out[pol] = d
    if a.json:
        print(json.dumps(out, indent=1))
        return
    for pol, d in out.items():
        print(f"== policy {pol}")
        for k in sorted(d):
            print(f"   {k}: {d[k]}")


if __name__ == "__main__":
    main()
