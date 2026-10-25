#!/usr/bin/env python3
"""Frozen task manifest: task list, dev/test split BY TASK FAMILY, exclusion reasons, retry rules, content hash.

A research claim is only as good as the list it was measured on. The manifest is written once, BEFORE any test run,
committed, and its hash is quoted in the pre-registration (docs/eval-protocol.md). Runners refuse to run the test
split unless they are given the exact hash, so a changed list, a dropped task, an edited task JSON or an edited
exclusion reason cannot slip in silently.

  manifest.py create  --list TASKLIST --out M.json [--name N] [--test-frac 0.5] [--seed 0] [--all-dev]
                      [--test-families a,b] [--exclude task=reason ...] [--margin 0.05] [--infra-retries 2]
  manifest.py validate M.json                      -> prints the hash, exit 1 when invalid
  manifest.py hash M.json                          -> prints the hash only
  manifest.py list M.json --split dev|test [--out LIST]   -> the split's runnable tasks as a run_bench task list
  manifest.py guard --list TASKLIST [--manifest M.json] [--hash H]   (run_bench.sh calls this before anything runs)
  manifest.py denominators M.json RUN_DIR [RUN_DIR...] --split test [--json OUT]

Rules the code enforces:
  * the split is by task family (OSWorld app domain, e.g. every libreoffice_calc task together); no family on both
    sides. A held-out run is not an unseen task.
  * every excluded task carries a reason; excluded tasks stay in the manifest and in the planned-vs-valid report.
  * the hash covers the task list, every task JSON's content (sha256), families, split, exclusions, retry rules and
    the pre-registered non-inferiority margin; `created` and free notes are not hashed.
  * guard: a list that touches a test task must (1) come with --hash equal to the manifest's recomputed hash, (2) be
    exactly the manifest's runnable test split (no cherry-picked subset, no extra tasks), (3) not mix dev and test.
  * denominators: planned = runnable test tasks x repeats x policies; missing runs and every infra failure count as
    failures in the planned denominator and are counted per policy and per kind, never dropped.
"""
import argparse
import hashlib
import json
import os
import random
import re
import sys
import time
from collections import Counter, defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
TASKS = os.path.join(HERE, "tasks")
DEFAULT_MANIFEST = os.path.join(TASKS, "manifest.json")
SCHEMA = "gui-gate-bench-manifest/1"
# infra = the run did not measure the agent (setup/eval harness failure, relay/API error, missing run). Retried only
# under these rules; the last attempt counts. Agent outcomes (wrong answer, FAIL reply, max_turns, timeout) are never
# retried and never infra.
DEFAULT_RETRY = {"retry_on": ["relay_error", "setup_failed"], "max_attempts": 3,
                 "never_retry": ["agent failure", "FAIL reply", "max_turns", "timeout", "eval mismatch"],
                 "last_attempt_counts": True, "infra_counts_as_failure_in_planned": True}
OW_NAME = re.compile(r"^(?:ow|og)\d+-([a-z_]+?)(?:-[0-9a-f]{8}|-pre|-)")
UUID_NAME = re.compile(r"^([a-z_]+)-[0-9a-f]{8}-[0-9a-f]{4}-")


def task_name(entry):
    b = os.path.basename(entry.strip())
    return b[:-5] if b.endswith(".json") else b


def family_of(entry, task_json=None):
    """OSWorld app domain = task family. ow01-libreoffice_calc-7efeb4b1 -> libreoffice_calc; og02-writer-pdf-exists ->
    osguard-writer; chrome-<uuid> (OSWorld-Verified lists) -> chrome; else the task JSON's snapshot."""
    n = task_name(entry)
    if n.startswith("og"):
        m = re.match(r"^og\d+-([a-z_]+)", n)
        return "osguard-" + (m.group(1) if m else "misc")
    m = OW_NAME.match(n)
    if m:
        return m.group(1)
    m = UUID_NAME.match(n)
    if m:
        return m.group(1)
    if task_json and task_json.get("snapshot"):
        return task_json["snapshot"]
    return n.split("-")[0]


