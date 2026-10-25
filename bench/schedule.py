#!/usr/bin/env python3
"""Run order for paired comparisons: interleave policies / systems task by task (ABBA counterbalancing).

  schedule.py --tasks TASKLIST --conds "A B" --repeats "0 1 2" [--mode abba|blocked] [--out SCHEDULE]

Writes one "<repeat> <task-entry> <cond>" line per run, in run order. run_bench.sh --interleave abba executes it.

blocked  every cond of a task back to back, always in the given order (the old run_bench.sh order).
abba     every cond of a task back to back, the order reversed on every other task (A B | B A | A B ...) and the
         starting order flipped on every other repeat, so slow drift of the API / box (latency, relay errors,
         caches warming) hits every cond equally often first and second. With 3+ conds the reversal is a
         rotation-plus-reversal (Williams-style balanced sequence over consecutive tasks).

Repeats are INDEPENDENT REPEATS: the model API used so far takes no sampling seed, so the repeat number is a label,
not a controlled random seed. Each run records "repeat_kind":"independent" and whether the API accepted a seed
(PI_API_SEED_ACCEPTED=1 only after it was verified for that provider; default false). GUI_GATE_VALUE_SEED is a
different thing: the gate's own RNG for the vp_random policy, which IS seeded by the repeat number.
"""
import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


def williams_rows(n):
    """Balanced Latin square rows (Williams design) over n conditions: each cond precedes every other equally often
    across consecutive rows (exactly for even n; for odd n use the rows and their reverses)."""
    base = []
    lo, hi = 0, n - 1
    for i in range(n):
        if i % 2 == 0:
            base.append(lo)
            lo += 1
        else:
            base.append(hi)
            hi -= 1
    rows = [[(b + r) % n for b in base] for r in range(n)]
    if n % 2:
        rows += [list(reversed(x)) for x in rows]
    return rows


def order_for(conds, task_index, repeat_index, mode="abba"):
    conds = list(conds)
    if mode == "blocked" or len(conds) < 2:
        return conds
    if len(conds) == 2:
        flip = (task_index + repeat_index) % 2 == 1
        return conds[::-1] if flip else conds
    rows = williams_rows(len(conds))
    return [conds[i] for i in rows[(task_index + repeat_index) % len(rows)]]


def schedule(tasks, conds, repeats, mode="abba"):
    out = []
    for ri, r in enumerate(repeats):
        for ti, t in enumerate(tasks):
            for c in order_for(conds, ti, ri, mode):
                out.append((r, t, c))
    return out


def balance(sched):
    """{cond: {position: count}} of each cond's position inside its task block (a check for the report)."""
    from collections import defaultdict
    pos = defaultdict(lambda: defaultdict(int))
    block = []
    last = None
    for r, t, c in sched + [(None, None, None)]:
        if (r, t) != last and block:
            for i, cc in enumerate(block):
                pos[cc][i] += 1
            block = []
        if c is not None:
            block.append(c)
        last = (r, t)
    return {c: dict(v) for c, v in pos.items()}


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--tasks", required=True)
    ap.add_argument("--conds", required=True)
    ap.add_argument("--repeats", default="0")
    ap.add_argument("--mode", default="abba", choices=["abba", "blocked"])
    ap.add_argument("--out")
    a = ap.parse_args(argv)
    tasks = [x.strip() for x in open(a.tasks) if x.strip() and not x.strip().startswith("#")]
    s = schedule(tasks, a.conds.split(), a.repeats.split(), a.mode)
    txt = "".join(f"{r} {t} {c}\n" for r, t, c in s)
    if a.out:
        open(a.out, "w").write(txt)
    else:
        sys.stdout.write(txt)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
