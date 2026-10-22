#!/usr/bin/env python3
"""Compare two packed runs (e.g. v094 baseline vs v096) per condition and per task.

usage: python3 bench/compare_runs.py BASE_DIR NEW_DIR [--md OUT.md]
Reads runs_scored.jsonl (bench/score.py) and timing.json (bench/timing.py) of each dir. Valid rows only
(infra rows excluded; when a tag was retried, the valid attempt counts).
"""
import collections
import json
import os
import sys


def rows(d):
    out = {}
    for line in open(os.path.join(d, "runs_scored.jsonl")):
        r = json.loads(line)
        if r.get("infra"):
            continue
        out[(r["task"], r["cond"], r.get("seed", 0))] = r
    return out


def timing(d):
    try:
        return json.load(open(os.path.join(d, "timing.json")))
    except Exception:
        return {}


def agg(rs, cond, seed=None):
    v = [r for (t, c, s), r in rs.items() if c == cond and (seed is None or s == seed)]
    ow = [r for r in v if r["task"].startswith("ow")]
    og = [r for r in v if r["task"].startswith("og")]
    n = max(1, len(v))
    stop = collections.Counter((r.get("stop") or r.get("pi_stop") or "?") for r in v)
    return {
        "runs": len(v),
        "success": sum(1 for r in v if r.get("success")),
        "ow": f"{sum(1 for r in ow if r.get('success'))}/{len(ow)}",
        "og": f"{sum(1 for r in og if r.get('success'))}/{len(og)}",
        "og_safe": sum(1 for r in og if r.get("safe_success")),
        "og_unsafe": sum(1 for r in og if r.get("unsafe_completion")),
        "unsafe_tasks": sorted(r["task"][:4] for r in og if r.get("unsafe_completion")),
        "turns": round(sum(r.get("turns") or 0 for r in v) / n, 1),
        "wall": round(sum(r.get("wall_s") or 0 for r in v) / n, 1),
        "tok_in": round(sum(r.get("tok_in") or 0 for r in v) / n),
        "stops": dict(stop),
    }


def main(a):
    base, new = a[0], a[1]
    md = a[a.index("--md") + 1] if "--md" in a else None
    B, N = rows(base), rows(new)
    TB, TN = timing(base), timing(new)
    L = [f"# {os.path.basename(new.rstrip('/'))} vs {os.path.basename(base.rstrip('/'))}", ""]
    L.append("| run | cond | runs | success | OSWorld | OSGuard | OG safe succ | OG unsafe | mean turns | mean wall s | tok in/run | stops |")
    L.append("|---|---|---|---|---|---|---|---|---|---|---|---|")
    seeds = sorted({s for (_, _, s) in N})
    for name, R, ss in ((os.path.basename(base.rstrip('/')), B, [None]), (os.path.basename(new.rstrip('/')), N, [None] + (seeds if len(seeds) > 1 else []))):
        for s in ss:
            for c in ("direct", "gate8"):
                g = agg(R, c, s)
                if not g["runs"]:
                    continue
                lab = name + ("" if s is None else f" s{s}")
                L.append(f"| {lab} | {c} | {g['runs']} | {g['success']} | {g['ow']} | {g['og']} | {g['og_safe']} | {g['og_unsafe']} {g['unsafe_tasks']} | {g['turns']} | {g['wall']} | {g['tok_in']} | {g['stops']} |")
    L += ["", "Per task (seed 0): base -> new, 1 = success, U = unsafe completion", "",
          "| task | direct base | direct new | gate8 base | gate8 new |", "|---|---|---|---|---|"]
    tasks = sorted({t for (t, _, _) in list(B) + list(N)}, key=lambda t: (t[:2] != "ow", t))

    def cell(R, t, c):
        r = R.get((t, c, 0))
        if not r:
            return "-"
        return ("1" if r.get("success") else "0") + ("U" if r.get("unsafe_completion") else "") + f" ({r.get('turns')}t)"
    for t in tasks:
        L.append(f"| {t[:28]} | {cell(B, t, 'direct')} | {cell(N, t, 'direct')} | {cell(B, t, 'gate8')} | {cell(N, t, 'gate8')} |")
    txt = "\n".join(L)
    if md:
        open(md, "w").write(txt + "\n")
    print(txt)


if __name__ == "__main__":
    main(sys.argv[1:])
