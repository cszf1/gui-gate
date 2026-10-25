#!/usr/bin/env python3
"""Context ablation: full context vs the current compression vs a new scheme, on the same gate8 base.

  bash bench/run_bench.sh --interleave abba OUT "ctx_full ctx_current ctx_new" TASKLIST "0 1 2"
  python3 bench/score.py OUT
  python3 bench/context_ablation.py OUT [OUT...] [--facts FACTS.json] [--baseline ctx_full] [--md OUT.md] [--json OUT.json]

A shorter context is not an improvement by itself. Per run this reports what compression can break:
  stale_token_errors   gate action events with stale_error=true (+ pi_stale_mentions: tool results whose text says an
                       element token / snapshot is stale or unknown - the agent acted on an outdated observation)
  repeated_actions     gate retro_repeat events + pi_repeat_calls: a GUI tool call identical (name + arguments) to the
                       previous GUI call with no read in between
  lost_facts           task facts (--facts {task: [{"key", "value"}]}: values the task needs, e.g. a number to copy
                       between windows) that the agent SAW (in the prompt or a tool result) but never USED afterwards
                       (no later tool-call argument and not in the final answer contains the value). A heuristic
                       proxy; facts_reacquired counts how often a value was read again after first sight (re-reading
                       what compression dropped)
  compactions          Pi events whose type mentions compaction + gate role=compact calls
  tok_in / tok_out / turns / wall_s / success
Comparisons are paired by task (mean over repeats) against the baseline, over ALL runs (failures included).

Expected log fields (what the parser reads; a missing field counts as 0 / absent, never as an error):
  runs.jsonl        task, cond (ctx_full|ctx_current|ctx_new or any cond), seed, attempt, tag, context, pi.wall_s
  runs_scored.jsonl success, infra (bench/score.py)
  pi/<tag>.jsonl    Pi --mode json events: type=message_end with message.role assistant (content[].type toolCall:
                    name, arguments; text; usage.input/cacheRead/output) or toolResult (content[].type text/image);
                    the first user message = the prompt; any event type containing "compact"
  gate/<tag>.jsonl  ev=action (tool, executed, stale_error), ev=retro_repeat, ev=role (role=compact, calls), ev=obs
"""
import json
import os
import re
import sys
from collections import Counter, defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import stats  # noqa: E402

STALE = re.compile(r"stale|no longer (exists|valid)|unknown (element|token|snapshot)|snapshot[^\n]{0,40}(expired|outdated|old)", re.I)
READS = {"get_window_state", "inspect", "list_windows", "list_apps"}
NON_GUI = READS | {"set_plan", "check_done", "request_human"}


def _jl(p):
    try:
        with open(p, encoding="utf-8", errors="replace") as f:
            for line in f:
                try:
                    yield json.loads(line)
                except json.JSONDecodeError:
                    pass
    except OSError:
        return


def _text(content):
    if isinstance(content, str):
        return content
    return "".join(c.get("text", "") for c in content or [] if isinstance(c, dict) and c.get("type") == "text")


def parse_pi(path, facts=()):
    r = Counter()
    last_gui = None
    seen, used, reacq = {}, set(), Counter()
    order = 0
    final = ""
    for e in _jl(path):
        t = e.get("type") or ""
        if "compact" in t.lower():
            r["compactions"] += 1
        if t != "message_end":
            continue
        m = e.get("message") or {}
        role = m.get("role")
        order += 1
        if role == "user":
            txt = _text(m.get("content"))
            for f in facts:
                if f["value"] in txt and f["key"] not in seen:
                    seen[f["key"]] = order
        elif role == "assistant":
            r["turns"] += 1
            u = m.get("usage") or {}
            r["tok_in"] += (u.get("input") or 0) + (u.get("cacheRead") or 0)
            r["tok_out"] += u.get("output") or 0
            for c in m.get("content") or []:
                if c.get("type") == "toolCall":
                    name = (c.get("name") or "").replace("mcp__gui__", "")
                    args = json.dumps(c.get("arguments") or {}, sort_keys=True)
                    if name in READS:
                        last_gui = None
                    elif name not in NON_GUI:
                        if last_gui == (name, args):
                            r["pi_repeat_calls"] += 1
                        last_gui = (name, args)
                    for f in facts:
                        if f["key"] in seen and f["value"] in args:
                            used.add(f["key"])
            txt = _text(m.get("content"))
            if txt.strip():
                final = txt
        elif role == "toolResult":
            txt = _text(m.get("content"))
            if STALE.search(txt):
                r["pi_stale_mentions"] += 1
            for f in facts:
                if f["value"] in txt:
                    if f["key"] in seen:
                        reacq[f["key"]] += 1
                    else:
                        seen[f["key"]] = order
    for f in facts:
        if f["key"] in seen and f["value"] in final:
            used.add(f["key"])
    r["facts_total"] = len(facts)
    r["facts_seen"] = len(seen)
    r["lost_facts"] = len(set(seen) - used)
    r["facts_reacquired"] = sum(reacq.values())
    return dict(r)


def parse_gate(path):
    r = Counter()
    for e in _jl(path):
        ev = e.get("ev")
        if ev == "action" and e.get("stale_error"):
            r["stale_token_errors"] += 1
        elif ev == "retro_repeat":
            r["gate_repeats"] += 1
        elif ev == "role" and e.get("role") == "compact":
            r["gate_compact_calls"] += e.get("calls") or 1
    return dict(r)


