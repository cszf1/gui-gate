#!/usr/bin/env python3
"""Per-condition use of the 0.9.6 features in a run directory (Pi event streams + gate logs).

usage: python3 bench/tools096_stats.py RUNDIR [--md OUT.md] [--json OUT.json]

Counts, valid runs only (runs.jsonl infra == null): agent tool calls per tool (run_command, check_done, set_plan,
run_actions, ...), plan verdicts (feasible:false), check_done gap kinds, loop-breaker notes by level and refusals,
AT-SPI extras attached (Calc cells / terminal text, full vs diff/unchanged), commands run (exit != 0, timed out),
reviewer calls, gate decisions, turns, max-turn / timeout stops, model s/turn (from the .times sidecar when present).
"""
import collections
import glob
import json
import os
import sys


def jl(path):
    out = []
    try:
        with open(path, errors="replace") as f:
            for line in f:
                try:
                    out.append(json.loads(line))
                except Exception:
                    pass
    except OSError:
        pass
    return out


def main(argv):
    if not argv:
        print(__doc__)
        return 2
    rd = argv[0]
    md = argv[argv.index("--md") + 1] if "--md" in argv else None
    js = argv[argv.index("--json") + 1] if "--json" in argv else None
    runs = [r for r in jl(os.path.join(rd, "runs.jsonl")) if not r.get("infra")]
    agg = collections.defaultdict(lambda: collections.defaultdict(collections.Counter))
    nruns = collections.Counter()
    per_run = []
    for r in runs:
        c, tag = r["cond"], r["tag"]
        nruns[c] += 1
        A = agg[c]
        pi = r.get("pi") or {}
        A["stop"][pi.get("stop")] += 1
        A["sum"]["turns"] += pi.get("turns") or 0
        A["sum"]["wall_s"] += pi.get("wall_s") or 0
        calls = collections.Counter()
        for e in jl(os.path.join(rd, "pi", tag + ".jsonl")):
            if e.get("type") == "tool_execution_start":
                calls[(e.get("toolName") or "").replace("mcp__gui__", "")] += 1
        for k, v in calls.items():
            A["calls"][k] += v
            A["runs_using"][k] += 1
        ev = collections.Counter()
        for g in jl(os.path.join(rd, "gate", tag + ".jsonl")):
            t = g.get("ev")
            if t == "action":
                A["decision"][g.get("decision")] += 1
                # same rule as bench/timing.py: a step review (`judge`) or a risk review (`risk_gate.reviewer`)
                rg = g.get("risk_gate")
                if isinstance(g.get("judge"), dict):
                    A["sum"]["reviewer_calls"] += 1
                    A["reviewer"]["step"] += 1
                if isinstance(rg, dict) and "reviewer" in rg:
                    A["sum"]["reviewer_calls"] += 1
                    A["reviewer"]["risk"] += 1
            elif t == "loop_breaker":
                A["loop"]["refused" if g.get("refused") else "level%s" % g.get("level")] += 1
                ev["loop_max"] = max(ev["loop_max"], int(g.get("level") or 0))
            elif t == "tree_extra":
                k = g.get("kind")
                if g.get("error"):
                    A["extra"][k + "_error"] += 1
                elif k == "cells":
                    A["extra"]["cells_diff" if g.get("diff") else "cells_full"] += 1
                else:
                    A["extra"]["term_same" if g.get("same") else "term_text"] += 1
            elif t == "command":
                A["cmd"]["run"] += 1
                if g.get("exit") not in (0, None):
                    A["cmd"]["exit_nonzero"] += 1
                if g.get("timed_out"):
                    A["cmd"]["timed_out"] += 1
            elif t == "check_done":
                A["check"]["calls"] += 1
                for k in g.get("kinds") or []:
                    A["check"][k] += 1
            elif t == "plan":
                A["plan"]["infeasible" if g.get("feasible") is False else "set"] += 1
            elif t == "request_human":
                A["sum"]["human_requests"] += 1
            elif t == "batch":
                A["sum"]["batches"] += 1
        if ev["loop_max"]:
            A["loop_runs"]["max_level%d" % ev["loop_max"]] += 1
        per_run.append({"tag": tag, "cond": c, "calls": dict(calls), "turns": pi.get("turns"), "stop": pi.get("stop")})
    out = {c: {k: dict(v) for k, v in A.items()} for c, A in agg.items()}
    for c in out:
        out[c]["valid_runs"] = nruns[c]
    if js:
        json.dump({"by_cond": out, "runs": per_run}, open(js, "w"), indent=1)
    lines = ["# 0.9.6 feature use (bench/tools096_stats.py)", "", "Valid runs only (infra rows excluded).", ""]
    for c in sorted(out):
        o = out[c]
        n = max(1, o["valid_runs"])
        lines.append(f"## {c} ({o['valid_runs']} valid runs)")
        lines.append(f"- stops: {o.get('stop', {})}; mean turns {o.get('sum', {}).get('turns', 0) / n:.1f}; mean wall {o.get('sum', {}).get('wall_s', 0) / n:.1f} s")
        calls = o.get("calls", {})
        ru = o.get("runs_using", {})
        for k in ["run_command", "check_done", "set_plan", "run_actions", "request_human", "get_window_state", "click", "press_key", "type_text"]:
            lines.append(f"- {k}: {calls.get(k, 0)} calls in {ru.get(k, 0)} runs")
        lines.append(f"- plan: {o.get('plan', {})}; check_done gaps: {o.get('check', {})}")
        lines.append(f"- commands (gate log): {o.get('cmd', {})}")
        lines.append(f"- loop breaker events: {o.get('loop', {})}; runs by max level: {o.get('loop_runs', {})}")
        lines.append(f"- AT-SPI extras: {o.get('extra', {})}")
        lines.append(f"- gate decisions: {o.get('decision', {})}; reviewer calls {o.get('sum', {}).get('reviewer_calls', 0)} {o.get('reviewer', {})}; human requests {o.get('sum', {}).get('human_requests', 0)}; batches {o.get('sum', {}).get('batches', 0)}")
        lines.append("")
    txt = "\n".join(lines)
    if md:
        open(md, "w").write(txt + "\n")
    print(txt)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