def read_list(path):
    return [x.strip() for x in open(path) if x.strip() and not x.strip().startswith("#")]


def file_sha(entry, base=TASKS):
    p = entry if os.path.isabs(entry) else os.path.join(base, entry)
    try:
        return hashlib.sha256(open(p, "rb").read()).hexdigest()
    except OSError:
        return None


def _load_task(entry, base=TASKS):
    p = entry if os.path.isabs(entry) else os.path.join(base, entry)
    try:
        return json.load(open(p))
    except Exception:
        return None


def canonical(m):
    core = {k: v for k, v in m.items() if k not in ("hash", "created", "notes")}
    return json.dumps(core, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def compute_hash(m):
    return hashlib.sha256(canonical(m).encode()).hexdigest()


def build(entries, name="bench", test_frac=0.5, seed=0, exclusions=None, retry=None, margin=None, all_dev=False,
          test_families=None, base=TASKS, notes=""):
    exclusions = exclusions or {}
    fams = {}
    rows = []
    for e in entries:
        tj = _load_task(e, base)
        f = family_of(e, tj)
        fams.setdefault(f, []).append(e)
        rows.append({"task": task_name(e), "entry": e, "family": f, "sha256": file_sha(e, base)})
    names = sorted(fams)
    if all_dev:
        test = set()
    elif test_families:
        unknown = set(test_families) - set(names)
        if unknown:
            raise ValueError(f"unknown test families: {sorted(unknown)}")
        test = set(test_families)
    else:
        order = names[:]
        random.Random(seed).shuffle(order)
        k = int(round(len(order) * test_frac)) if len(order) > 1 else 0
        test = set(order[:k])
    for r in rows:
        r["split"] = "test" if r["family"] in test else "dev"
        if r["task"] in exclusions:
            r["excluded"] = True
            r["reason"] = exclusions[r["task"]]
        else:
            r["excluded"] = False
    unknown_ex = set(exclusions) - {r["task"] for r in rows}
    if unknown_ex:
        raise ValueError(f"exclusions name tasks that are not in the list: {sorted(unknown_ex)}")
    m = {"schema": SCHEMA, "name": name, "split_rule": "by task family (OSWorld app domain); no family on both sides",
         "split_seed": None if (all_dev or test_families) else seed, "test_frac": None if (all_dev or test_families) else test_frac,
         "families": {f: ("test" if f in test else "dev") for f in names}, "tasks": rows,
         "retry_rules": retry or DEFAULT_RETRY, "noninferiority_margin": margin,
         "tuning_rule": "dev split only; nothing (rules, prompts, memory, calibrators, thresholds) may be changed after "
                        "looking at test-split results", "created": time.strftime("%Y-%m-%d %H:%M:%S %z"), "notes": notes}
    m["hash"] = compute_hash(m)
    return m


def validate(m, base=TASKS, check_files=True):
    """-> list of problems (empty = valid)."""
    errs = []
    if m.get("schema") != SCHEMA:
        errs.append(f"schema {m.get('schema')!r} != {SCHEMA}")
    if m.get("hash") != compute_hash(m):
        errs.append("hash mismatch: the manifest was edited after it was frozen")
    side = defaultdict(set)
    seen = Counter()
    for r in m.get("tasks", []):
        side[r["family"]].add(r["split"])
        seen[r["task"]] += 1
        if r["split"] not in ("dev", "test"):
            errs.append(f"{r['task']}: split {r['split']!r}")
        if r.get("excluded") and not (r.get("reason") or "").strip():
            errs.append(f"{r['task']}: excluded without a reason")
        if m.get("families", {}).get(r["family"]) != r["split"]:
            errs.append(f"{r['task']}: family {r['family']} is {m.get('families', {}).get(r['family'])} but the task is {r['split']}")
        if check_files and r.get("sha256"):
            now = file_sha(r["entry"], base)
            if now is not None and now != r["sha256"]:
                errs.append(f"{r['task']}: task JSON changed since the manifest was frozen")
    for f, s in side.items():
        if len(s) > 1:
            errs.append(f"family {f} is on both sides of the split")
    for t, c in seen.items():
        if c > 1:
            errs.append(f"{t}: listed {c} times")
    return errs


def runnable(m, split):
    return [r for r in m["tasks"] if r["split"] == split and not r.get("excluded")]


def guard(list_entries, m=None, given_hash=None, base=TASKS):
    """-> (ok, split, message). split is 'test' when the list touches the test split, else 'dev'."""
    if m is None:
        return True, "dev", "no manifest: nothing is marked test"
    by = {r["task"]: r for r in m["tasks"]}
    names = [task_name(e) for e in list_entries]
    test = [n for n in names if n in by and by[n]["split"] == "test"]
    if not test:
        return True, "dev", f"dev run ({len(names)} tasks; manifest {m.get('name')} {m.get('hash', '')[:12]})"
    errs = validate(m, base)
    if errs:
        return False, "test", "manifest invalid: " + "; ".join(errs[:5])
    if not given_hash:
        return False, "test", "the list touches the frozen test split: pass --manifest-hash (the pre-registered hash)"
    if given_hash != m["hash"]:
        return False, "test", f"manifest hash mismatch: given {given_hash[:12]}, manifest {m['hash'][:12]}"
    other = [n for n in names if n not in test]
    if other:
        return False, "test", f"test runs must not mix in non-test tasks: {other[:5]}"
    want = sorted(r["task"] for r in runnable(m, "test"))
    if sorted(names) != want:
        missing = sorted(set(want) - set(names))
        extra = sorted(set(names) - set(want))
        return False, "test", f"the list is not the frozen test split (missing {missing[:5]}, excluded/extra {extra[:5]})"
    return True, "test", f"test run on the frozen split ({len(want)} tasks, hash {m['hash'][:12]})"


def task_allowed(entry, m=None, given_hash=None, base=TASKS):
    """Per-task form of guard for runners that take one task at a time (counterfactual/osworld.py): a test-split task
    needs a valid manifest and the matching hash. -> (ok, split, message)."""
    if m is None:
        return True, "dev", "no manifest"
    r = {x["task"]: x for x in m["tasks"]}.get(task_name(entry))
    if r is None or r["split"] != "test":
        return True, "dev", "not a test task"
    if r.get("excluded"):
        return False, "test", f"{r['task']} is excluded: {r.get('reason')}"
    errs = validate(m, base)
    if errs:
        return False, "test", "manifest invalid: " + "; ".join(errs[:3])
    if given_hash != m["hash"]:
        return False, "test", "test-split task without the matching manifest hash"
    return True, "test", "ok"


def default_manifest():
    return load(DEFAULT_MANIFEST) if os.path.exists(DEFAULT_MANIFEST) else None


def denominators(m, rows, split="test", policies=None):
    """rows: runs_scored.jsonl dicts (task, cond, seed, attempt, success, infra). The last attempt of each
    (task, cond, seed) counts. -> {policy: {...}} with planned and valid denominators and infra by kind."""
    plan = [r["task"] for r in runnable(m, split)]
    excluded = [r for r in m["tasks"] if r["split"] == split and r.get("excluded")]
    best = {}
    for r in rows:
        k = (r["task"], r["cond"], r.get("seed", 0))
        if k not in best or (r.get("attempt") or 1) >= (best[k].get("attempt") or 1):
            best[k] = r
    pols = policies or sorted({k[1] for k in best})
    seeds = sorted({k[2] for k in best}) or [0]
    out = {"split": split, "planned_tasks": len(plan), "repeats": seeds, "excluded": {r["task"]: r["reason"] for r in excluded},
           "manifest_hash": m.get("hash"), "policies": {}}
    for p in pols:
        kinds = Counter()
        k_ok = valid = valid_ok = 0
        attempts = 0
        for t in plan:
            for s in seeds:
                r = best.get((t, p, s))
                if r is None:
                    kinds["missing"] += 1
                    continue
                attempts += r.get("attempt") or 1
                if r.get("infra"):
                    kinds[str(r["infra"])] += 1
                    continue
                valid += 1
                if r.get("success"):
                    valid_ok += 1
                    k_ok += 1
        n = len(plan) * len(seeds)
        out["policies"][p] = {"planned_runs": n, "planned_success": k_ok, "planned_rate": k_ok / n if n else None,
                              "valid_runs": valid, "valid_success": valid_ok, "valid_rate": valid_ok / valid if valid else None,
                              "infra_or_missing": n - valid, "infra_by_kind": dict(kinds), "attempts": attempts,
                              "outside_plan": sorted({k[0] for k in best if k[1] == p and k[0] not in plan})}
    return out


def load(path):
    return json.load(open(path))


def main(argv):
    ap = argparse.ArgumentParser(prog="manifest.py")
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("create")
    c.add_argument("--list", required=True)
    c.add_argument("--out", required=True)
    c.add_argument("--name", default="bench")
    c.add_argument("--test-frac", type=float, default=0.5)
    c.add_argument("--seed", type=int, default=0)
    c.add_argument("--all-dev", action="store_true")
    c.add_argument("--test-families")
    c.add_argument("--exclude", action="append", default=[], help="task=reason")
    c.add_argument("--margin", type=float)
    c.add_argument("--infra-retries", type=int)
    c.add_argument("--notes", default="")
    v = sub.add_parser("validate")
    v.add_argument("manifest")
    h = sub.add_parser("hash")
    h.add_argument("manifest")
    li = sub.add_parser("list")
    li.add_argument("manifest")
    li.add_argument("--split", required=True, choices=["dev", "test"])
    li.add_argument("--out")
    g = sub.add_parser("guard")
    g.add_argument("--list", required=True)
    g.add_argument("--manifest")
    g.add_argument("--hash")
    d = sub.add_parser("denominators")
    d.add_argument("manifest")
    d.add_argument("runs", nargs="+")
    d.add_argument("--split", default="test")
    d.add_argument("--json")
    a = ap.parse_args(argv)
    if a.cmd == "create":
        ex = {}
        for x in a.exclude:
            t, _, why = x.partition("=")
            ex[task_name(t)] = why
        retry = dict(DEFAULT_RETRY)
        if a.infra_retries is not None:
            retry["max_attempts"] = 1 + a.infra_retries
        m = build(read_list(a.list), a.name, a.test_frac, a.seed, ex, retry, a.margin, a.all_dev,
                  a.test_families.split(",") if a.test_families else None, notes=a.notes)
        json.dump(m, open(a.out, "w"), indent=1, ensure_ascii=False)
        print(m["hash"])
        return 0
    if a.cmd in ("validate", "hash"):
        m = load(a.manifest)
        errs = validate(m)
        if a.cmd == "hash":
            print(m.get("hash"))
            return 0 if not errs else 1
        for e in errs:
            print("invalid:", e, file=sys.stderr)
        print(m.get("hash"))
        return 1 if errs else 0
    if a.cmd == "list":
        m = load(a.manifest)
        lines = [r["entry"] for r in runnable(m, a.split)]
        txt = "\n".join(lines) + ("\n" if lines else "")
        if a.out:
            open(a.out, "w").write(txt)
        else:
            sys.stdout.write(txt)
        return 0
    if a.cmd == "guard":
        mp = a.manifest or (DEFAULT_MANIFEST if os.path.exists(DEFAULT_MANIFEST) else None)
        m = load(mp) if mp else None
        ok, split, msg = guard(read_list(a.list), m, a.hash)
        print(json.dumps({"ok": ok, "split": split, "manifest": mp, "hash": (m or {}).get("hash"), "msg": msg}))
        return 0 if ok else 3
    if a.cmd == "denominators":
        m = load(a.manifest)
        rows = []
        for dd in a.runs:
            rows += [json.loads(x) for x in open(os.path.join(dd, "runs_scored.jsonl")) if x.strip()]
        res = denominators(m, rows, a.split)
        if a.json:
            json.dump(res, open(a.json, "w"), indent=1)
        print(json.dumps(res, indent=1))
        return 0
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
