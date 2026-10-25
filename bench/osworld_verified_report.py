#!/usr/bin/env python3
"""Report for a full OSWorld-Verified run (run_bench.sh over bench/tasks/ovf.txt), one condition.

usage: python3 bench/osworld_verified_report.py RUNDIR [--expected 361] [--md OUT.md] [--json OUT.json] [--csv OUT.csv]

Per task (the latest attempt of each task tag): domain, OSWorld score (official evaluator, osworld_native.py eval),
success (score >= 1), turns (Pi assistant messages = steps), stop reason (agent / max_turns / timeout), wall time,
tokens, tool calls, gate actions / interventions, request_human calls, infra status.

Scoring rules:
- infra rows (relay_error after all retries, setup_failed, eval_error) are listed separately and excluded from the
  "valid" denominator; the official-style rate counts every task of the expected list (missing / infra = 0).
- partial scores (0 < score < 1, a few OSWorld metrics are fractional) are summed in "score_sum" (the leaderboard
  averages scores) and also reported as success = score >= 1.

Gate statistics: reviewer calls (step review `judge`, risk review `risk_gate.reviewer`) with latency (`ms`),
0.9.7 model roles from `briefing` (feasibility, planner) and `role` events (verifier, judge, search, compact, grounder):
HTTP call counts and an approximate latency per role invocation (briefing.ms for feasibility+planner together,
inspect.ms for search / grounder, check_done.ms for the verifier, otherwise the gap to the previous gate log event).
"""
import argparse
import collections
import csv
import glob
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from stats import wilson  # noqa: E402

DISCLAIMER = ("custom environment based on OSWorld tasks and evaluators (run natively, not the official VM / app versions / "
              "budget): a custom-environment result, not an official OSWorld-Verified score; not comparable to the "
              "OSWorld leaderboard")


def jl(p):
    out = []
    try:
        with open(p, errors="replace") as f:
            for line in f:
                try:
                    out.append(json.loads(line))
                except Exception:
                    pass
    except OSError:
        pass
    return out


def jload(p):
    try:
        return json.load(open(p))
    except Exception:
        return None


def med(xs):
    xs = [x for x in xs if x is not None]
    return round(statistics.median(xs), 1) if xs else None


def mean(xs):
    xs = [x for x in xs if x is not None]
    return round(sum(xs) / len(xs), 1) if xs else None


def task_rows(rd):
    runs = jl(os.path.join(rd, "runs.jsonl"))
    by_task = collections.OrderedDict()
    attempts = collections.Counter()
    for r in runs:
        by_task[r["task"]] = r  # latest attempt wins
        attempts[r["task"]] += 1
    rows = []
    for task, r in by_task.items():
        tag = r["tag"]
        dom = task.split("-")[0]
        pi = r.get("pi") or {}
        row = {"task": task, "domain": dom, "tag": tag, "attempts": attempts[task], "infra": r.get("infra"),
               "stop": pi.get("stop"), "wall_s": pi.get("wall_s"), "turns_monitor": pi.get("turns")}
        ev = jload(os.path.join(rd, "eval", tag + ".eval.json")) or {}
        if r.get("infra") == "setup_failed":
            st = jload(os.path.join(rd, "eval", tag + ".setup.json")) or {}
            row["error"] = (st.get("error") or "")[:300]
            rows.append(row)
            continue
        if not ev.get("ok"):
            row["infra"] = row["infra"] or "eval_error"
            row["error"] = (ev.get("error") or "eval missing")[:300]
        row["score"] = ev.get("score")
        row["success"] = bool(ev.get("success"))
        row["final_fail"] = ev.get("final_fail")
        turns = calls = shots = tin = tout = tcache = 0
        tools = collections.Counter()
        err = None
        final = ""
        for e in jl(os.path.join(rd, "pi", tag + ".jsonl")):
            t = e.get("type")
            if t == "message_end":
                m = e.get("message") or {}
                if m.get("role") == "assistant":
                    turns += 1
                    u = m.get("usage") or {}
                    tin += u.get("input") or 0
                    tcache += u.get("cacheRead") or 0
                    tout += u.get("output") or 0
                    for c in m.get("content") or []:
                        if c.get("type") == "toolCall":
                            calls += 1
                            tools[(c.get("name") or "").replace("mcp__gui__", "")] += 1
                    txt = "".join(c.get("text", "") for c in m.get("content") or [] if c.get("type") == "text")
                    if txt.strip():
                        final = txt.strip()
                    if m.get("stopReason") == "error":
                        err = (m.get("errorMessage") or "")[:200]
                elif m.get("role") == "toolResult":
                    shots += sum(1 for c in m.get("content") or [] if c.get("type") == "image")
        row.update(turns=turns, tool_calls=calls, screenshots=shots, tok_in=tin, tok_cache=tcache, tok_out=tout,
                   pi_error=err, final=final[:200], request_human=tools.get("request_human", 0))
        row["tools"] = dict(tools)
        g = gate_stats(os.path.join(rd, "gate", tag + ".jsonl"))
        row["gate"] = g
        rows.append(row)
    return rows


