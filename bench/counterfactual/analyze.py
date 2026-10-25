#!/usr/bin/env python3
"""Analyse counterfactual branches (harness.py output): paired increments per checkpoint, task-family bootstrap CIs,
the matched-wait contrast and an incremental reliability table against a model's predicted increments.

usage: python3 bench/counterfactual/analyze.py BRANCHES.jsonl [--pred PRED.jsonl] [--level step|task] [--md OUT.md]
                                              [--plot OUT.png] [--bins 5]

Delta_o(checkpoint) = mean success over repeats under o - mean success under direct, on the SAME checkpoint (paired).
Only branches whose restore was verified (restored_ok) are used; the count of unfaithful restores is reported.
CIs resample task families (then checkpoints inside them), so states of one family never count as independent.
--pred: lines {"checkpoint", "treatment", "delta"} (e.g. the gate's value log `delta` mean for that state); the
reliability table bins checkpoints by predicted delta and compares with the paired observed delta. Negative and zero
increments are kept as they are.
--plot: the incremental reliability diagram (predicted vs paired observed increment per equal-mass bin, one panel per
treatment, task-family bootstrap 95% CIs per bin, diagonal = perfect calibration). PNG via matplotlib when it is
installed; otherwise (or with GG_NO_MATPLOTLIB=1) OUT.csv + OUT.txt (ASCII). The CSV is written in both cases.
Prediction lines may also carry "q" (predicted success probability under the treatment): then Brier and NLL of q
against the branch outcomes are reported too (AUC alone says nothing about calibration of increments).
"""
import csv
import json
import math
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


def _bins(pts, bins):
    """equal-mass bins over points sorted by prediction; a bin never splits equal predictions (ties stay together, so
    the order of observed values inside a tie cannot manufacture a slope)."""
    n = len(pts)
    b = min(bins, n)
    cuts, start = [], 0
    for i in range(1, b + 1):
        end = i * n // b
        while 0 < end < n and pts[end][0] == pts[end - 1][0]:
            end += 1
        if end > start:
            cuts.append((start, end))
            start = end
    if start < n:
        cuts.append((start, n))
    return [pts[a:z] for a, z in cuts]


def reliability(deltas, preds, bins=5):
    """deltas: checkpoint -> observed delta; preds: checkpoint -> predicted delta. Equal-mass bins by prediction."""
    pts = sorted(((preds[cp], cp, d) for cp, d in deltas.items() if cp in preds), key=lambda x: (x[0], x[1]))
    rows = []
    for seg in _bins(pts, bins) if pts else []:
        rows.append({"n": len(seg), "pred_lo": seg[0][0], "pred_hi": seg[-1][0], "mean_pred": sum(p for p, _, _ in seg) / len(seg),
                     "mean_observed": sum(d for _, _, d in seg) / len(seg), "checkpoints": [cp for _, cp, _ in seg]})
    return rows


def reliability_ci(deltas, preds, fam, bins=5, reps=1000, seed=0):
    """reliability() with a task-family bootstrap 95% CI of the observed mean per bin (families resampled inside the
    bin, then checkpoints inside each family), so correlated states of one family do not shrink the interval."""
    rows = reliability(deltas, preds, bins)
    rng = random.Random(seed)
    for row in rows:
        seg = [(None, deltas[cp], cp) for cp in row.pop("checkpoints")]
        byf = defaultdict(list)
        for _, d, cp in seg:
            byf[fam.get(cp, cp)].append(d)
        keys = sorted(byf)
        boots = []
        for _ in range(reps):
            xs = []
            for _ in keys:
                f = byf[keys[rng.randrange(len(keys))]]
                xs.extend(f[rng.randrange(len(f))] for _ in f)
            boots.append(sum(xs) / len(xs))
        boots.sort()
        row["ci95"] = [boots[int(0.025 * (reps - 1))], boots[int(0.975 * (reps - 1))]]
        row["families"] = len(keys)
    return rows


