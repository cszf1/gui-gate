#!/usr/bin/env python3
"""Offline end-to-end dry run of the P1 protocol on the fixture twin (no desktop, no model, no network).

usage: python3 bench/counterfactual/dryrun.py --out DIR [--gate target/release/gui-gate] [--seeds 3] [--prefixes 2,5]
                                              [--repeats 2] [--restore snapshot|replay] [--families relabel,loading,popup,mixed]
                                              [--unseen-layouts N]

Builds checkpoints (family x seed x prefix length) with the fixed policy pi, branches each under all seven
treatments (harness.TREATMENTS) in a shuffled order with repeats, verifies every restore by state hash, splits the
checkpoints by TASK FAMILY (train / test families never share a checkpoint), and writes:
  DIR/checkpoints.jsonl  DIR/branches.jsonl  DIR/split.json  DIR/analysis.json  DIR/analysis.md
  DIR/preds.jsonl        a toy cell-mean increment predictor FITTED ON THE TRAIN FAMILIES ONLY (treatment x the
                         controller's blind-action count x candidate kind), applied to every checkpoint
  DIR/reliability.png    incremental reliability diagram on the TEST families (+ .csv; .txt when no matplotlib)
  DIR/reliability_unseen.png  the same on the held-out unseen layouts (--unseen-layouts N > 0), never used for fitting
With --gate every action goes through the real gui-gate binary (fixture_driver.py behind it; slower).
"""
import json
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import analyze  # noqa: E402
import harness  # noqa: E402


def family_split(families, test_frac=0.5, seed=0):
    fams = sorted(families)
    random.Random(seed).shuffle(fams)
    k = max(1, int(round(len(fams) * test_frac))) if len(fams) > 1 else 0
    return {"test_families": sorted(fams[:k]), "train_families": sorted(fams[k:]),
            "rule": "every checkpoint, branch and repeat of a family is on one side only"}