ROLE_NAMES = ["feasibility", "planner", "verifier", "judge", "search", "compact", "grounder"]


def gate_stats(path):
    g = {"actions": 0, "executed": 0, "interventions": 0, "decisions": collections.Counter(), "reviewer": [],
         "reviewer_decisions": collections.Counter(), "roles": {r: {"invocations": 0, "http_calls": 0, "errors": 0, "ms": []} for r in ROLE_NAMES},
         "human_events": 0, "loop_notes": 0, "loop_refused": 0, "events": collections.Counter()}
    prev_ts = None
    for e in jl(path):
        ev = e.get("ev")
        g["events"][ev] += 1
        ts = e.get("ts")
        if ev == "action":
            g["actions"] += 1
            if e.get("executed"):
                g["executed"] += 1
            else:
                g["interventions"] += 1
            g["decisions"][e.get("decision")] += 1
            j = e.get("judge")
            if isinstance(j, dict):
                g["reviewer"].append(j.get("ms"))
                g["reviewer_decisions"][j.get("decision")] += 1
            rg = e.get("risk_gate")
            if isinstance(rg, dict) and isinstance(rg.get("reviewer"), dict):
                g["reviewer"].append(rg["reviewer"].get("ms"))
                g["reviewer_decisions"][rg["reviewer"].get("decision")] += 1
            if e.get("decision") in ("ask_human", "handoff") or e.get("human"):
                g["human_events"] += 1
        elif ev == "request_human":
            g["human_events"] += 1
        elif ev == "loop_breaker":
            g["loop_refused" if e.get("refused") else "loop_notes"] += 1
        elif ev == "briefing":
            for r in ("feasibility", "planner"):
                x = e.get(r)
                if isinstance(x, dict):
                    R = g["roles"][r]
                    R["invocations"] += 1
                    R["http_calls"] += x.get("calls") or 0
                    R["errors"] += 1 if x.get("error") else 0
            if e.get("ms") is not None and (isinstance(e.get("feasibility"), dict) or isinstance(e.get("planner"), dict)):
                g["roles"]["feasibility"]["ms"].append(e.get("ms"))  # feasibility + planner together
        elif ev == "role":
            r = e.get("role")
            if r in g["roles"]:
                R = g["roles"][r]
                R["invocations"] += 1
                R["http_calls"] += e.get("calls") or 1
                R["errors"] += 1 if e.get("error") else 0
                if r not in ("search", "grounder", "verifier") and ts and prev_ts:
                    R["ms"].append(round((ts - prev_ts) * 1000, 1))
        elif ev == "inspect" and e.get("kind") in ("howto", "locate"):
            r = "search" if e.get("kind") == "howto" else "grounder"
            if g["roles"][r]["invocations"] > len(g["roles"][r]["ms"]):
                g["roles"][r]["ms"].append(e.get("ms"))
        elif ev == "check_done":
            if g["roles"]["verifier"]["invocations"] > len(g["roles"]["verifier"]["ms"]):
                g["roles"]["verifier"]["ms"].append(e.get("ms"))
        if ts:
            prev_ts = ts
    return g


