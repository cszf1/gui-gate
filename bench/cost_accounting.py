#!/usr/bin/env python3
"""Full cost accounting per run and per policy (failures and timeouts included).

  cost_accounting.py RUN_DIR [RUN_DIR...] [--prices PRICES.json] [--md OUT.md] [--json OUT.json] [--baseline COND]

Per run (run_costs): from the Pi event stream (pi/<tag>.jsonl) and the gate log (gate/<tag>.jsonl):
  pi_turns            assistant messages of the acting model (100 Pi turns != 100 environment actions)
  pi_tool_calls       tool calls the acting model made
  gui_actions         actions the gate actually EXECUTED on the desktop (ev=action, executed=true, a GUI tool;
                      batch sub-steps count one by one; blocked / intervened actions do not)
  gui_actions_blocked actions the gate did not execute (interventions, denials)
  commands            run_command executions (ev=command)
  observations        reads (ev=obs; auto = the gate's own auto-observe)
  screenshots_model   images the acting model received (toolResult image parts in the Pi stream)
  screenshots_gate    screenshots the gate captured (ev=obs with screenshot=true, plus ev=img images)
  role_calls          model-role calls inside the gate by role (ev=role: verifier, compact, judge, search, grounder,
                      planner, ...; `calls` field, default 1)
  reviewer_calls      the gate's independent reviewer model (actions with a non-null `judge`)
  tok_in / tok_out    acting-model tokens (input + cacheRead / output) from Pi usage
  role_tok_in/out     role + reviewer tokens IF the gate logged them (fields tok_in / tok_out on ev=role or inside
                      `judge`); null when the gate did not log them - reported as "not logged", never as 0
  cost_usd            Pi usage.cost.total summed, + role tokens priced with --prices when given; null if unknown
  wall_s              Pi wall time (pi_monitor; timeouts are cut at the timeout and still counted)
  run_wall_s          whole run incl. setup and evaluation (run_bench.sh >= this version)
Time comparisons (summarize) use EVERY planned run: success-only speed comparisons are survivor-biased. A missing
run's time is imputed at the cap (PI_TIMEOUT, default 480 s) and counted in `imputed`.
"""
import json
import os
import sys
from collections import Counter, defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import stats  # noqa: E402

GUI_TOOLS = {"click", "double_click", "right_click", "type_text", "set_value", "press_key", "hotkey", "scroll", "drag",
             "select", "move", "key", "focus_window", "launch_app", "open"}
NON_GUI = {"run_actions", "run_steps", "run_command", "inspect", "check_done", "set_plan", "request_human",
           "get_window_state", "list_windows", "list_apps"}


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


def pi_costs(path):
    c = Counter()
    cost = 0.0
    cost_seen = False
    for e in _jl(path):
        if e.get("type") != "message_end":
            continue
        m = e.get("message") or {}
        if m.get("role") == "assistant":
            c["pi_turns"] += 1
            u = m.get("usage") or {}
            c["tok_in"] += (u.get("input") or 0) + (u.get("cacheRead") or 0)
            c["tok_out"] += u.get("output") or 0
            uc = u.get("cost")
            if isinstance(uc, dict) and uc.get("total") is not None:
                cost += uc["total"] or 0.0
                cost_seen = True
            c["pi_tool_calls"] += sum(1 for x in m.get("content") or [] if x.get("type") == "toolCall")
            if m.get("stopReason") == "error":
                c["api_errors"] += 1
        elif m.get("role") == "toolResult":
            c["screenshots_model"] += sum(1 for x in m.get("content") or [] if x.get("type") == "image")
    out = dict(c)
    out["pi_cost_usd"] = cost if cost_seen else None
    return out


