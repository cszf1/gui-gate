#!/usr/bin/env python3
"""Human pilot analysis: does value-based asking burden people less than threshold asking, at what cost in success?

  human_pilot.py SESSIONS.jsonl [--md OUT.md] [--json OUT.json] [--baseline threshold_ask] [--margin 0.1]
  human_pilot.py assign --participants 12 --tasks TASKLIST|--n-tasks 8 [--seed 0] [--out ASSIGN.jsonl]
  human_pilot.py from-logs --meta META.jsonl [--human logs/human.jsonl ...] [--gate-log LOGTASK=GATE.jsonl ...]
                 [--out SESSIONS.jsonl]

from-logs builds SESSIONS.jsonl from what the systems log, so nobody re-types the numbers:
  * the gui-pi desktop client's `<data>/logs/human.jsonl` (schema `gui-pi-human/1`, desktop design 0.3 section 6.1):
    one `human_q` line per question {task, type: approve|clarify|takeover|help, outcome, refused, secs, repeated,
    takeover_steps} and one `human_summary` line per task {asks, by_type, repeated, person_active_s, person_wait_s,
    agent_blocked_s, takeover_steps, cancelled, timed_out, declined, unavailable, task_s};
  * the gate's `person_burden` events (gui-gate >= 0.10.1 log: kind, outcome, repeat, wait_s, active_s, totals), used
    as a cross-check, or as the source when a task has no human.jsonl lines. The gate only sees the time a question was
    open (its person_wait_s is the desktop's agent_blocked_s), never the time a person spent watching between
    questions, so a gate-only session has waiting_s = null and is left out of the waiting-time mean.
  META.jsonl gives what no log knows, one line per desktop task:
    {"log_task": "task-7", "participant": "p01", "task": "ow07", "condition": "value_ask", "order": 0,
     "success": true, "interruption_rating": 2, "gate_log": "optional/path/to/gate.jsonl"}

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
        if "repeated" in a:  # from a log that already decided it (gui-pi-human/1, gate person_burden)
            rep += 1 if a.get("repeated") else 0
            continue
        q = norm_q(a.get("question"))
        if q and q in seen:
            rep += 1
        seen.add(q)
    appr = [a for a in asks if a.get("kind") == "approve" and a.get("outcome") == "approved" and a.get("approval_correct") is not None]
    wrong = sum(1 for a in appr if a.get("approval_correct") is False)
    timeouts = [a for a in asks if a.get("outcome") == "timeout"]
    return {"asks": len(asks), "repeated": rep, "repeated_ratio": rep / len(asks) if asks else 0.0,
            "active_s": float(s.get("active_s") or 0.0),
            "waiting_s": None if s.get("waiting_s") is None and "waiting_s" in s else float(s.get("waiting_s") or 0.0),
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
                  "active_s": sum(m["active_s"] for m in ms) / n,
                  "waiting_s": (sum(w) / len(w)) if (w := [m["waiting_s"] for m in ms if m["waiting_s"] is not None]) else None,
                  "waiting_s_sessions": len(w),
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


# ---------------------------------------------------------------- from-logs: gui-pi-human/1 + gate person_burden
HUMAN_SCHEMA = "gui-pi-human/1"
DESKTOP_KIND = {"approve": "approve", "clarify": "answer", "takeover": "takeover", "help": "help"}
GATE_KIND = {"approve": "approve", "answer": "answer", "takeover": "takeover", "ask_help": "help", "help": "help"}
NO_ANSWER = {"timeout": "timeout", "timed out": "timeout", "expired": "timeout", "cancelled": "cancel", "window closed": "cancel",
             "window_closed": "cancel", "declined_form": "cancel", "unavailable": "unavailable",
             "channel_unavailable": "unavailable", "no_response": "no_response"}


def _yes(kind):
    return {"approve": "approved", "answer": "answered", "takeover": "taken_over", "help": "helped"}.get(kind, "approved")


def desktop_outcome(kind, word):
    """gui-pi-human/1 outcome words (desktop core human::label) -> this script's outcome vocabulary."""
    if word in ("allowed", "answered"):
        return _yes(kind)
    if word == "done by the person":
        return "taken_over" if kind == "takeover" else "helped"
    if word == "try another way":
        return "hint"
    if word in NO_ANSWER:
        return NO_ANSWER[word]
    return "denied"  # "declined" (approve / takeover refused, empty clarify answer)