def summarize(rows, expected):
    dom_n = collections.Counter()
    S = {"expected": expected, "rows": len(rows)}
    valid = [r for r in rows if not r.get("infra")]
    infra = [r for r in rows if r.get("infra")]
    succ = [r for r in valid if r.get("success")]
    S.update(valid=len(valid), infra=len(infra), success=len(succ),
             score_sum=round(sum((r.get("score") or 0) for r in valid), 2),
             rate_official=round(100 * len(succ) / expected, 2) if expected else None,
             rate_valid=round(100 * len(succ) / len(valid), 2) if valid else None,
             infra_kinds=dict(collections.Counter(r["infra"] for r in infra)))
    doms = collections.OrderedDict()
    for r in rows:
        d = doms.setdefault(r["domain"], {"tasks": 0, "valid": 0, "success": 0, "score_sum": 0.0, "infra": 0})
        d["tasks"] += 1
        if r.get("infra"):
            d["infra"] += 1
        else:
            d["valid"] += 1
            d["success"] += 1 if r.get("success") else 0
            d["score_sum"] += r.get("score") or 0
    S["domains"] = doms
    S["stops"] = dict(collections.Counter(r.get("stop") for r in valid))
    S["turns_mean"] = mean([r.get("turns") for r in valid]); S["turns_median"] = med([r.get("turns") for r in valid])
    S["wall_mean_s"] = mean([r.get("wall_s") for r in valid]); S["wall_median_s"] = med([r.get("wall_s") for r in valid])
    S["wall_total_h"] = round(sum((r.get("wall_s") or 0) for r in rows) / 3600, 2)
    S["tok_in"] = sum(r.get("tok_in") or 0 for r in valid); S["tok_cache"] = sum(r.get("tok_cache") or 0 for r in valid)
    S["tok_out"] = sum(r.get("tok_out") or 0 for r in valid)
    S["screenshots"] = sum(r.get("screenshots") or 0 for r in valid)
    S["tool_calls"] = sum(r.get("tool_calls") or 0 for r in valid)
    tools = collections.Counter()
    for r in valid:
        tools.update(r.get("tools") or {})
    S["tools"] = dict(tools.most_common())
    S["request_human_calls"] = sum(r.get("request_human") or 0 for r in valid)
    S["final_fail_answers"] = sum(1 for r in valid if r.get("final_fail"))
    rv, rvd = [], collections.Counter()
    roles = {k: {"invocations": 0, "http_calls": 0, "errors": 0, "ms": []} for k in ROLE_NAMES}
    G = collections.Counter()
    for r in valid:
        g = r.get("gate") or {}
        rv += g.get("reviewer", [])
        rvd.update(g.get("reviewer_decisions") or {})
        for k in ("actions", "executed", "interventions", "human_events", "loop_notes", "loop_refused"):
            G[k] += g.get(k, 0)
        for k, R in (g.get("roles") or {}).items():
            for f in ("invocations", "http_calls", "errors"):
                roles[k][f] += R[f]
            roles[k]["ms"] += [x for x in R["ms"] if x is not None]
    S["reviewer"] = {"calls": len(rv), "decisions": dict(rvd), "ms_mean": mean(rv), "ms_median": med(rv)}
    S["roles"] = {k: {"invocations": R["invocations"], "http_calls": R["http_calls"], "errors": R["errors"],
                      "ms_mean": mean(R["ms"]), "ms_median": med(R["ms"]), "timed": len(R["ms"]),
                      "runs_using": sum(1 for r in valid if ((r.get("gate") or {}).get("roles") or {}).get(k, {}).get("invocations"))}
                  for k, R in roles.items()}
    S["gate"] = dict(G)
    return S


