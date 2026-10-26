#!/usr/bin/env python3
"""Failure classification of one finished bench run (read only on the run's folder).

    python3 bench/failure_analysis.py RESULTS_DIR [--osworld OSWORLD_CHECKOUT] [-o OUT.json]

Uses the same run selection as bench/offline_v2.py (`first_valid`: per task the first attempt that is not infra), then
gives every FAILED used run exactly one primary cause, by a fixed precedence over signals that the logs actually carry:

  1. budget       stop = timeout / max_turns (the run was cut, whatever else happened)
  2. gate_block   the gate refused a step and the final answer gives up / names the refusal (approval denied, refused)
  3. site_blocked the final answer says the web site refused the environment (403, bot check, captcha, Akamai ...)
  4. infeasible_missed  the task is an OSWorld `infeasible` task and the agent did not answer FAIL
  5. gave_up      the agent answered FAIL on a feasible task (no gate refusal named)
  6. loop         loop breaker / repeated identical steps dominate the run (>= 3 loop_breaker events or repeats)
  7. exec_errors  the gate let steps through and many failed (>= 5 executed actions with is_error)
  8. wrong_result the agent said DONE, nothing above applies: model ability / wrong reading of the task / evaluator strictness

Contributing flags (any number per run) are counted apart: gate refusals by kind (incl. the known false positive
`str.replace(...)` read as a file move, and scratch files left in /tmp by EARLIER tasks on the same box), env
differences named in the final (Chromium vs Chrome, versions), partial score, reviewer denials.
`evaluator` cannot be decided from logs alone; partial scores and the env-difference flag are the only hints.
"""
import argparse, glob, json, os, re, sys
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from offline_v2 import audit_runs, gate_file, read_jsonl_robust  # noqa: E402

SITE = re.compile(r"(?i)\b(403|forbidden|access denied|bot[- ]check|captcha|akamai|cloudflare|blocked (by|in this env)|"
                  r"ip-blocked|hard-blocked|slide to verify|human verification|are you a robot|perimeterx|rate[- ]limit)")
GATE_WORDS = re.compile(r"(?i)(refus|not approved|approval|denied|deny|the gate|was blocked by the safety|not allowed)")
ENV_DIFF = re.compile(r"(?i)(chromium|installed (browser|version)|not installed|version \d|xfce|debian|no such (app|program))")
FP_REPLACE = re.compile(r"script code deletes or moves files \(\.replace\(['\"]")
TMP_RESIDUE = re.compile(r"high-risk action: (`rm` deletes|`>` would overwrite existing file)")


def infeasible_ids(osworld):
    out = set()
    if not osworld:
        return out
    for p in glob.glob(os.path.join(osworld, "evaluation_examples", "examples", "*", "*.json")):
        try:
            j = json.load(open(p, encoding="utf-8"))
        except (OSError, ValueError):
            continue
        ev = j.get("evaluator") or {}
        f = ev.get("func")
        if f == "infeasible" or (isinstance(f, list) and "infeasible" in f):
            out.add(os.path.basename(os.path.dirname(p)) + "-" + j["id"])
    return out


def gate_events(path):
    rows, _ = read_jsonl_robust(path) if os.path.exists(path) else ([], [])
    return rows


def refusal_kind(e):
    r = e.get("reason") or ""
    if FP_REPLACE.search(r):
        return "fp_str_replace"
    if TMP_RESIDUE.search(r):
        return "own_scratch_or_tmp_residue"
    if "high-risk action" in r or "medium-risk action" in r:
        return "risk_" + str(e.get("decision"))
    if "loop breaker" in r:
        return "loop_stop"
    if e.get("decision") in ("look_full", "look_light"):
        return "observe_first"
    if e.get("decision") == "handoff":
        return "handoff_captcha"
    return str(e.get("decision"))


def full_final(results_dir, tag, attempt, row):
    """the whole final answer (runs_scored keeps only its first 160 characters)"""
    from offline_v2 import _stem
    p = os.path.join(results_dir, "pi", _stem(tag, attempt) + ".jsonl.final.txt")
    try:
        return open(p, encoding="utf-8", errors="replace").read()
    except OSError:
        return row.get("final") or ""


def pi_tail(results_dir, tag, attempt, wall):
    """(seconds between the last Pi event and the end of the run, type of that last event): a long silent tail after
    turn_start / message_start is a model call that never answered (relay stall), after agent_end a Pi process that did
    not exit, after tool_execution_start a tool call (the gate) that never returned"""
    from offline_v2 import _stem
    p = os.path.join(results_dir, "pi", _stem(tag, attempt) + ".jsonl.times")
    rows, _ = read_jsonl_robust(p) if os.path.exists(p) else ([], [])
    if not rows or not wall:
        return None, None
    return round(float(wall) - float(rows[-1].get("t") or 0), 1), rows[-1].get("type")