def gate_outcome(kind, outcome):
    """gate human_outcome -> outcome vocabulary. A cancelled/declined form, closed window or timeout is no answer."""
    if outcome == "accepted":
        return _yes(kind)
    if outcome == "denied":
        return "denied"
    if outcome in ("cancelled", "declined", "window_closed"):
        return "cancel"
    return NO_ANSWER.get(outcome, outcome or "no_response")


def read_jsonl(path):
    out = []
    for line in open(path, encoding="utf-8"):
        line = line.strip()
        if not line:
            continue
        try:
            out.append(json.loads(line))
        except ValueError:
            continue  # a torn last line of a crashed run
    return out


def desktop_sessions(paths):
    """{log_task: {"asks": [...], "summary": {...} | None}} from the desktop client's human.jsonl files."""
    by = defaultdict(lambda: {"asks": [], "summary": None})
    for p in paths:
        for e in read_jsonl(p):
            if e.get("schema") != HUMAN_SCHEMA:
                continue
            t = str(e.get("task") or "")
            if e.get("ev") == "human_q":
                k = DESKTOP_KIND.get(e.get("type"), e.get("type") or "answer")
                by[t]["asks"].append({"kind": k, "outcome": desktop_outcome(k, e.get("outcome")), "secs": float(e.get("secs") or 0.0),
                                      "repeated": bool(e.get("repeated")), "refused": bool(e.get("refused")),
                                      "takeover_steps": int(e.get("takeover_steps") or 0), "raw_outcome": e.get("outcome")})
            elif e.get("ev") == "human_summary":
                by[t]["summary"] = e
    return dict(by)


def gate_burden(path):
    """The gate's person_burden events of one log: (asks, last totals)."""
    asks, totals = [], None
    for e in read_jsonl(path):
        if e.get("ev") != "person_burden":
            continue
        k = GATE_KIND.get(e.get("kind"), e.get("kind") or "answer")
        asks.append({"kind": k, "outcome": gate_outcome(k, e.get("outcome")), "secs": float(e.get("wait_s") or 0.0),
                     "repeated": bool(e.get("repeat")), "label": e.get("label"), "raw_outcome": e.get("outcome")})
        totals = e.get("totals") or totals
    return asks, (totals or {})


