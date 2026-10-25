#!/usr/bin/env python3
"""Human pilot analysis: does value-based asking burden people less than threshold asking, at what cost in success?

  human_pilot.py SESSIONS.jsonl [--md OUT.md] [--json OUT.json] [--baseline threshold_ask] [--margin 0.1]
  human_pilot.py assign --participants 12 --tasks TASKLIST|--n-tasks 8 [--seed 0] [--out ASSIGN.jsonl]

A simulated person tunes the protocol and `--human deny` tests unattended runs; neither shows that a REAL person's
burden went down. This script analyses a real pilot. One JSON line per (participant, task) session:

  {"participant": "p03", "task": "ow07", "condition": "threshold_ask" | "value_ask", "order": 0,   # position in the
   "asks": [{"kind": "approve|answer|takeover|help", "question": "...", "t_ask": 12.0, "t_answer": 19.5,   # session
             "outcome": "approved|denied|answered|taken_over|helped|timeout|cancel",
             "timeout_reason": "...",                      # timeouts: recorded, never read as "the user refused"
             "approval_correct": true|false|null}],         # approvals only: was approving right (ground truth)?
   "active_s": 41.0, "waiting_s": 63.0, "takeover_steps": 3, "cancelled": false, "timed_out": false,
   "success": true, "total_s": 312.0, "interruption_rating": 2}     # 1 (not disruptive) .. 7 (very disruptive)

Per condition: asks per task, repeated-question ratio (a question whose normalised text was already asked in the same
session), active vs waiting person-seconds, takeover steps, cancel and timeout rates, wrong-approval rate
(approvals that ground truth says were wrong / approvals with a ground truth), success, total time over ALL sessions
(failures included), mean interruption rating.
Paired comparison: per (participant, task) pairs when both conditions ran the same task for the same person, else
per participant means; bootstrap CI + sign-flip permutation p for each metric, McNemar on success, and an optional
non-inferiority test of success with a pre-registered margin. The order balance (how many pairs ran each condition
first) is reported; an unbalanced pilot is flagged, since learning effects then masquerade as condition effects.
"""
import json
import os
import random
import re
import sys
from collections import Counter, defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import stats  # noqa: E402

CONDITIONS = ("threshold_ask", "value_ask")
METRICS = ("asks", "repeated_ratio", "active_s", "waiting_s", "takeover_steps", "cancelled", "timed_out", "success",
           "total_s", "interruption_rating")
LOWER_IS_BETTER = {"asks", "repeated_ratio", "active_s", "waiting_s", "takeover_steps", "cancelled", "timed_out",
                   "total_s", "interruption_rating"}


def norm_q(q):
    return re.sub(r"[^a-z0-9]+", " ", (q or "").lower()).strip()


def session_metrics(s):
    asks = s.get("asks") or []
    seen, rep = set(), 0
    for a in asks:
        q = norm_q(a.get("question"))
        if q and q in seen:
            rep += 1
        seen.add(q)
    appr = [a for a in asks if a.get("kind") == "approve" and a.get("outcome") == "approved" and a.get("approval_correct") is not None]
    wrong = sum(1 for a in appr if a.get("approval_correct") is False)
    timeouts = [a for a in asks if a.get("outcome") == "timeout"]
    return {"asks": len(asks), "repeated": rep, "repeated_ratio": rep / len(asks) if asks else 0.0,
            "active_s": float(s.get("active_s") or 0.0), "waiting_s": float(s.get("waiting_s") or 0.0),
            "takeover_steps": int(s.get("takeover_steps") or 0), "cancelled": 1.0 if s.get("cancelled") else 0.0,
            "timed_out": 1.0 if (s.get("timed_out") or timeouts) else 0.0, "success": 1.0 if s.get("success") else 0.0,
            "total_s": float(s.get("total_s") or 0.0),
            "interruption_rating": None if s.get("interruption_rating") is None else float(s["interruption_rating"]),
            "approvals_judged": len(appr), "wrong_approvals": wrong,
            "timeout_reasons": [a.get("timeout_reason") or "unrecorded" for a in timeouts],
            "kinds": Counter(a.get("kind") for a in asks)}