def load(dirs, facts=None):
    facts = facts or {}
    best = {}
    scored = {}
    for d in dirs:
        for r in _jl(os.path.join(d, "runs.jsonl")):
            k = (r["task"], r["cond"], r.get("seed", 0))
            if k not in best or (r.get("attempt") or 1) >= (best[k][1].get("attempt") or 1):
                best[k] = (d, r)
        for r in _jl(os.path.join(d, "runs_scored.jsonl")):
            scored[(r["task"], r["cond"], r.get("seed", 0))] = r
    rows = []
    for (t, c, s), (d, r) in sorted(best.items()):
        x = {"task": t, "cond": c, "seed": s, "context": r.get("context") or c, "infra": r.get("infra"),
             "wall_s": (r.get("pi") or {}).get("wall_s")}
        x.update(parse_pi(os.path.join(d, "pi", r["tag"] + ".jsonl"), facts.get(t, [])))
        x.update(parse_gate(os.path.join(d, "gate", r["tag"] + ".jsonl")))
        sc = scored.get((t, c, s)) or {}
        x["success"] = bool(sc.get("success")) and not (sc.get("infra") or r.get("infra"))
        x["repeated_actions"] = x.get("gate_repeats", 0) + x.get("pi_repeat_calls", 0)
        x["compactions"] = x.get("compactions", 0) + x.get("gate_compact_calls", 0)
        rows.append(x)
    return rows


KEYS = ("success", "stale_token_errors", "pi_stale_mentions", "repeated_actions", "lost_facts", "facts_reacquired",
        "compactions", "tok_in", "tok_out", "turns", "wall_s")


def summarize(rows, baseline="ctx_full"):
    conds = sorted({r["cond"] for r in rows})
    tasks = sorted({r["task"] for r in rows})
    res = {"conditions": {}, "paired": {}}
    for c in conds:
        rs = [r for r in rows if r["cond"] == c]
        agg = {"runs": len(rs), "infra": sum(1 for r in rs if r["infra"])}
        for k in KEYS:
            vs = [float(r.get(k) or 0) for r in rs if not (k == "wall_s" and r.get(k) is None)]
            agg["mean_" + k] = sum(vs) / len(vs) if vs else None
        agg["facts_seen"] = sum(r.get("facts_seen", 0) for r in rs)
        agg["facts_lost"] = sum(r.get("lost_facts", 0) for r in rs)
        res["conditions"][c] = agg
    if baseline in conds:
        def per_task(c, k):
            out = []
            for t in tasks:
                vs = [float(r.get(k) or 0) for r in rows if r["cond"] == c and r["task"] == t]
                out.append(sum(vs) / len(vs) if vs else 0.0)
            return out
        for c in conds:
            if c == baseline:
                continue
            res["paired"][c] = {k: stats.paired_bootstrap(per_task(baseline, k), per_task(c, k), seed=11)
                                for k in ("success", "stale_token_errors", "repeated_actions", "lost_facts", "tok_in", "wall_s")}
            res["paired"][c]["vs"] = baseline
    return res


def to_md(res):
    L = ["# Context ablation (same gate8 base; only the Pi context handling differs)", "",
         "| condition | runs | infra | success | stale-token errors | stale mentions | repeated actions | lost facts (of seen) | "
         "re-reads of facts | compactions | tok in | tok out | turns | wall s |", "|" + "---|" * 14]
    for c, a in res["conditions"].items():
        def f(k, nd=2):
            v = a.get("mean_" + k)
            return "-" if v is None else f"{v:.{nd}f}"
        L.append(f"| {c} | {a['runs']} | {a['infra']} | {f('success')} | {f('stale_token_errors')} | {f('pi_stale_mentions')} | "
                 f"{f('repeated_actions')} | {a['facts_lost']}/{a['facts_seen']} | {f('facts_reacquired')} | {f('compactions')} | "
                 f"{f('tok_in', 0)} | {f('tok_out', 0)} | {f('turns', 1)} | {f('wall_s', 1)} |")
    for c, d in res["paired"].items():
        L += ["", f"## {c} vs {d['vs']} (paired by task, all runs)", "", "| metric | diff | 95% CI |", "|---|---|---|"]
        for k, b in d.items():
            if k == "vs":
                continue
            L.append(f"| {k} | {b['diff']:+.3f} | {b['ci'][0]:+.3f}..{b['ci'][1]:+.3f} |")
    L += ["", "Shorter context alone is not an improvement: compare success, stale-token errors, repeats and lost facts first."]
    return "\n".join(L) + "\n"


def main(a):
    flags, dirs, i = {}, [], 0
    while i < len(a):
        if a[i].startswith("--"):
            flags[a[i]] = a[i + 1]
            i += 2
        else:
            dirs.append(a[i])
            i += 1
    facts = json.load(open(flags["--facts"])) if "--facts" in flags else None
    rows = load(dirs, facts)
    res = summarize(rows, flags.get("--baseline", "ctx_full"))
    res["runs"] = rows
    md = to_md(res)
    if "--md" in flags:
        open(flags["--md"], "w").write(md)
    if "--json" in flags:
        json.dump(res, open(flags["--json"], "w"), indent=1)
    print(md)
    return res


if __name__ == "__main__":
    main(sys.argv[1:])