def prob_scores(recs, qpreds, level="step"):
    """Brier and NLL of predicted success probabilities q (checkpoint, treatment) against branch outcomes."""
    key = "step_success" if level == "step" else "task_success"
    out = defaultdict(lambda: [0.0, 0.0, 0])
    for r in recs:
        q = qpreds.get((r["checkpoint"], r["treatment"]))
        if q is None or not r.get("restored_ok"):
            continue
        y = 1.0 if r[key] else 0.0
        qq = min(1 - 1e-6, max(1e-6, q))
        o = out[r["treatment"]]
        o[0] += (qq - y) ** 2
        o[1] += -(y * math.log(qq) + (1 - y) * math.log(1 - qq))
        o[2] += 1
    return {t: {"brier": v[0] / v[2], "nll": v[1] / v[2], "n": v[2]} for t, v in out.items() if v[2]}


def _ascii_panel(t, rows, width=41, height=15):
    """ASCII reliability panel: x = predicted, y = observed, '*' bin mean, '|' CI, '.' diagonal."""
    vals = [v for r in rows for v in (r["mean_pred"], r["ci95"][0], r["ci95"][1])]
    lo, hi = min(vals + [-0.1]), max(vals + [0.1])
    span = hi - lo or 1.0
    grid = [[" "] * width for _ in range(height)]

    def xy(px, py):
        return (min(width - 1, max(0, int(round((px - lo) / span * (width - 1))))),
                min(height - 1, max(0, int(round((hi - py) / span * (height - 1))))))
    for i in range(width):
        v = lo + span * i / (width - 1)
        x, y = xy(v, v)
        grid[y][x] = "."
    zx, zy = xy(0.0, 0.0)
    for i in range(width):
        if grid[zy][i] == " ":
            grid[zy][i] = "-"
    for r in rows:
        x, y0 = xy(r["mean_pred"], r["ci95"][1])
        _, y1 = xy(r["mean_pred"], r["ci95"][0])
        for y in range(y0, y1 + 1):
            grid[y][x] = "|"
        _, ym = xy(r["mean_pred"], r["mean_observed"])
        grid[ym][x] = "*"
    lines = [f"{t}: predicted (x) vs observed paired increment (y), range {lo:+.2f}..{hi:+.2f}; '.' = perfect, '-' = zero"]
    lines += ["  " + "".join(g) for g in grid]
    return "\n".join(lines)


