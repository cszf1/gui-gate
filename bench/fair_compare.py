#!/usr/bin/env python3
"""Fair baseline comparison of value policies on the same engineering base (P2).

usage: python3 bench/fair_compare.py RUN_DIR [RUN_DIR...] [--baseline vp_direct] [--policies a,b,...]
                                     [--planned TASKLIST] [--cost wall_s|tok_in|turns] [--md OUT.md] [--json OUT.json]

Reads runs_scored.jsonl (bench/score.py) of every RUN_DIR. One row per (task, cond, seed) = the LAST attempt.
Two denominators, both reported:
  planned - every task of the frozen list (--planned, else every task seen) x every repeat; missing runs and infra
            failures (relay / setup / eval errors) count as failures. This is the primary analysis: a policy that
            makes more API calls must not lose its API errors from the denominator.
  valid   - runs without an infra failure only (descriptive).
Per policy: success with a Wilson 95% CI, mean cost (all runs, failures and timeouts included).
Per policy vs the baseline: task-level paired difference of the per-task success rate (mean over repeats) with a
bootstrap 95% CI over tasks, a sign-flip permutation p over tasks, and an exact McNemar p over (task, seed) pairs.
Budget curve: share of planned runs that succeeded within each cost budget (success vs cost).
Seeds are repeat labels: the model API takes no sampling seed, so these are independent repeats, not seeded runs.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import stats  # noqa: E402

COSTS = ("wall_s", "tok_in", "turns")


def load(dirs):
    best = {}
    for d in dirs:
        p = os.path.join(d, "runs_scored.jsonl")
        for line in open(p):
            r = json.loads(line)
            k = (r["task"], r["cond"], r.get("seed", 0))
            if k not in best or (r.get("attempt") or 1) >= (best[k].get("attempt") or 1):
                best[k] = r
    return best


def table(best, policies, tasks, seeds, cost):
    """planned rows: one per (task, policy, seed), infra / missing = failure with the cost it had (missing: None)"""
    out = []
    for p in policies:
        for t in tasks:
            for s in seeds:
                r = best.get((t, p, s))
                ok = bool(r and not r.get("infra") and r.get("success"))
                out.append({"policy": p, "task": t, "seed": s, "success": ok, "present": r is not None,
                            "infra": (r or {}).get("infra") if r else "missing", cost: (r or {}).get(cost)})
    return out


def summarize(rows, policies, baseline, cost, budgets):
    res = {"policies": {}, "paired": {}, "budget_curve": {}}
    for p in policies:
        rs = [r for r in rows if r["policy"] == p]
        k = sum(r["success"] for r in rs)
        valid = [r for r in rs if r["present"] and not r["infra"]]
        kv = sum(r["success"] for r in valid)
        costs = [r[cost] for r in rs if r[cost] is not None]
        res["policies"][p] = {
            "planned_runs": len(rs), "success": k, "rate": k / len(rs) if rs else 0.0, "wilson95": stats.wilson(k, len(rs)),
            "valid_runs": len(valid), "valid_success": kv, "valid_rate": kv / len(valid) if valid else None,
            "valid_wilson95": stats.wilson(kv, len(valid)) if valid else None,
            "infra_or_missing": len(rs) - len(valid), f"mean_{cost}": sum(costs) / len(costs) if costs else None}
    tasks = sorted({r["task"] for r in rows})
    for p in policies:
        if p == baseline:
            continue

        def per_task(pol):
            return [sum(r["success"] for r in rows if r["policy"] == pol and r["task"] == t) /
                    max(1, sum(1 for r in rows if r["policy"] == pol and r["task"] == t)) for t in tasks]
        a, b = per_task(baseline), per_task(p)
        pairs = sorted({(r["task"], r["seed"]) for r in rows})
        look = {(r["policy"], r["task"], r["seed"]): r["success"] for r in rows}
        mc = stats.mcnemar_exact([look[(baseline, t, s)] for t, s in pairs], [look[(p, t, s)] for t, s in pairs])
        res["paired"][p] = {"vs": baseline, "tasks": len(tasks), "diff_bootstrap": stats.paired_bootstrap(a, b, seed=1),
                            "permutation": stats.paired_permutation(a, b, seed=1), "mcnemar_task_seed": mc}
    res["budget_curve"] = {p: [list(x) for x in v] for p, v in stats.budget_curve(rows, budgets, cost).items()}
    return res


def budgets_for(rows, cost, k=8):
    v = sorted(r[cost] for r in rows if r[cost] is not None)
    if not v:
        return []
    return sorted({round(v[min(len(v) - 1, int(i * (len(v) - 1) / (k - 1)))], 1) for i in range(k)})


def to_md(res, cost):
    L = ["# Fair value-policy comparison", "",
         "Same engineering base (same gate, tools, prompt, model, turn and time budget); only GUI_GATE_VALUE_POLICY differs.",
         "Seeds are independent repeats (no sampling seed reaches the model API).", "",
         f"| policy | planned runs | success | rate | Wilson 95% | valid runs | valid rate | infra/missing | mean {cost} |",
         "|---|---|---|---|---|---|---|---|---|"]
    for p, a in res["policies"].items():
        w = a["wilson95"]
        vr = "-" if a["valid_rate"] is None else f"{a['valid_rate']:.3f}"
        mc = a[f"mean_{cost}"]
        L.append(f"| {p} | {a['planned_runs']} | {a['success']} | {a['rate']:.3f} | {w[0]:.3f}-{w[1]:.3f} | {a['valid_runs']} | {vr} | "
                 f"{a['infra_or_missing']} | {'-' if mc is None else round(mc, 1)} |")
    L += ["", "## Paired (task level)", "", "| policy | vs | tasks | diff | 95% CI | permutation p | McNemar n01/n10 | McNemar p |",
          "|---|---|---|---|---|---|---|---|"]
    for p, d in res["paired"].items():
        bt = d["diff_bootstrap"]
        L.append(f"| {p} | {d['vs']} | {d['tasks']} | {bt['diff']:+.3f} | {bt['ci'][0]:+.3f}..{bt['ci'][1]:+.3f} | "
                 f"{d['permutation']['p']:.4f} | {d['mcnemar_task_seed']['n01']}/{d['mcnemar_task_seed']['n10']} | {d['mcnemar_task_seed']['p']:.4f} |")
    L += ["", f"## Budget curve (success within {cost} budget, planned runs)", ""]
    pols = list(res["budget_curve"])
    if pols:
        bud = [x[0] for x in res["budget_curve"][pols[0]]]
        L.append("| budget | " + " | ".join(pols) + " |")
        L.append("|---|" + "---|" * len(pols))
        for i, b in enumerate(bud):
            L.append(f"| {b} | " + " | ".join(f"{res['budget_curve'][p][i][1]:.3f}" for p in pols) + " |")
    L += ["", "A non-significant difference is not evidence of equality; claims need a pre-registered margin."]
    return "\n".join(L) + "\n"


def main(a):
    flags, dirs, i = {}, [], 0
    while i < len(a):
        if a[i].startswith("--"):
            flags[a[i]] = a[i + 1]
            i += 2
        else:
            dirs.append(a[i])
            i += 1
    opt = flags.get
    cost = opt("--cost", "wall_s")
    assert cost in COSTS, cost
    best = load(dirs)
    policies = opt("--policies").split(",") if opt("--policies") else sorted({k[1] for k in best})
    baseline = opt("--baseline", "vp_direct" if "vp_direct" in policies else policies[0])
    if baseline not in policies:
        policies = [baseline] + policies
    if opt("--planned"):
        tasks = [os.path.basename(x.strip())[:-5] if x.strip().endswith(".json") else x.strip() for x in open(opt("--planned")) if x.strip()]
    else:
        tasks = sorted({k[0] for k in best if k[1] in policies})
    seeds = sorted({k[2] for k in best if k[1] in policies}) or [0]
    rows = table(best, policies, tasks, seeds, cost)
    res = summarize(rows, policies, baseline, cost, budgets_for(rows, cost))
    res["planned_tasks"], res["repeats"], res["cost"] = len(tasks), seeds, cost
    md = to_md(res, cost)
    if opt("--md"):
        open(opt("--md"), "w").write(md)
    if opt("--json"):
        json.dump(res, open(opt("--json"), "w"), indent=1)
    print(md)
    return res


if __name__ == "__main__":
    main(sys.argv[1:])
