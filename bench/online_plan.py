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

v0.10.3 (review 2026-10-26, section 6, "minimal online plan"):

    python3 bench/online_plan.py pilots TASK_LIST -o OUT_DIR --freeze freeze.json [--checkpoints ckpts.txt] [--seed 20261026]

writes two plans, both only "frozen" when EVERY item of FREEZE_KEYS has a hash in freeze.json (Pi, main model and its
thinking setting, reviewer model, roles, prompts, driver, app environment, timeouts, cost configuration, gate commit,
value weights); with anything missing the plan is written with status "draft" and the missing keys, and is never
called frozen (--require-frozen makes that an error):
* policy pilot: Chrome, Impress, Calc and multi_apps, 3 tasks each drawn with seed 20261026; conditions
  direct-with-safety (safety and hard rules kept, value step always direct), legacy (the default), frozen-new (the
  frozen v2 model); seeds 0/1/2 -> 12 x 3 x 3 = 108 task runs, blocked by task in a seeded order;
* increment pilot: 24 fully recoverable, safe checkpoints; branches direct / look_full / matched wait (waits as long as
  look_full takes, without new information), 3 repeats each -> 216 branch continuations with a fixed continuation
  policy, local post-condition and continuation budget; analysed clustered by checkpoint.
Analysis (both): repeats are first aggregated within a task (checkpoint), then paired bootstrap / sign-flip
permutation over tasks (checkpoints); McNemar is NOT applied to repeat averages (it needs one binary outcome per
pair). Infra (pre-registered): the primary analysis keeps EVERY planned unit (a unit still infra counts as a failure);
external infra may be re-run at most once with the same rule in every condition; the valid-pairs analysis is reported
separately; an algorithm hang (gate, Pi, model loop, check_done) is never infra.
"""
import argparse
import hashlib
import json
import os
import random
import re
import sys

# everything that must be pinned before a plan may be called frozen (a sha256 hex, or a 7-40 hex git commit)
FREEZE_KEYS = ["pi", "main_model", "main_thinking", "reviewer_model", "roles", "prompts", "driver", "app_env",
               "timeouts", "cost_config", "gate_commit", "value_weights"]
HEX = re.compile(r"^[0-9a-f]{7,64}$")
PILOT_SEED = 20261026
POLICY_DOMAINS = {"Chrome": "chrome", "Impress": "libreoffice_impress", "Calc": "libreoffice_calc", "multi_apps": "multi_apps"}
POLICY_CONDITIONS = {
    "direct_with_safety": {"GUI_GATE_VALUE_POLICY": "direct", "GUI_GATE_VALUE_MODEL": "legacy"},
    "legacy": {"GUI_GATE_VALUE_POLICY": "learned", "GUI_GATE_VALUE_MODEL": "legacy"},
    "frozen_new": {"GUI_GATE_VALUE_POLICY": "learned", "GUI_GATE_VALUE_MODEL": "v2"},
}
INCREMENT_ARMS = ["direct", "look_full", "matched_wait"]

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


def check_freeze(freeze):
    """(complete, hashes, missing): every FREEZE_KEYS item needs a hash (sha256 hex or a git commit)."""
    freeze = freeze or {}
    hashes, missing = {}, []
    for k in FREEZE_KEYS:
        v = freeze.get(k)
        h = v.get("sha256") or v.get("commit") if isinstance(v, dict) else v
        if isinstance(h, str) and HEX.match(h.strip().lower()):
            hashes[k] = h.strip().lower()
        else:
            missing.append(k)
    return not missing, hashes, missing


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for b in iter(lambda: fh.read(1 << 16), b""):
            h.update(b)
    return h.hexdigest()


ANALYSIS_TASK = {
    "unit": "task (paired across conditions)",
    "aggregation": "repeats (seeds) are averaged WITHIN each task first; each task then contributes one paired difference",
    "tests": ["paired bootstrap over tasks of the mean difference (95% CI)", "sign-flip permutation test over tasks"],
    "not_used": "McNemar on repeat averages (it assumes one binary outcome per pair; only valid with exactly one run per task and condition)",
    "secondary": ["total wall seconds and its tail (p90)", "interventions per GUI action (budget)", "model tokens", "person requests"],
}
INFRA_RULE = {
    "primary": "keep every planned unit; a unit that is still infra after the allowed re-run counts as a failure in its condition",
    "rerun": "external infra only (box setup failure, relay/API outage, evaluator crash with a logged external cause): at most one re-run, same rule in every condition, decided before looking at outcomes",
    "valid_pairs": "reported separately: tasks with a valid run in every condition",
    "not_infra": "algorithm hangs (gate, Pi loop, model loop, check_done or role deadline) are failures of the condition, never infra",
    "never": "no retry until success",
}


def _schedule(units, conds, repeats, rng, key):
    blocks = []
    for u in units:
        for r in repeats:
            order = list(conds)
            rng.shuffle(order)
            blocks.append([(u, c, r) for c in order])
    rng.shuffle(blocks)
    out = []
    for b in blocks:
        for (u, c, r) in b:
            out.append({"slot": len(out), key: u, "condition": c, "repeat": r})
    return out


def _stamp(plan, complete, hashes, missing, require_frozen):
    if require_frozen and not complete:
        raise ValueError("refusing a frozen plan: no hash for " + ", ".join(missing))
    plan["status"] = "frozen" if complete else "draft"
    plan["frozen"] = {"complete": complete, "hashes": hashes, "missing": missing, "required": FREEZE_KEYS}
    if not complete:
        plan["note_draft"] = "DRAFT: not frozen (missing: " + ", ".join(missing) + "); it must not be run as the pre-registered experiment"
    plan["plan_sha256"] = plan_hash(plan)
    return plan


def make_policy_pilot(task_list, freeze=None, seed=PILOT_SEED, per_domain=3, seeds=(0, 1, 2), require_frozen=False):
    tasks = read_tasks(task_list)
    rng = random.Random(f"policy:{seed}")
    picked = {}
    for label, dom in POLICY_DOMAINS.items():
        pool = sorted(t for t in tasks if domain(t) == dom)
        if len(pool) < per_domain:
            raise ValueError(f"domain {dom} has {len(pool)} tasks (< {per_domain})")
        picked[label] = sorted(rng.sample(pool, per_domain))
    units = [t for label in POLICY_DOMAINS for t in picked[label]]
    sched = _schedule(units, list(POLICY_CONDITIONS), list(seeds), random.Random(f"policy-order:{seed}"), "task")
    for c in sched:
        c["tag"] = f"{c['task']}-{c['condition']}-s{c['repeat']}"
    complete, hashes, missing = check_freeze(freeze)
    plan = {"kind": "policy_pilot_v1", "dry": True, "seed": seed, "task_list": os.path.basename(task_list),
            "selection": {"rule": f"{per_domain} tasks per domain, random.Random('policy:{seed}').sample over the sorted domain list", "tasks": picked},
            "conditions": {k: dict(v, GUI_GATE_POLICY="gate") for k, v in POLICY_CONDITIONS.items()},
            "conditions_note": "identical safety rules, hard rules, reviewer, person channel and budgets in every condition; only the value step differs",
            "seeds": list(seeds), "runs": len(sched), "schedule": sched,
            "analysis": dict(ANALYSIS_TASK, comparisons=["frozen_new vs legacy", "frozen_new vs direct_with_safety", "legacy vs direct_with_safety"],
                             power="12 tasks: for direction and variance only; cannot detect a few percentage points"),
            "infra": INFRA_RULE, "note": "plan only: this generator never starts a box, a gate, a model or a benchmark"}
    return _stamp(plan, complete, hashes, missing, require_frozen)


def make_increment_pilot(freeze=None, checkpoints=None, seed=PILOT_SEED, n=24, repeats=3, require_frozen=False):
    if checkpoints:
        pool = sorted(set(checkpoints))
        if len(pool) < n:
            raise ValueError(f"{len(pool)} checkpoints (< {n})")
        cps = sorted(random.Random(f"ckpt:{seed}").sample(pool, n))
        sel = "given list, random.Random('ckpt:%d').sample" % seed
    else:
        cps = [f"ckpt-{i + 1:02d}" for i in range(n)]
        sel = "slots: to be filled by the pre-registered rule before freezing"
    sched = _schedule(cps, INCREMENT_ARMS, list(range(repeats)), random.Random(f"inc-order:{seed}"), "checkpoint")
    complete, hashes, missing = check_freeze(freeze)
    plan = {"kind": "increment_pilot_v1", "dry": True, "seed": seed, "treatment": "look_full",
            "checkpoints": cps, "checkpoint_selection": {"source": sel,
                "rule": "safe (no risky effect pending), fully restorable (app state + files + window layout snapshot verified by hash), the gate's decision at the checkpoint is not a safety ask; one per task"},
            "arms": {"direct": "send the candidate action now",
                     "look_full": "return a tree + screenshot to the acting model before the step (the model may change the action)",
                     "matched_wait": "wait as long as look_full takes (pre-registered: median look_full extra time of the pilot), then send the candidate action; separates new information from the UI settling"},
            "repeats": repeats, "branches": len(sched), "schedule": sched,
            "continuation": {"policy": "the frozen agent (same Pi, model, prompts) continues from the branch", "budget": "fixed continuation budget (turns and seconds) per branch, the same for every arm",
                             "local_postcondition": "pre-registered per checkpoint (target control / file / app state)", "untested_options": "remain unknown (not estimated)"},
            "analysis": {"unit": "checkpoint (cluster)", "aggregation": "the 3 repeats of an arm are averaged within the checkpoint first",
                         "estimands": ["E[Y(look_full) - Y(direct)]", "E[Y(look_full) - Y(matched_wait)]", "same for the local post-condition and the remaining time"],
                         "tests": ["checkpoint-cluster bootstrap of the mean paired difference", "sign-flip permutation over checkpoints"],
                         "incremental_calibration": "bin checkpoints by the frozen model's predicted tau and compare with the observed paired difference (bench/offline_v2.py incremental_calibration; negative allowed)",
                         "not_used": "McNemar on repeat averages"},
            "infra": INFRA_RULE, "note": "plan only: never runs anything"}
    return _stamp(plan, complete, hashes, missing, require_frozen)


def make_plan(task_list, arms=("legacy", "v2"), holdout_frac=0.3, seed=0, repeats=1, name="value-v2-online",
              weights=None, weights_sha256=None, gate_commit=None, margin=0.03, freeze=None):
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
        "pinned": {"gate_commit": gate_commit, "weights": weights, "weights_sha256": weights_sha256,
                   "differs_between_arms": ["GUI_GATE_VALUE_MODEL"]},
        "schedule": schedule,
        "analysis": {
            "unit": "held-out task (paired across arms; repeats averaged per task before pairing)",
            "primary": "task success (OSWorld evaluator score >= 1)",
            "tests": ["bench/stats.py paired_bootstrap over tasks (mean difference of within-task averages, 95% CI)", "sign-flip permutation over tasks",
                      f"bench/stats.py noninferiority with the pre-registered margin {margin}",
                      "bench/stats.py mcnemar_exact ONLY when repeats == 1 (one binary outcome per task and arm)"],
            "noninferiority_margin": margin,
            "secondary": ["GUI interventions per GUI action", "wall seconds per task", "model tokens per task", "person requests"],
            "infra": INFRA_RULE,
            "claims": "only this randomized comparison (or paired counterfactual branches) can support 'v2 avoided failures'; offline logs cannot",
        },
        "note": "plan only: this generator never starts a box, a gate, a model or a benchmark",
    }
    if gate_commit and "gate_commit" not in (freeze or {}):
        freeze = dict(freeze or {}, gate_commit=gate_commit)
    if weights_sha256 and "value_weights" not in (freeze or {}):
        freeze = dict(freeze or {}, value_weights=weights_sha256)
    complete, hashes, missing = check_freeze(freeze)
    return _stamp(plan, complete, hashes, missing, False)


def pilots_main(argv):
    ap = argparse.ArgumentParser(description="write the policy and increment pilot plans (never runs anything)")
    ap.add_argument("task_list")
    ap.add_argument("-o", "--out", required=True, help="output directory")
    ap.add_argument("--freeze", default=None, help="json: one hash per FREEZE_KEYS item")
    ap.add_argument("--checkpoints", default=None, help="file with one recoverable checkpoint id per line")
    ap.add_argument("--seed", type=int, default=PILOT_SEED)
    ap.add_argument("--require-frozen", action="store_true", help="error instead of a draft when a hash is missing")
    a = ap.parse_args(argv)
    freeze = None
    if a.freeze:
        with open(a.freeze, encoding="utf-8") as fh:
            freeze = json.load(fh)
    cps = None
    if a.checkpoints:
        with open(a.checkpoints, encoding="utf-8") as fh:
            cps = [x.strip() for x in fh if x.strip() and not x.startswith("#")]
    try:
        pol = make_policy_pilot(a.task_list, freeze, a.seed, require_frozen=a.require_frozen)
        inc = make_increment_pilot(freeze, cps, a.seed, require_frozen=a.require_frozen)
    except ValueError as e:
        print(json.dumps({"error": str(e)}))
        return 2
    os.makedirs(a.out, exist_ok=True)
    for name, p in (("policy_pilot.json", pol), ("increment_pilot.json", inc)):
        with open(os.path.join(a.out, name), "w") as fh:
            json.dump(p, fh, indent=1)
    print(json.dumps({"out": a.out, "policy_runs": pol["runs"], "increment_branches": inc["branches"], "status": pol["status"],
                      "missing": pol["frozen"]["missing"]}))
    return 0


def main(argv=None):
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv and argv[0] == "pilots":
        return pilots_main(argv[1:])
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
