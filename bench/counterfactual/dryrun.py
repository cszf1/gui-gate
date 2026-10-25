#!/usr/bin/env python3
"""Offline end-to-end dry run of the P1 protocol on the fixture twin (no desktop, no model, no network).

usage: python3 bench/counterfactual/dryrun.py --out DIR [--gate target/release/gui-gate] [--seeds 3] [--prefixes 2,5]
                                              [--repeats 2] [--restore snapshot|replay] [--families relabel,loading,popup,mixed]

Builds checkpoints (family x seed x prefix length) with the fixed policy pi, branches each under all seven
treatments (harness.TREATMENTS) in a shuffled order with repeats, verifies every restore by state hash, splits the
checkpoints by TASK FAMILY (train / test families never share a checkpoint), and writes:
  DIR/checkpoints.jsonl  DIR/branches.jsonl  DIR/split.json  DIR/analysis.json  DIR/analysis.md
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


def run(out, gate=None, seeds=3, prefixes=(2, 5), repeats=2, restore="snapshot", families=None, length=8, budget_s=160.0):
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
    res = {"all": analyze.analyze(recs), "test_families_only": analyze.analyze([r for r in recs if r["family"] in split["test_families"]]),
           "task_level": analyze.analyze(recs, level="task"), "split": split, "backend": "gate" if gate else "sim", "restore": restore}
    json.dump(res, open(os.path.join(out, "analysis.json"), "w"), indent=1)
    md = "# Counterfactual dry run (fixture twin, scripted controller - NOT model data)\n\n" + analyze.to_md(res["all"]) + "\n" + analyze.to_md(res["task_level"])
    open(os.path.join(out, "analysis.md"), "w").write(md)
    return cps, recs, res


def main(a):
    flags = {a[i]: a[i + 1] for i in range(len(a) - 1) if a[i].startswith("--")}
    _, recs, res = run(flags.get("--out", "cf-dryrun"), gate=flags.get("--gate"), seeds=int(flags.get("--seeds", 3)),
                       prefixes=tuple(int(x) for x in flags.get("--prefixes", "2,5").split(",")), repeats=int(flags.get("--repeats", 2)),
                       restore=flags.get("--restore", "snapshot"), families=flags["--families"].split(",") if "--families" in flags else None)
    print(analyze.to_md(res["all"]))


if __name__ == "__main__":
    main(sys.argv[1:])