def x_features(cp):
    """what is visible BEFORE the intervention: the controller's blind actions since its last read, candidate kind"""
    b = cp["controller"]["b"]
    seq = b.get("seq") or []
    return (min(3, b.get("since_read", 0)), "ok" if cp["candidate"] == "ok" else "button",
            "late" if seq and b.get("k", 0) >= len(seq) // 2 else "early")


def fit_predictor(cps, recs, train_families, level="step"):
    """cell means of the paired increment per (treatment, x_features) on train-family checkpoints; fallback = the
    treatment's train mean, then 0. Deliberately simple: the point is the protocol, not the model."""
    deltas, _ = analyze.paired_deltas([r for r in recs if r["family"] in train_families], level)
    feat = {c["id"]: x_features(c) for c in cps}
    cell, tmean = {}, {}
    for o, d in deltas.items():
        tmean[o] = sum(d.values()) / len(d) if d else 0.0
        acc = {}
        for cp, v in d.items():
            acc.setdefault(feat[cp], []).append(v)
        for k, vs in acc.items():
            cell[(o, k)] = sum(vs) / len(vs)
    preds = []
    for c in cps:
        if c["done"]:
            continue
        for o in harness.TREATMENTS:
            if o == "direct":
                continue
            v = cell.get((o, feat[c["id"]]), tmean.get(o, 0.0))
            preds.append({"checkpoint": c["id"], "treatment": o, "delta": v, "family": c["family"],
                          "predictor": "cell-mean on train families"})
    return preds


def run(out, gate=None, seeds=3, prefixes=(2, 5), repeats=2, restore="snapshot", families=None, length=8, budget_s=160.0,
        unseen_layouts=0):
    os.makedirs(out, exist_ok=True)
    fams = families or list(harness.FAMILIES)
    mk = harness.gate_backend(gate) if gate else harness.sim_backend
    cps = [harness.make_checkpoint(mk, seed, fam, length, n, budget_s) for fam in fams for seed in range(seeds) for n in prefixes]
    with open(os.path.join(out, "checkpoints.jsonl"), "w") as f:
        for c in cps:
            f.write(json.dumps(c) + "\n")
    recs = harness.run_branches(cps, mk, repeats=repeats, seed=1, restore_mode=restore, out=os.path.join(out, "branches.jsonl"))
    split = family_split(fams)
    json.dump(split, open(os.path.join(out, "split.json"), "w"), indent=1)
    # held-out unseen layouts (layouts.py): own families, never in the train split, never used for fitting
    ucps, urecs = [], []
    if unseen_layouts:
        import layouts
        ufams = layouts.families(layouts.unseen_layout_set(unseen_layouts))
        ucps = [harness.make_checkpoint(mk, seed, fam, length, n, budget_s) for fam in ufams for seed in range(seeds) for n in prefixes]
        with open(os.path.join(out, "checkpoints.jsonl"), "a") as f:
            for c in ucps:
                f.write(json.dumps(c) + "\n")
        urecs = harness.run_branches(ucps, mk, repeats=repeats, seed=2, restore_mode=restore)
        with open(os.path.join(out, "branches.jsonl"), "a") as f:
            for r in urecs:
                f.write(json.dumps(r) + "\n")
        split["unseen_layout_test"] = sorted(ufams)
        split["unseen_rule"] = "unseen layouts are evaluation-only: not in the train families, not used for the predictor"
        json.dump(split, open(os.path.join(out, "split.json"), "w"), indent=1)
    preds = fit_predictor(cps + ucps, recs, split["train_families"])  # fitted on train-family branches only
    with open(os.path.join(out, "preds.jsonl"), "w") as f:
        for p in preds:
            f.write(json.dumps(p) + "\n")
    pmap = {(p["checkpoint"], p["treatment"]): p["delta"] for p in preds}
    test_recs = [r for r in recs if r["family"] in split["test_families"]]
    res = {"all": analyze.analyze(recs), "test_families_only": analyze.analyze(test_recs, pmap),
           "task_level": analyze.analyze(recs, level="task"), "split": split, "backend": "gate" if gate else "sim", "restore": restore}
    plots = {"test": analyze.plot_reliability(res["test_families_only"]["incremental_reliability"], os.path.join(out, "reliability.png"),
                                              "Incremental reliability, TEST families (dry run: scripted controller, NOT model data)")}
    if urecs:
        res["unseen_layouts"] = analyze.analyze(urecs, pmap)
        plots["unseen"] = analyze.plot_reliability(res["unseen_layouts"]["incremental_reliability"], os.path.join(out, "reliability_unseen.png"),
                                                   "Incremental reliability, UNSEEN layouts (dry run, NOT model data)")
    res["plots"] = plots
    json.dump(res, open(os.path.join(out, "analysis.json"), "w"), indent=1)
    md = "# Counterfactual dry run (fixture twin, scripted controller - NOT model data)\n\n" + analyze.to_md(res["all"]) + "\n" + analyze.to_md(res["task_level"])
    md += "\n## Test families only, with the train-family predictor\n\n" + analyze.to_md(res["test_families_only"])
    if urecs:
        md += "\n## Unseen layouts (evaluation only)\n\n" + analyze.to_md(res["unseen_layouts"])
    open(os.path.join(out, "analysis.md"), "w").write(md)
    return cps + ucps, recs + urecs, res


def main(a):
    flags = {a[i]: a[i + 1] for i in range(len(a) - 1) if a[i].startswith("--")}
    _, recs, res = run(flags.get("--out", "cf-dryrun"), gate=flags.get("--gate"), seeds=int(flags.get("--seeds", 3)),
                       prefixes=tuple(int(x) for x in flags.get("--prefixes", "2,5").split(",")), repeats=int(flags.get("--repeats", 2)),
                       restore=flags.get("--restore", "snapshot"), families=flags["--families"].split(",") if "--families" in flags else None,
                       unseen_layouts=int(flags.get("--unseen-layouts", 0)))
    print(analyze.to_md(res["all"]))


if __name__ == "__main__":
    main(sys.argv[1:])
