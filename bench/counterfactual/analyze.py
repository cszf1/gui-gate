#!/usr/bin/env python3
"""Analyse counterfactual branches (harness.py output): paired increments per checkpoint, task-family bootstrap CIs,
the matched-wait contrast and an incremental reliability table against a model's predicted increments.

usage: python3 bench/counterfactual/analyze.py BRANCHES.jsonl [--pred PRED.jsonl] [--level step|task] [--md OUT.md]

Delta_o(checkpoint) = mean success over repeats under o - mean success under direct, on the SAME checkpoint (paired).
Only branches whose restore was verified (restored_ok) are used; the count of unfaithful restores is reported.
CIs resample task families (then checkpoints inside them), so states of one family never count as independent.
--pred: lines {"checkpoint", "treatment", "delta"} (e.g. the gate's value log `delta` mean for that state); the
reliability table bins checkpoints by predicted delta and compares with the paired observed delta. Negative and zero
increments are kept as they are.
"""
import json
import os
import random
import sys
from collections import defaultdict

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import stats  # noqa: E402


def load(p):
    return [json.loads(x) for x in open(p) if x.strip()]


def paired_deltas(recs, level="step"):
    key = "step_success" if level == "step" else "task_success"
    by = defaultdict(lambda: defaultdict(list))
    fam = {}
    for r in recs:
        if not r.get("restored_ok"):
            continue
        by[r["checkpoint"]][r["treatment"]].append(1.0 if r[key] else 0.0)
        fam[r["checkpoint"]] = r["family"]
    out = defaultdict(dict)  # treatment -> checkpoint -> delta
    for cp, t in by.items():
        if "direct" not in t:
            continue
        base = sum(t["direct"]) / len(t["direct"])
        for o, ys in t.items():
            if o != "direct":
                out[o][cp] = sum(ys) / len(ys) - base
    return out, fam


def family_bootstrap(deltas, fam, reps=2000, seed=0):
    fams = defaultdict(list)
    for cp, d in deltas.items():
        fams[fam[cp]].append(d)
    keys = sorted(fams)
    if not keys:
        return None, [None, None]
    mean = sum(deltas.values()) / len(deltas)
    rng = random.Random(seed)
    boots = []
    for _ in range(reps):
        xs = []
        for _ in keys:
            f = fams[keys[rng.randrange(len(keys))]]
            xs.extend(f[rng.randrange(len(f))] for _ in f)
        boots.append(sum(xs) / len(xs))
    boots.sort()
    return mean, [boots[int(0.025 * (reps - 1))], boots[int(0.975 * (reps - 1))]]


def costs(recs):
    c = defaultdict(lambda: [0.0, 0.0, 0.0, 0])
    for r in recs:
        x = c[r["treatment"]]
        x[0] += r["cost_s"]
        x[1] += r["tokens"]
        x[2] += r["person_s"]
        x[3] += 1
    return {t: {"cost_s": v[0] / v[3], "tokens": v[1] / v[3], "person_s": v[2] / v[3], "n": v[3]} for t, v in c.items()}


def reliability(deltas, preds, bins=5):
    """deltas: checkpoint -> observed delta; preds: checkpoint -> predicted delta. Equal-mass bins by prediction."""
    pts = sorted((preds[cp], d) for cp, d in deltas.items() if cp in preds)
    if not pts:
        return []
    n = len(pts)
    b = min(bins, n)
    rows = []
    for i in range(b):
        seg = pts[i * n // b:(i + 1) * n // b]
        if seg:
            rows.append({"n": len(seg), "pred_lo": seg[0][0], "pred_hi": seg[-1][0], "mean_pred": sum(p for p, _ in seg) / len(seg),
                         "mean_observed": sum(d for _, d in seg) / len(seg)})
    return rows


def analyze(recs, preds=None, level="step"):
    deltas, fam = paired_deltas(recs, level)
    res = {"level": level, "branches": len(recs), "unfaithful_restores": sum(1 for r in recs if not r.get("restored_ok")),
           "checkpoints": len({r["checkpoint"] for r in recs}), "families": sorted(set(fam.values())), "treatments": {},
           "costs": costs(recs)}
    for o, d in sorted(deltas.items()):
        mean, ci = family_bootstrap(d, fam)
        res["treatments"][o] = {"checkpoints": len(d), "delta": mean, "ci95_family_bootstrap": ci,
                                "negative": sum(1 for x in d.values() if x < 0), "zero": sum(1 for x in d.values() if x == 0),
                                "positive": sum(1 for x in d.values() if x > 0)}
    # does an observation help beyond the time it takes? look_light vs matched_wait on the same checkpoints
    if "look_light" in deltas and "matched_wait" in deltas:
        common = sorted(set(deltas["look_light"]) & set(deltas["matched_wait"]))
        a = [deltas["matched_wait"][c] for c in common]
        b = [deltas["look_light"][c] for c in common]
        res["look_light_minus_matched_wait"] = stats.paired_bootstrap(a, b, seed=3)
    if preds:
        res["incremental_reliability"] = {o: reliability(deltas[o], {cp: v for (cp, t), v in preds.items() if t == o}) for o in deltas}
    return res


def to_md(res):
    L = [f"# Counterfactual branches ({res['level']} success)", "",
         f"branches {res['branches']}, checkpoints {res['checkpoints']}, families {res['families']}, unfaithful restores {res['unfaithful_restores']}", "",
         "| treatment | checkpoints | delta vs direct | 95% CI (family bootstrap) | neg / zero / pos | mean cost s | tokens | person s |",
         "|---|---|---|---|---|---|---|---|"]
    for o, t in res["treatments"].items():
        c = res["costs"].get(o, {})
        ci = t["ci95_family_bootstrap"]
        L.append(f"| {o} | {t['checkpoints']} | {t['delta']:+.3f} | {ci[0]:+.3f}..{ci[1]:+.3f} | {t['negative']}/{t['zero']}/{t['positive']} | "
                 f"{c.get('cost_s', 0):.1f} | {c.get('tokens', 0):.0f} | {c.get('person_s', 0):.1f} |")
    if "look_light_minus_matched_wait" in res:
        x = res["look_light_minus_matched_wait"]
        L += ["", f"look_light minus matched_wait (information beyond waiting): {x['diff']:+.3f} [{x['ci'][0]:+.3f}, {x['ci'][1]:+.3f}]"]
    return "\n".join(L) + "\n"


def main(a):
    recs = load(a[0])
    flags = {a[i]: a[i + 1] for i in range(1, len(a) - 1) if a[i].startswith("--")}
    preds = None
    if flags.get("--pred"):
        preds = {(p["checkpoint"], p["treatment"]): p["delta"] for p in load(flags["--pred"])}
    res = analyze(recs, preds, flags.get("--level", "step"))
    md = to_md(res)
    if flags.get("--md"):
        open(flags["--md"], "w").write(md)
    print(md)
    return res


if __name__ == "__main__":
    main(sys.argv[1:])