def classify(row, ev, infeasible, final=None, tail=(None, None)):
    final = final if final is not None else (row.get("final") or "")
    stop = row.get("stop")
    acts = [e for e in ev if e.get("ev") == "action"]
    refused = [e for e in acts if not e.get("executed")]
    hard_ref = [e for e in refused if e.get("decision") in ("deny", "ask_human", "stop_loop", "handoff")]
    exec_err = sum(1 for e in acts if e.get("executed") and e.get("is_error"))
    loops = sum(1 for e in ev if e.get("ev") == "loop_breaker")
    repeats = sum(1 for e in ev if e.get("ev") == "retro_repeat")
    flags = Counter(refusal_kind(e) for e in refused)
    if ENV_DIFF.search(final):
        flags["env_difference_named"] += 1
    sc = row.get("score") or 0
    if 0 < sc < 1:
        flags["partial_score"] += 1
    flags["reviewer_deny"] += sum(1 for e in refused if e.get("tier") == "review" and e.get("decision") == "deny")
    flags = +flags
    said_fail = bool(row.get("final_fail")) or final.lstrip().upper().startswith("FAIL")
    if stop in ("timeout", "max_turns"):
        cause = "budget"
    elif hard_ref and (said_fail or GATE_WORDS.search(final)) and (said_fail or re.search(r"(?i)\bgate\b|approv", final)):
        cause = "gate_block"
    elif SITE.search(final):
        cause = "site_blocked"
    elif row["task"] in infeasible and not said_fail:
        cause = "infeasible_missed"
    elif said_fail:
        cause = "gave_up"
    elif loops >= 3 or repeats >= 6:
        cause = "loop"
    elif exec_err >= 5:
        cause = "exec_errors"
    else:
        cause = "wrong_result"
    sub = None
    if cause == "budget":
        idle, last = tail
        if idle is not None and idle >= 600 and last in ("turn_start", "message_start"):
            sub = "model_call_stall"
        elif idle is not None and idle >= 600 and last == "agent_end":
            sub = "pi_did_not_exit"
        elif idle is not None and idle >= 600 and last == "tool_execution_start":
            sub = "tool_call_stall"
        else:
            sub = "loop" if (loops >= 3 or repeats >= 6) else ("exec_errors" if exec_err >= 5 else "slow_progress")
    return {"task": row["task"], "tag": row["tag"], "cause": cause, "sub": sub, "stop": stop, "tail_idle_s": tail[0], "tail_last_event": tail[1], "score": sc, "said_fail": said_fail,
            "infeasible_task": row["task"] in infeasible, "refused": len(refused), "hard_refused": len(hard_ref),
            "exec_errors": exec_err, "loop_breaker": loops, "retro_repeat": repeats, "flags": dict(flags),
            "final": final[:200]}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("results_dir")
    ap.add_argument("--osworld", default=None, help="OSWorld checkout (evaluation_examples/) to know infeasible tasks")
    ap.add_argument("-o", "--out", default=None)
    a = ap.parse_args()
    audit = audit_runs(a.results_dir)
    scored, _ = read_jsonl_robust(os.path.join(a.results_dir, "runs_scored.jsonl"))
    by_tag = {r["tag"]: r for r in scored}
    inf = infeasible_ids(a.osworld)
    rows = []
    succ_inf = 0
    for t, v in sorted(audit["tasks"].items()):
        if v["status"] != "used":
            continue
        r = by_tag[v["tag"]]
        if v["success"]:
            succ_inf += t in inf
            continue
        rows.append(classify(r, gate_events(gate_file(a.results_dir, v["tag"], v["attempt"])), inf,
                             full_final(a.results_dir, v["tag"], v["attempt"], r),
                             pi_tail(a.results_dir, v["tag"], v["attempt"], r.get("wall_s"))))
    causes = Counter(r["cause"] for r in rows)
    flags = Counter()
    runs_with = Counter()
    for r in rows:
        for k, n in r["flags"].items():
            flags[k] += n
            runs_with[k] += 1
    # the same flags over successful runs, so a flag is not read as a failure cause when it is just as common in wins
    win_with = Counter()
    for t, v in audit["tasks"].items():
        if v["status"] == "used" and v["success"]:
            ev = gate_events(gate_file(a.results_dir, v["tag"], v["attempt"]))
            for k in {refusal_kind(e) for e in ev if e.get("ev") == "action" and not e.get("executed")}:
                win_with[k] += 1
    out = {"results_dir": os.path.relpath(a.results_dir), "used": sum(1 for v in audit["tasks"].values() if v["status"] == "used"),
           "failed": len(rows), "infeasible_tasks_known": len(inf), "infeasible_solved": succ_inf,
           "by_domain": dict(Counter(r["task"].split("-")[0] for r in rows)),
           "causes": dict(causes.most_common()), "flag_counts": dict(flags.most_common()), "failed_runs_with_flag": dict(runs_with.most_common()),
           "successful_runs_with_refusal_kind": dict(win_with.most_common()),
           "budget_sub": dict(Counter(r["sub"] for r in rows if r["cause"] == "budget")),
           "wrong_result_flags": dict(Counter(k for r in rows if r["cause"] == "wrong_result" for k in r["flags"])),
           "causes_by_domain": {c: dict(Counter(r["task"].split("-")[0] for r in rows if r["cause"] == c)) for c in causes},
           "runs": rows}
    s = json.dumps(out, indent=1, ensure_ascii=False)
    if a.out:
        open(a.out, "w", encoding="utf-8").write(s + "\n")
    print(json.dumps({k: out[k] for k in out if k != "runs"}, indent=1, ensure_ascii=False))


if __name__ == "__main__":
    main()
