#!/usr/bin/env python3
"""Full OSWorld-Verified task list for run_bench.sh, taken unmodified from an OSWorld checkout.

  make_verified_list.py OSWORLD_DIR [--set test_nogdrive] [--name ovf]

writes bench/tasks/<name>/<domain>-<uuid>.json as symlinks to OSWORLD_DIR/evaluation_examples/examples/<domain>/<uuid>.json
and bench/tasks/<name>.txt (one "<name>/<domain>-<uuid>.json" per line, in the set's order). test_nogdrive = the official
OSWorld-Verified test_all (369) minus the 8 Google Drive tasks = 361. Nothing in any task is changed (osworld_native.py
applies its documented compat rewrites at setup/eval time).
"""
import argparse, json, os

ap = argparse.ArgumentParser()
ap.add_argument("osworld")
ap.add_argument("--set", default="test_nogdrive")
ap.add_argument("--name", default="ovf")
ap.add_argument("--shuffle", type=int, default=None, help="shuffle the run order with this seed (partial runs stay representative)")
a = ap.parse_args()
here = os.path.dirname(os.path.abspath(__file__))
ex = os.path.join(a.osworld, "evaluation_examples")
sets = json.load(open(os.path.join(ex, a.set + ".json")))
out = os.path.join(here, "tasks", a.name)
os.makedirs(out, exist_ok=True)
lines = []
for dom, ids in sets.items():
    for i in ids:
        src = os.path.join(ex, "examples", dom, i + ".json")
        json.load(open(src))  # must exist and parse
        dst = os.path.join(out, f"{dom}-{i}.json")
        if os.path.lexists(dst):
            os.remove(dst)
        os.symlink(src, dst)
        lines.append(f"{a.name}/{dom}-{i}.json")
if a.shuffle is not None:
    import random
    random.Random(a.shuffle).shuffle(lines)
open(os.path.join(here, "tasks", a.name + ".txt"), "w").write("\n".join(lines) + "\n")
print(len(lines), "tasks ->", os.path.join(here, "tasks", a.name + ".txt"))