def md(S, rows):
    L = []
    L.append(f"# OSWorld-Verified report\n")
    wv, wo = wilson(S['success'], S['valid']), wilson(S['success'], S['expected'])
    partial = " PARTIAL run (not every expected task finished)." if S['rows'] < S['expected'] else ""
    L.append(f"> {DISCLAIMER}.{partial} Wilson 95 % CI: valid {100 * wv[0]:.1f}-{100 * wv[1]:.1f} %, "
             f"planned list {100 * wo[0]:.1f}-{100 * wo[1]:.1f} % (task correlation and valid-run selection not included).\n")
    L.append(f"- tasks expected {S['expected']}, finished rows {S['rows']}, valid {S['valid']}, infra {S['infra']} {S['infra_kinds']}")
    L.append(f"- **success {S['success']}/{S['expected']} = {S['rate_official']} %** (official-style: every expected task counts, infra/missing = 0); "
             f"{S['success']}/{S['valid']} = {S['rate_valid']} % of valid runs; score sum {S['score_sum']}")
    L.append(f"- stops {S['stops']}; turns mean {S['turns_mean']} / median {S['turns_median']}; wall mean {S['wall_mean_s']} s / median {S['wall_median_s']} s; total {S['wall_total_h']} h")
    L.append(f"- tokens: input {S['tok_in']:,} + cache read {S['tok_cache']:,}, output {S['tok_out']:,}; screenshots returned {S['screenshots']}; tool calls {S['tool_calls']}")
    L.append(f"- request_human calls {S['request_human_calls']} (human=deny); FAIL answers {S['final_fail_answers']}")
    L.append(f"- gate: {S['gate']}")
    L.append(f"- reviewer (System One): {S['reviewer']}")
    L.append("\n| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |\n|---|---|---|---|---|---|---|")
    for k, R in S["roles"].items():
        L.append(f"| {k} | {R['runs_using']} | {R['invocations']} | {R['http_calls']} | {R['errors']} | {R['ms_mean']} | {R['ms_median']} |")
    L.append("\n(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)")
    L.append("\n| domain | tasks | valid | success | rate (of tasks) | score sum | infra |\n|---|---|---|---|---|---|---|")
    for d, x in S["domains"].items():
        L.append(f"| {d} | {x['tasks']} | {x['valid']} | {x['success']} | {round(100*x['success']/x['tasks'],1)} % | {round(x['score_sum'],2)} | {x['infra']} |")
    L.append(f"\ntools: {S['tools']}")
    bad = [r for r in rows if r.get("infra")]
    if bad:
        L.append("\n## infra rows\n")
        for r in bad:
            L.append(f"- {r['task']}: {r['infra']} (attempts {r['attempts']}) {r.get('error') or r.get('pi_error') or ''}"[:400])
    return "\n".join(L) + "\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("rundir")
    ap.add_argument("--expected", type=int, default=361)
    ap.add_argument("--md"); ap.add_argument("--json"); ap.add_argument("--csv")
    a = ap.parse_args()
    rows = task_rows(a.rundir)
    S = summarize(rows, a.expected)
    text = md(S, rows)
    if a.md:
        open(a.md, "w").write(text)
    if a.json:
        json.dump({"summary": S, "tasks": rows}, open(a.json, "w"), indent=1, default=lambda o: dict(o) if isinstance(o, collections.Counter) else str(o))
    if a.csv:
        keys = ["task", "domain", "attempts", "infra", "score", "success", "final_fail", "stop", "turns", "wall_s", "tool_calls",
                "screenshots", "tok_in", "tok_cache", "tok_out", "request_human", "pi_error", "error", "final"]
        with open(a.csv, "w", newline="") as f:
            w = csv.DictWriter(f, fieldnames=keys + ["gate_actions", "gate_interventions", "reviewer_calls"], extrasaction="ignore")
            w.writeheader()
            for r in rows:
                g = r.get("gate") or {}
                w.writerow({**r, "gate_actions": g.get("actions"), "gate_interventions": g.get("interventions"), "reviewer_calls": len(g.get("reviewer") or [])})
    print(text)


if __name__ == "__main__":
    main()
