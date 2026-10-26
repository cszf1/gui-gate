#!/usr/bin/env python3
"""Online experiment plan generator for the value model v2 (stdlib only). It WRITES A PLAN FILE and never starts anything.

    python3 bench/online_plan.py TASK_LIST -o plan.json [--arms legacy,v2] [--holdout 0.3] [--seed 0] [--repeats 1]
                                 [--name NAME] [--weights PATH --weights-sha256 HEX] [--gate-commit SHA] [--margin 0.03]

TASK_LIST: one task per line (an OSWorld task json path like ovf/<task>.json, or a bare task id), e.g.
results/<run>/setup/task_list_ovf.txt.

What the plan freezes:
* the task split: a seeded, stratified-by-domain held-out set (the only tasks the comparison may run) and the
  development set (tasks whose logs may be used to fit or tune the model). Fitting on held-out tasks is forbidden;
* the arms: identical gate configuration except GUI_GATE_VALUE_MODEL (legacy = the default path, v2 = the opt-in
  model). Both arms load the same weights file, which the default path ignores;
* the run order: every (held-out task, arm, repeat) cell exactly once, in a seeded random order, blocked by task
  so both arms of a task run close in time (drift in the box or the model API hits both arms alike);
* the analysis: primary = task success on held-out tasks, paired by task (bench/stats.py mcnemar_exact,
  paired_bootstrap, noninferiority with the pre-registered margin); infra failures are retried with the same rule in
  both arms and reported, never silently dropped; secondary = interventions per GUI action, wall seconds, tokens.

The plan carries its own sha256 (over the canonical JSON without the hash) so a later run can prove it used this
plan unchanged. Nothing here launches a box, a gate, a model or a benchmark.
"""
import argparse
import hashlib
import json
import os
import random
import sys

BASE_ENV = {
    "GUI_GATE_POLICY": "gate",
    "GUI_GATE_VALUE_POLICY": "learned",
    "GUI_GATE_HUMAN": "deny",
    "GUI_GATE_DIALOG": "none",
    "GUI_GATE_NO_UPDATE_CHECK": "1",
}
ARM_MODELS = {"legacy": "legacy", "v2": "v2", "treatment": "treatment"}


def read_tasks(path):
    out = []
    with open(path, encoding="utf-8") as fh:
        lines = fh.readlines()
    for line in lines:
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        t = os.path.basename(line)
        out.append(t[:-5] if t.endswith(".json") else t)
    if len(set(out)) != len(out):
        raise ValueError("duplicate task in the task list")
    return out


def domain(task):
    return task.split("-")[0]


def split_holdout(tasks, frac, seed):
    """Stratified by domain: round(frac * n_domain) held-out tasks per domain (at least one per domain with >= 2 tasks),
    then corrected to round(frac * n) overall. Seeded and order-independent (tasks are sorted first)."""
    rng = random.Random(f"holdout:{seed}")
    by = {}
    for t in sorted(tasks):
        by.setdefault(domain(t), []).append(t)
    held = []
    for d in sorted(by):
        ts = by[d][:]
        rng.shuffle(ts)
        k = int(round(frac * len(ts)))
        if len(ts) >= 2:
            k = max(1, min(k, len(ts) - 1))
        held += ts[:k]
    want = int(round(frac * len(tasks)))
    rest = sorted(set(tasks) - set(held))
    rng.shuffle(rest)
    while len(held) < want and rest:
        held.append(rest.pop())
    held_sorted = sorted(held)
    rng.shuffle(held_sorted)
    while len(held_sorted) > want:
        held_sorted.pop()
    return sorted(held_sorted), sorted(set(tasks) - set(held_sorted))