def gate_costs(path):
    c = Counter()
    roles = Counter()
    rtin = rtout = 0
    rtok_seen = False
    for e in _jl(path):
        ev = e.get("ev")
        if ev == "action":
            tool = e.get("tool") or ""
            if tool not in NON_GUI:
                if e.get("executed"):
                    c["gui_actions"] += 1
                else:
                    c["gui_actions_blocked"] += 1
            j = e.get("judge")
            if j not in (None, False, "", {}):
                c["reviewer_calls"] += 1
                if isinstance(j, dict) and (j.get("tok_in") is not None or j.get("tok_out") is not None):
                    rtin += j.get("tok_in") or 0
                    rtout += j.get("tok_out") or 0
                    rtok_seen = True
            if e.get("stale_error"):
                c["stale_errors"] += 1
        elif ev == "command":
            c["commands"] += 1
            if e.get("timed_out"):
                c["command_timeouts"] += 1
        elif ev == "obs":
            c["observations"] += 1
            if e.get("auto"):
                c["auto_observations"] += 1
            if e.get("screenshot"):
                c["screenshots_gate"] += 1
        elif ev == "img":
            c["screenshots_gate"] += e.get("images") or 0
        elif ev == "role":
            n = e.get("calls")
            n = 1 if n is None else n
            roles[e.get("role") or "?"] += n
            if e.get("tok_in") is not None or e.get("tok_out") is not None:
                rtin += e.get("tok_in") or 0
                rtout += e.get("tok_out") or 0
                rtok_seen = True
        elif ev == "retro_repeat":
            c["repeats"] += 1
        elif ev in ("request_human", "human_outcome"):
            c["human_" + ev] += 1
    out = dict(c)
    out["role_calls"] = dict(roles)
    out["role_calls_total"] = sum(roles.values())
    out["role_tok_in"] = rtin if rtok_seen else None
    out["role_tok_out"] = rtout if rtok_seen else None
    return out


def run_costs(out_dir, tag, run_row=None, prices=None):
    r = {"tag": tag}
    r.update(pi_costs(os.path.join(out_dir, "pi", tag + ".jsonl")))
    r.update(gate_costs(os.path.join(out_dir, "gate", tag + ".jsonl")))
    for k in ("pi_turns", "pi_tool_calls", "gui_actions", "gui_actions_blocked", "commands", "observations",
              "screenshots_model", "screenshots_gate", "reviewer_calls", "tok_in", "tok_out", "stale_errors", "repeats"):
        r.setdefault(k, 0)
    rr = run_row or {}
    pi = rr.get("pi") or {}
    r["wall_s"] = pi.get("wall_s")
    r["run_wall_s"] = rr.get("run_wall_s")
    r["stop"] = pi.get("stop")
    r["actions_per_turn"] = round(r["gui_actions"] / r["pi_turns"], 3) if r["pi_turns"] else None
    cost = r.get("pi_cost_usd")
    if prices and r.get("role_tok_in") is not None:
        cost = (cost or 0.0) + r["role_tok_in"] / 1e6 * prices.get("role_in_per_mtok", 0) + \
            (r["role_tok_out"] or 0) / 1e6 * prices.get("role_out_per_mtok", 0)
    if prices and not r.get("pi_cost_usd"):
        cost = (cost or 0.0) + r["tok_in"] / 1e6 * prices.get("in_per_mtok", 0) + r["tok_out"] / 1e6 * prices.get("out_per_mtok", 0)
    r["cost_usd"] = cost
    return r


def load_runs(dirs):
    best = {}
    for d in dirs:
        rows = list(_jl(os.path.join(d, "runs.jsonl")))
        for r in rows:
            k = (r["task"], r["cond"], r.get("seed", 0))
            if k not in best or (r.get("attempt") or 1) >= (best[k][1].get("attempt") or 1):
                best[k] = (d, r)
    return best