def per_condition(sessions):
    out = {}
    for c in sorted({s["condition"] for s in sessions}):
        ms = [session_metrics(s) for s in sessions if s["condition"] == c]
        n = len(ms)
        rat = [m["interruption_rating"] for m in ms if m["interruption_rating"] is not None]
        judged = sum(m["approvals_judged"] for m in ms)
        tot_asks = sum(m["asks"] for m in ms)
        kinds = Counter()
        reasons = Counter()
        for m in ms:
            kinds.update(m["kinds"])
            reasons.update(m["timeout_reasons"])
        k_ok = int(sum(m["success"] for m in ms))
        out[c] = {"sessions": n, "asks_per_task": tot_asks / n if n else None,
                  "repeated_question_ratio": sum(m["repeated"] for m in ms) / tot_asks if tot_asks else 0.0,
                  "active_s": sum(m["active_s"] for m in ms) / n, "waiting_s": sum(m["waiting_s"] for m in ms) / n,
                  "takeover_steps": sum(m["takeover_steps"] for m in ms) / n,
                  "cancel_rate": sum(m["cancelled"] for m in ms) / n, "timeout_rate": sum(m["timed_out"] for m in ms) / n,
                  "wrong_approval_rate": (sum(m["wrong_approvals"] for m in ms) / judged) if judged else None,
                  "approvals_judged": judged, "success": k_ok / n, "success_wilson95": stats.wilson(k_ok, n),
                  "total_s_all_sessions": sum(m["total_s"] for m in ms) / n,
                  "interruption_rating": sum(rat) / len(rat) if rat else None, "ratings": len(rat),
                  "asks_by_kind": dict(kinds), "timeout_reasons": dict(reasons)}
    return out


def pairs(sessions, a="threshold_ask", b="value_ask"):
    """(participant, task) pairs with both conditions; falls back to participant means when tasks differ."""
    by = defaultdict(dict)
    for s in sessions:
        by[(s["participant"], s["task"])][s["condition"]] = s
    tp = [(k, v[a], v[b]) for k, v in sorted(by.items()) if a in v and b in v]
    if tp:
        return "participant_task", tp
    byp = defaultdict(lambda: defaultdict(list))
    for s in sessions:
        byp[s["participant"]][s["condition"]].append(s)
    pp = [((p, "*"), v[a], v[b]) for p, v in sorted(byp.items()) if a in v and b in v]
    return "participant", pp


def _val(x, metric):
    if isinstance(x, list):  # participant-level: mean over that participant's sessions
        vs = [session_metrics(s)[metric] for s in x]
        vs = [v for v in vs if v is not None]
        return sum(vs) / len(vs) if vs else None
    return session_metrics(x)[metric]


def order_balance(sessions, a="threshold_ask", b="value_ask"):
    first = Counter()
    byp = defaultdict(list)
    for s in sessions:
        byp[s["participant"]].append(s)
    for p, ss in byp.items():
        ss = sorted(ss, key=lambda s: s.get("order", 0))
        if ss:
            first[ss[0]["condition"]] += 1
    na, nb = first.get(a, 0), first.get(b, 0)
    return {"participants_first": dict(first), "balanced": abs(na - nb) <= 1}


def paired_compare(sessions, a="threshold_ask", b="value_ask", margin=None, preregistered=True):
    unit, ps = pairs(sessions, a, b)
    res = {"unit": unit, "pairs": len(ps), "order": order_balance(sessions, a, b), "metrics": {}}
    for m in METRICS:
        xs, ys = [], []
        for _, sa, sb in ps:
            va, vb = _val(sa, m), _val(sb, m)
            if va is not None and vb is not None:
                xs.append(va)
                ys.append(vb)
        if not xs:
            continue
        bt = stats.paired_bootstrap(xs, ys, seed=7)
        res["metrics"][m] = {"n": len(xs), "mean_" + a: sum(xs) / len(xs), "mean_" + b: sum(ys) / len(ys),
                             "diff": bt["diff"], "ci95": bt["ci"], "permutation_p": stats.paired_permutation(xs, ys, seed=7)["p"],
                             "better_if": "lower" if m in LOWER_IS_BETTER else "higher"}
    if unit == "participant_task":
        sa = [session_metrics(x)["success"] for _, x, _ in ps]
        sb = [session_metrics(y)["success"] for _, _, y in ps]
        res["success_mcnemar"] = stats.mcnemar_exact(sa, sb)
        if margin is not None:
            ni = stats.noninferiority(sa, sb, margin, seed=7, preregistered=preregistered)
            res["success_noninferiority"] = ni
            res["success_noninferiority_verdict"] = stats.verdict(ni, "success")
    return res


