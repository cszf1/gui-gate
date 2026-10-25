#!/usr/bin/env python3
"""Leakage lint: the agent must not be able to read test answers, and nothing tuned may contain them.

  leakage_lint.py --tasks TASKLIST|--manifest M.json [--split test] --agent-dir DIR [--agent-dir DIR...]
                  [--answer-dir DIR...] [--artifact PATH...] [--unseen-layouts LAYOUTS.json] [--json OUT]

Two checks, both deterministic and offline:

1. Readability (run directory). Evaluator answer files of the test tasks - local paths named in each task JSON's
   `evaluator.expected` / `evaluator.result` (type vm_file / local_file / cloud_file cache), any --answer-dir (e.g.
   OSWorld's cache_dir with downloaded gold files, the bench's eval/ output dir) and bench/tasks itself (the task JSONs
   contain the expected values) - must not lie inside, or be reachable through a symlink from, any directory the agent
   runs in or is pointed at (--agent-dir: Pi's cwd, the gate's run_command cwd, the task-text dir). A file the agent's
   cwd can reach by a relative path is a leak even when nobody "meant" it.
   NOTE: an agent with an unrestricted same-user shell can read any file the user can read; this lint proves only
   that nothing is put in its way. Real isolation needs OS permissions (docs/security-model.md).

2. Content (memory, calibrators, rules). Every --artifact (weights/*.json, the gate's memory dir, skills / app hints,
   value-model files, calibrators, thresholds files) is scanned for test-task ids, test-task gold literals (string
   values of `expected` rules: e.g. "rules": {"expected": ["Bold"]}) and unseen-layout captions / ids. A hit means
   something tuned has seen a test answer.

Exit 0 = clean, 1 = leaks found (each listed), 2 = usage error.
"""
import argparse
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import manifest as mf  # noqa: E402

GOLD_MIN_LEN = 4  # shorter literals ("1", "Yes") match everywhere and say nothing
GENERIC = {"true", "false", "none", "null", "yes", "no", "file", "path", "type", "rules", "expected", "result", "match"}


def _walk_values(x, keys=()):
    if isinstance(x, dict):
        for k, v in x.items():
            yield from _walk_values(v, keys + (str(k),))
    elif isinstance(x, list):
        for v in x:
            yield from _walk_values(v, keys)
    else:
        yield keys, x


def answer_info(task_json):
    """-> (local answer paths, gold literals) of one task JSON's evaluator."""
    paths, lits = set(), set()
    ev = (task_json or {}).get("evaluator") or {}
    exps = ev.get("expected")
    exps = exps if isinstance(exps, list) else [exps]
    for e in exps:
        if not isinstance(e, dict):
            continue
        for keys, v in _walk_values(e):
            if not isinstance(v, str):
                continue
            if keys and keys[-1] in ("path", "dest", "file") and not v.startswith(("http://", "https://")):
                paths.add(v)
            elif "rules" in keys or (keys and keys[-1] in ("expected", "expect", "answer", "gold")):
                s = v.strip()
                if len(s) >= GOLD_MIN_LEN and s.lower() not in GENERIC:
                    lits.add(s)
    return paths, lits


def _real(p):
    return os.path.realpath(os.path.expanduser(p))


def _inside(path, d):
    p, d = _real(path), _real(d)
    return p == d or p.startswith(d.rstrip(os.sep) + os.sep)


def reachable_from(agent_dir, targets, max_entries=20000):
    """targets reachable from agent_dir: directly inside it, or through a symlink found under it."""
    hits = []
    for t in targets:
        if _inside(t, agent_dir):
            hits.append((t, "inside"))
    n = 0
    for root, dirs, files in os.walk(agent_dir, followlinks=False):
        for name in dirs + files:
            n += 1
            if n > max_entries:
                return hits
            fp = os.path.join(root, name)
            if os.path.islink(fp):
                for t in targets:
                    if _inside(t, fp) or _inside(_real(fp), t):
                        hits.append((t, f"symlink {fp} -> {os.readlink(fp)}"))
    return hits


def scan_artifacts(paths, needles, max_bytes=50_000_000):
    """needles: {kind: set(strings)}. -> [(artifact, kind, needle)]"""
    hits = []
    files = []
    for p in paths:
        if os.path.isdir(p):
            for root, _, fs in os.walk(p):
                files += [os.path.join(root, f) for f in fs]
        elif os.path.exists(p):
            files.append(p)
    for f in files:
        try:
            if os.path.getsize(f) > max_bytes:
                continue
            txt = open(f, encoding="utf-8", errors="replace").read()
        except OSError:
            continue
        for kind, ns in needles.items():
            for nd in ns:
                if nd and nd in txt:
                    hits.append((f, kind, nd))
    return hits


def lint(test_entries, agent_dirs, answer_dirs=(), artifacts=(), unseen=None, base=mf.TASKS):
    task_paths, ids, gold = [], set(), set()
    for e in test_entries:
        p = e if os.path.isabs(e) else os.path.join(base, e)
        task_paths.append(p)
        ids.add(mf.task_name(e))
        try:
            tj = json.load(open(p))
        except Exception:
            tj = None
        if tj:
            if tj.get("id"):
                ids.add(tj["id"])
            ps, ls = answer_info(tj)
            gold |= ls
            task_paths += list(ps)
    targets = task_paths + [base] + list(answer_dirs)
    if unseen:
        targets.append(unseen)
    problems = []
    for d in agent_dirs:
        for t, how in reachable_from(d, targets):
            problems.append({"check": "readable", "agent_dir": d, "answer": t, "how": how})
    needles = {"test_task_id": ids, "gold_literal": gold}
    if unseen:
        doc = json.load(open(unseen))
        needles["unseen_layout"] = {x["id"] for x in doc["layouts"]} | {w for x in doc["layouts"] for w in x["vocab"]}
    for f, kind, nd in scan_artifacts(artifacts, needles):
        problems.append({"check": "content", "artifact": f, "kind": kind, "needle": nd})
    return problems


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--tasks")
    ap.add_argument("--manifest")
    ap.add_argument("--split", default="test")
    ap.add_argument("--agent-dir", action="append", default=[])
    ap.add_argument("--answer-dir", action="append", default=[])
    ap.add_argument("--artifact", action="append", default=[])
    ap.add_argument("--unseen-layouts")
    ap.add_argument("--json")
    a = ap.parse_args(argv)
    if a.manifest:
        entries = [r["entry"] for r in mf.runnable(mf.load(a.manifest), a.split)]
    elif a.tasks:
        entries = mf.read_list(a.tasks)
    else:
        print("leakage_lint: --tasks or --manifest", file=sys.stderr)
        return 2
    if not a.agent_dir and not a.artifact:
        print("leakage_lint: nothing to check (--agent-dir / --artifact)", file=sys.stderr)
        return 2
    probs = lint(entries, a.agent_dir, a.answer_dir, a.artifact, a.unseen_layouts)
    out = {"ok": not probs, "test_tasks": len(entries), "problems": probs}
    if a.json:
        json.dump(out, open(a.json, "w"), indent=1)
    print(json.dumps(out, indent=1))
    return 0 if not probs else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