def plot_reliability(rel, path, title="Incremental reliability", force_ascii=False):
    """rel: {treatment: reliability_ci rows}. Writes <path stem>.csv always; the PNG with matplotlib, else <stem>.txt.
    Returns {"png": path|None, "csv": ..., "ascii": ...|None}."""
    stem = path[:-4] if path.lower().endswith(".png") else path
    with open(stem + ".csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["treatment", "bin", "n", "families", "pred_lo", "pred_hi", "mean_pred", "mean_observed", "ci_lo", "ci_hi"])
        for t, rows in sorted(rel.items()):
            for i, r in enumerate(rows):
                w.writerow([t, i, r["n"], r.get("families"), round(r["pred_lo"], 4), round(r["pred_hi"], 4),
                            round(r["mean_pred"], 4), round(r["mean_observed"], 4), round(r["ci95"][0], 4), round(r["ci95"][1], 4)])
    res = {"png": None, "csv": stem + ".csv", "ascii": None}
    plt = None
    if not force_ascii and os.environ.get("GG_NO_MATPLOTLIB") != "1":
        try:
            import matplotlib
            matplotlib.use("Agg")
            import matplotlib.pyplot as plt  # noqa: F811
        except Exception:
            plt = None
    ts = [t for t in sorted(rel) if rel[t]]
    if plt is not None and ts:
        cols = min(3, len(ts))
        nrow = (len(ts) + cols - 1) // cols
        fig, axes = plt.subplots(nrow, cols, figsize=(4.2 * cols, 3.8 * nrow), squeeze=False)
        for k, t in enumerate(ts):
            ax = axes[k // cols][k % cols]
            rows = rel[t]
            xs = [r["mean_pred"] for r in rows]
            ys = [r["mean_observed"] for r in rows]
            err = [[y - r["ci95"][0] for y, r in zip(ys, rows)], [r["ci95"][1] - y for y, r in zip(ys, rows)]]
            vals = xs + [r["ci95"][0] for r in rows] + [r["ci95"][1] for r in rows]
            lo, hi = min(vals + [-0.05]), max(vals + [0.05])
            pad = 0.05 * (hi - lo)
            ax.plot([lo - pad, hi + pad], [lo - pad, hi + pad], ls="--", color="grey", lw=1, label="perfect")
            ax.axhline(0, color="black", lw=0.6)
            ax.axvline(0, color="black", lw=0.6)
            ax.errorbar(xs, ys, yerr=err, fmt="o", capsize=3, label="bin mean, 95% CI (family bootstrap)")
            for x, y, r in zip(xs, ys, rows):
                ax.annotate(str(r["n"]), (x, y), textcoords="offset points", xytext=(4, 4), fontsize=7)
            ax.set_title(t, fontsize=10)
            ax.set_xlabel("predicted increment vs direct")
            ax.set_ylabel("observed paired increment")
            ax.set_xlim(lo - pad, hi + pad)
            ax.set_ylim(lo - pad, hi + pad)
        for k in range(len(ts), nrow * cols):
            axes[k // cols][k % cols].axis("off")
        axes[0][0].legend(fontsize=7, loc="upper left")
        fig.suptitle(title, fontsize=11)
        fig.tight_layout()
        fig.savefig(stem + ".png", dpi=110)
        plt.close(fig)
        res["png"] = stem + ".png"
    else:
        txt = title + "\n\n" + "\n\n".join(_ascii_panel(t, rel[t]) for t in ts) + "\n"
        open(stem + ".txt", "w").write(txt)
        res["ascii"] = txt
    return res


def analyze(recs, preds=None, level="step", qpreds=None, bins=5):
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
        res["incremental_reliability"] = {o: reliability_ci(deltas[o], {cp: v for (cp, t), v in preds.items() if t == o}, fam, bins)
                                          for o in deltas}
    if qpreds:
        res["probability_scores"] = prob_scores(recs, qpreds, level)
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
    rel = res.get("incremental_reliability") or {}
    if any(rel.values()):
        L += ["", "## Incremental reliability (equal-mass bins of predicted increment; observed = paired, family-bootstrap 95% CI)", "",
              "| treatment | bin | n | families | mean predicted | mean observed | 95% CI |", "|---|---|---|---|---|---|---|"]
        for o, rows in rel.items():
            for i, r in enumerate(rows):
                L.append(f"| {o} | {i} | {r['n']} | {r.get('families', '-')} | {r['mean_pred']:+.3f} | {r['mean_observed']:+.3f} | "
                         f"{r['ci95'][0]:+.3f}..{r['ci95'][1]:+.3f} |")
    ps = res.get("probability_scores") or {}
    if ps:
        L += ["", "| treatment | Brier | NLL | n |", "|---|---|---|---|"] + [f"| {t} | {v['brier']:.4f} | {v['nll']:.4f} | {v['n']} |" for t, v in sorted(ps.items())]
    return "\n".join(L) + "\n"


def main(a):
    recs = load(a[0])
    flags = {a[i]: a[i + 1] for i in range(1, len(a) - 1) if a[i].startswith("--")}
    preds = qpreds = None
    if flags.get("--pred"):
        pl = load(flags["--pred"])
        preds = {(p["checkpoint"], p["treatment"]): p["delta"] for p in pl if p.get("delta") is not None}
        qpreds = {(p["checkpoint"], p["treatment"]): p["q"] for p in pl if p.get("q") is not None} or None
    res = analyze(recs, preds, flags.get("--level", "step"), qpreds, int(flags.get("--bins", 5)))
    if flags.get("--plot") and res.get("incremental_reliability"):
        res["plot"] = plot_reliability(res["incremental_reliability"], flags["--plot"],
                                       f"Incremental reliability ({res['level']} success)")
    md = to_md(res)
    if flags.get("--md"):
        open(flags["--md"], "w").write(md)
    print(md)
    return res


if __name__ == "__main__":
    main(sys.argv[1:])