def assign(participants, tasks, seed=0):
    """Counterbalanced plan: each participant does every task once, half the tasks under each condition (a person
    cannot do the same task twice without learning it). The condition of the first block alternates across
    participants (AB / BA) and the task-to-condition mapping alternates every two participants, so every task runs
    under both conditions equally often and every participant sees both conditions (within-person pairing).
    Use a multiple of 4 participants for full balance."""
    rng = random.Random(seed)
    tasks = list(tasks)
    rng.shuffle(tasks)
    half = len(tasks) // 2
    plan = []
    for i in range(participants):
        p = f"p{i + 1:02d}"
        first = CONDITIONS[i % 2]
        second = CONDITIONS[1 - i % 2]
        ta, tb = (tasks[:half], tasks[half:]) if (i // 2) % 2 == 0 else (tasks[half:], tasks[:half])
        order = 0
        for c, ts in ((first, ta), (second, tb)):
            for t in ts:
                plan.append({"participant": p, "task": t, "condition": c, "order": order})
                order += 1
    return plan


def to_md(pc, cmp_):
    L = ["# Human pilot: threshold asking vs value asking", "",
         "| condition | sessions | asks/task | repeated q | active s | waiting s | takeover steps | cancel | timeout | "
         "wrong approval | success | Wilson 95% | total s (all) | interruption (1-7) |", "|" + "---|" * 14]
    for c, a in pc.items():
        wa = "-" if a["wrong_approval_rate"] is None else f"{a['wrong_approval_rate']:.2f} ({a['approvals_judged']})"
        ir = "-" if a["interruption_rating"] is None else f"{a['interruption_rating']:.2f}"
        L.append(f"| {c} | {a['sessions']} | {a['asks_per_task']:.2f} | {a['repeated_question_ratio']:.2f} | {a['active_s']:.1f} | "
                 f"{a['waiting_s']:.1f} | {a['takeover_steps']:.2f} | {a['cancel_rate']:.2f} | {a['timeout_rate']:.2f} | {wa} | "
                 f"{a['success']:.2f} | {a['success_wilson95'][0]:.2f}-{a['success_wilson95'][1]:.2f} | {a['total_s_all_sessions']:.1f} | {ir} |")
    if cmp_:
        o = cmp_["order"]
        L += ["", f"## Paired ({cmp_['unit']}, {cmp_['pairs']} pairs; first condition per participant {o['participants_first']}"
                  f"{'' if o['balanced'] else ' - ORDER UNBALANCED: learning effects are confounded with the condition'})", "",
              "| metric | n | diff (value - threshold) | 95% CI | permutation p | better if |", "|---|---|---|---|---|---|"]
        for m, d in cmp_["metrics"].items():
            L.append(f"| {m} | {d['n']} | {d['diff']:+.3f} | {d['ci95'][0]:+.3f}..{d['ci95'][1]:+.3f} | {d['permutation_p']:.4f} | {d['better_if']} |")
        if "success_noninferiority_verdict" in cmp_:
            L += ["", "Success: " + cmp_["success_noninferiority_verdict"]]
    L += ["", "Timeouts deny the one action and are recorded with their reason; they are not read as the person refusing to help.",
          "A non-significant difference is not evidence that the two conditions burden people equally."]
    return "\n".join(L) + "\n"


def main(a):
    if a and a[0] == "assign":
        flags = {a[i]: a[i + 1] for i in range(1, len(a) - 1) if a[i].startswith("--")}
        if "--tasks" in flags:
            tasks = [os.path.basename(x.strip())[:-5] if x.strip().endswith(".json") else x.strip() for x in open(flags["--tasks"]) if x.strip()]
        else:
            tasks = [f"t{i + 1}" for i in range(int(flags.get("--n-tasks", 8)))]
        plan = assign(int(flags.get("--participants", 12)), tasks, int(flags.get("--seed", 0)))
        txt = "".join(json.dumps(x) + "\n" for x in plan)
        if "--out" in flags:
            open(flags["--out"], "w").write(txt)
        else:
            sys.stdout.write(txt)
        return plan
    flags = {a[i]: a[i + 1] for i in range(1, len(a) - 1) if a[i].startswith("--")}
    sessions = [json.loads(x) for x in open(a[0]) if x.strip()]
    pc = per_condition(sessions)
    cmp_ = paired_compare(sessions, flags.get("--baseline", "threshold_ask"), "value_ask",
                          float(flags["--margin"]) if "--margin" in flags else None, flags.get("--preregistered", "1") == "1") \
        if {"threshold_ask", "value_ask"} <= set(pc) else None
    res = {"conditions": pc, "paired": cmp_}
    md = to_md(pc, cmp_)
    if "--md" in flags:
        open(flags["--md"], "w").write(md)
    if "--json" in flags:
        json.dump(res, open(flags["--json"], "w"), indent=1)
    print(md)
    return res


if __name__ == "__main__":
    main(sys.argv[1:])