def from_logs(meta, human_paths=(), gate_logs=None):
    """Pilot sessions (the SESSIONS.jsonl shape) from META lines + desktop human.jsonl + gate logs."""
    gate_logs = dict(gate_logs or {})
    desk = desktop_sessions(human_paths)
    out = []
    for m in meta:
        lt = str(m.get("log_task") or m.get("task"))
        s = {k: m[k] for k in ("participant", "task", "condition", "order", "success", "interruption_rating") if k in m}
        s.setdefault("task", lt)
        s["log_task"] = lt
        d = desk.get(lt)
        gl = m.get("gate_log") or gate_logs.get(lt)
        g = gate_burden(gl) if gl and os.path.exists(gl) else None
        if d and (d["asks"] or d["summary"]):
            sm = d["summary"]
            asks = d["asks"]
            s["asks"] = asks
            s["source"] = HUMAN_SCHEMA
            if sm:
                s["active_s"] = float(sm.get("person_active_s") or 0.0)
                s["waiting_s"] = float(sm.get("person_wait_s") or 0.0)
                s["agent_blocked_s"] = float(sm.get("agent_blocked_s") or 0.0)
                s["takeover_steps"] = int(sm.get("takeover_steps") or 0)
                s["cancelled"] = int(sm.get("cancelled") or 0) > 0
                s["timed_out"] = int(sm.get("timed_out") or 0) > 0
                s["total_s"] = float(sm.get("task_s") or 0.0)
                if int(sm.get("asks") or 0) != len(asks):
                    s["warning"] = f"human_summary says {sm.get('asks')} asks, {len(asks)} human_q lines"
            else:  # the task did not end cleanly: rebuild what the questions give, no waiting time
                s["active_s"] = sum(a["secs"] for a in asks if a["outcome"] not in ("timeout", "cancel", "unavailable", "no_response"))
                s["waiting_s"] = None
                s["agent_blocked_s"] = sum(a["secs"] for a in asks)
                s["takeover_steps"] = sum(a["takeover_steps"] for a in asks)
                s["cancelled"] = any(a["outcome"] == "cancel" for a in asks)
                s["timed_out"] = any(a["outcome"] == "timeout" for a in asks)
                s["warning"] = "no human_summary line (task did not end cleanly): waiting_s unknown"
            if g is not None:
                s["gate_check"] = {"asks": len(g[0]), "not_asked_again": g[1].get("not_asked_again", 0),
                                   "agrees": len(g[0]) == len(asks)}
        elif g is not None:
            asks, tot = g
            s["asks"] = asks
            s["source"] = "gate person_burden"
            s["active_s"] = float(tot.get("person_active_s") or 0.0)
            s["waiting_s"] = None  # the gate cannot see the time a person spent watching between questions
            s["agent_blocked_s"] = float(tot.get("person_wait_s") or 0.0)
            s["takeover_steps"] = int(tot.get("takeover_steps") or 0)
            s["cancelled"] = any(a["outcome"] == "cancel" for a in asks)
            s["timed_out"] = any(a["outcome"] == "timeout" for a in asks)
            s["not_asked_again"] = tot.get("not_asked_again", 0)
        else:
            s.update({"asks": [], "source": "none", "active_s": 0.0, "waiting_s": None, "takeover_steps": 0,
                      "cancelled": False, "timed_out": False, "warning": "no person log for this task"})
        s.setdefault("total_s", float(m.get("total_s") or 0.0))
        if "total_s" in m:
            s["total_s"] = float(m["total_s"])
        out.append(s)
    return out


def from_logs_main(a):
    human, gates, meta_p, out_p = [], {}, None, None
    i = 0
    while i < len(a):
        if a[i] == "--human":
            human.append(a[i + 1]); i += 2
        elif a[i] == "--gate-log":
            k, _, v = a[i + 1].partition("="); gates[k] = v; i += 2
        elif a[i] == "--meta":
            meta_p = a[i + 1]; i += 2
        elif a[i] == "--out":
            out_p = a[i + 1]; i += 2
        else:
            raise SystemExit(f"unknown argument {a[i]}")
    if not meta_p:
        raise SystemExit("from-logs needs --meta META.jsonl (participant, condition, success per desktop task)")
    sessions = from_logs(read_jsonl(meta_p), human, gates)
    txt = "".join(json.dumps(x, ensure_ascii=False) + "\n" for x in sessions)
    if out_p:
        open(out_p, "w", encoding="utf-8").write(txt)
    else:
        sys.stdout.write(txt)
    return sessions


def to_md(pc, cmp_):
    L = ["# Human pilot: threshold asking vs value asking", "",
         "| condition | sessions | asks/task | repeated q | active s | waiting s | takeover steps | cancel | timeout | "
         "wrong approval | success | Wilson 95% | total s (all) | interruption (1-7) |", "|" + "---|" * 14]
    for c, a in pc.items():
        wa = "-" if a["wrong_approval_rate"] is None else f"{a['wrong_approval_rate']:.2f} ({a['approvals_judged']})"
        ir = "-" if a["interruption_rating"] is None else f"{a['interruption_rating']:.2f}"
        L.append(f"| {c} | {a['sessions']} | {a['asks_per_task']:.2f} | {a['repeated_question_ratio']:.2f} | {a['active_s']:.1f} | "
                 f"{'-' if a['waiting_s'] is None else format(a['waiting_s'], '.1f')} | {a['takeover_steps']:.2f} | {a['cancel_rate']:.2f} | {a['timeout_rate']:.2f} | {wa} | "
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
    if a and a[0] == "from-logs":
        return from_logs_main(a[1:])
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