def plan_hash(plan):
    body = {k: v for k, v in plan.items() if k != "plan_sha256"}
    return hashlib.sha256(json.dumps(body, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def make_plan(task_list, arms=("legacy", "v2"), holdout_frac=0.3, seed=0, repeats=1, name="value-v2-online",
              weights=None, weights_sha256=None, gate_commit=None, margin=0.03):
    tasks = read_tasks(task_list)
    for a in arms:
        if a not in ARM_MODELS:
            raise ValueError(f"unknown arm {a}")
    held, dev = split_holdout(tasks, holdout_frac, seed)
    base = dict(BASE_ENV)
    if weights:
        base["GUI_GATE_VALUE_WEIGHTS"] = weights
    arm_specs = [{"name": a, "env": dict(base, GUI_GATE_VALUE_MODEL=ARM_MODELS[a])} for a in arms]
    rng = random.Random(f"order:{seed}")
    blocks = []
    for t in held:
        for r in range(repeats):
            order = list(arms)
            rng.shuffle(order)  # which arm runs first inside the block is randomized too
            blocks.append([(t, a, r) for a in order])
    rng.shuffle(blocks)
    schedule = []
    for b in blocks:
        for (t, a, r) in b:
            schedule.append({"slot": len(schedule), "task": t, "domain": domain(t), "arm": a, "repeat": r,
                             "tag": f"{t}-v{a}-s{r}"})
    plan = {
        "kind": "online_plan_v1", "name": name, "dry": True, "seed": seed, "holdout_frac": holdout_frac, "repeats": repeats,
        "task_list": os.path.basename(task_list), "n_tasks": len(tasks),
        "tasks": {"holdout": held, "development": dev,
                  "rule": "the model may be fitted / tuned on development tasks only; holdout tasks are run once per arm and repeat and never used for fitting"},
        "arms": arm_specs,
        "frozen": {"gate_commit": gate_commit, "weights": weights, "weights_sha256": weights_sha256,
                   "differs_between_arms": ["GUI_GATE_VALUE_MODEL"]},
        "schedule": schedule,
        "analysis": {
            "unit": "held-out task (paired across arms; repeats averaged per task before pairing)",
            "primary": "task success (OSWorld evaluator score >= 1)",
            "tests": ["bench/stats.py mcnemar_exact (paired binary)", "bench/stats.py paired_bootstrap (mean difference, 95% CI)",
                      f"bench/stats.py noninferiority with the pre-registered margin {margin}"],
            "noninferiority_margin": margin,
            "secondary": ["GUI interventions per GUI action", "wall seconds per task", "model tokens per task", "person requests"],
            "infra": "setup / relay / evaluator failures are retried with the same rule in both arms (first valid attempt); a cell still infra after retries is reported and excluded from BOTH arms of that task",
            "claims": "only this randomized comparison (or paired counterfactual branches) can support 'v2 avoided failures'; offline logs cannot",
        },
        "note": "plan only: this generator never starts a box, a gate, a model or a benchmark",
    }
    plan["plan_sha256"] = plan_hash(plan)
    return plan


def main(argv=None):
    ap = argparse.ArgumentParser(description="write a frozen online experiment plan (never runs anything)")
    ap.add_argument("task_list")
    ap.add_argument("-o", "--out", required=True)
    ap.add_argument("--arms", default="legacy,v2")
    ap.add_argument("--holdout", type=float, default=0.3)
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--repeats", type=int, default=1)
    ap.add_argument("--name", default="value-v2-online")
    ap.add_argument("--weights", default=None)
    ap.add_argument("--weights-sha256", default=None)
    ap.add_argument("--gate-commit", default=None)
    ap.add_argument("--margin", type=float, default=0.03)
    a = ap.parse_args(argv)
    plan = make_plan(a.task_list, [x for x in a.arms.split(",") if x], a.holdout, a.seed, a.repeats, a.name,
                     a.weights, a.weights_sha256, a.gate_commit, a.margin)
    with open(a.out, "w") as fh:
        json.dump(plan, fh, indent=1)
    print(json.dumps({"out": a.out, "holdout": len(plan["tasks"]["holdout"]), "cells": len(plan["schedule"]), "plan_sha256": plan["plan_sha256"]}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