def summarize(best, prices=None, cap_s=480.0, baseline=None, scored=None):
    """best: {(task, cond, seed): (dir, runs.jsonl row)}. scored: optional {(task, cond, seed): success}."""
    per = defaultdict(list)
    rows = []
    for (t, c, s), (d, r) in sorted(best.items()):
        x = run_costs(d, r["tag"], r, prices)
        x.update(task=t, cond=c, seed=s, infra=r.get("infra"))
        if scored is not None:
            x["success"] = bool(scored.get((t, c, s)))
        rows.append(x)
        per[c].append(x)
    tasks = sorted({k[0] for k in best})
    seeds = sorted({k[2] for k in best})
    res = {"policies": {}, "paired_time": {}, "cap_s": cap_s}
    for c, xs in per.items():
        n_plan = len(tasks) * len(seeds)
        walls = [x["wall_s"] if x["wall_s"] is not None else cap_s for x in xs]
        imputed = sum(1 for x in xs if x["wall_s"] is None) + (n_plan - len(xs))
        walls += [cap_s] * (n_plan - len(xs))
        agg = {"planned_runs": n_plan, "present_runs": len(xs), "imputed_wall": imputed,
               "mean_wall_s_all_runs": sum(walls) / len(walls) if walls else None,
               "median_wall_s_all_runs": stats._quantile(walls, 0.5) if walls else None,
               "timeouts": sum(1 for x in xs if x.get("stop") in ("timeout", "max_turns")),
               "infra": dict(Counter(str(x["infra"]) for x in xs if x["infra"]))}
        for k in ("pi_turns", "pi_tool_calls", "gui_actions", "gui_actions_blocked", "commands", "observations",
                  "screenshots_model", "screenshots_gate", "reviewer_calls", "role_calls_total", "tok_in", "tok_out"):
            agg["mean_" + k] = sum(x.get(k) or 0 for x in xs) / len(xs) if xs else None
        rc = Counter()
        for x in xs:
            rc.update(x.get("role_calls") or {})
        agg["role_calls_by_role"] = dict(rc)
        rt = [x["role_tok_in"] for x in xs if x.get("role_tok_in") is not None]
        agg["role_tokens"] = "not logged by the gate" if not rt else sum(rt) / len(rt)
        costs = [x["cost_usd"] for x in xs if x.get("cost_usd") is not None]
        agg["mean_cost_usd"] = sum(costs) / len(costs) if costs else None
        rw = [x["run_wall_s"] for x in xs if x.get("run_wall_s") is not None]
        agg["mean_run_wall_s"] = sum(rw) / len(rw) if rw else None
        if scored is not None:
            ok = [x["wall_s"] for x in xs if x.get("success") and x["wall_s"] is not None]
            agg["mean_wall_s_successes_only_DESCRIPTIVE"] = sum(ok) / len(ok) if ok else None
        res["policies"][c] = agg
    if baseline and baseline in per:
        look = {(x["task"], x["cond"], x["seed"]): (x["wall_s"] if x["wall_s"] is not None else cap_s) for x in rows}

        def per_task(c):
            return [sum(look.get((t, c, s), cap_s) for s in seeds) / len(seeds) for t in tasks]
        for c in per:
            if c != baseline:
                res["paired_time"][c] = {"vs": baseline, "tasks": len(tasks),
                                         "wall_diff_bootstrap": stats.paired_bootstrap(per_task(baseline), per_task(c), seed=5)}
    res["runs"] = rows
    return res


def to_md(res):
    L = ["# Cost accounting (every planned run; failures, timeouts and infra runs included)", "",
         "| policy | planned | present | mean wall s (all) | median wall s | Pi turns | GUI actions | blocked | commands | "
         "model shots | gate shots | reviewer calls | role calls | tok in | tok out | cost $ | role tokens |",
         "|" + "---|" * 17]
    for c, a in sorted(res["policies"].items()):
        def f(k, nd=1):
            v = a.get(k)
            return "-" if v is None else (f"{v:.{nd}f}" if isinstance(v, float) else str(v))
        L.append(f"| {c} | {a['planned_runs']} | {a['present_runs']} | {f('mean_wall_s_all_runs')} | {f('median_wall_s_all_runs')} | "
                 f"{f('mean_pi_turns')} | {f('mean_gui_actions')} | {f('mean_gui_actions_blocked')} | {f('mean_commands')} | "
                 f"{f('mean_screenshots_model')} | {f('mean_screenshots_gate')} | {f('mean_reviewer_calls')} | {f('mean_role_calls_total')} | "
                 f"{f('mean_tok_in', 0)} | {f('mean_tok_out', 0)} | {f('mean_cost_usd', 4)} | {a['role_tokens'] if isinstance(a['role_tokens'], str) else round(a['role_tokens'])} |")
    if res["paired_time"]:
        L += ["", "## Paired wall time over ALL tasks (task means over repeats; missing = cap)", "", "| policy | vs | tasks | diff s | 95% CI |", "|---|---|---|---|---|"]
        for c, d in res["paired_time"].items():
            b = d["wall_diff_bootstrap"]
            L.append(f"| {c} | {d['vs']} | {d['tasks']} | {b['diff']:+.1f} | {b['ci'][0]:+.1f}..{b['ci'][1]:+.1f} |")
    L += ["", "Pi turns are model turns, not environment actions; GUI actions are what the gate executed on the desktop.",
          "Speed is compared over all runs; success-only times are descriptive and survivor-biased."]
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
    prices = json.load(open(flags["--prices"])) if "--prices" in flags else None
    best = load_runs(dirs)
    scored = {}
    for d in dirs:
        for r in _jl(os.path.join(d, "runs_scored.jsonl")):
            scored[(r["task"], r["cond"], r.get("seed", 0))] = bool(r.get("success")) and not r.get("infra")
    res = summarize(best, prices, float(flags.get("--cap", os.environ.get("PI_TIMEOUT", 480))), flags.get("--baseline"),
                    scored or None)
    md = to_md(res)
    if "--md" in flags:
        open(flags["--md"], "w").write(md)
    if "--json" in flags:
        json.dump(res, open(flags["--json"], "w"), indent=1)
    print(md)
    return res


if __name__ == "__main__":
    main(sys.argv[1:])
