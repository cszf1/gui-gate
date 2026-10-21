#!/usr/bin/env python3
"""Speed + gate statistics for a bench/run_bench.sh output dir (complements score.py).

  python bench/timing.py OUTDIR [--md FILE] [--json FILE]

Per valid run (infra rows excluded, as in score.py):
  model_s/turn   turn_start (request sent) -> assistant message_end, from pi_monitor's .times sidecar (arrival times);
                 runs without a sidecar fall back to (gap between assistant request timestamps - tool time)
  tool_s/turn    tool_execution_start -> tool_execution_end
  reviewer       gate log: every reviewer consultation (step review `judge` or risk review `risk_gate.reviewer`),
                 its latency (`ms`) and decision (allow / deny / no answer)
  interventions  gate actions not executed as sent (decision != direct), by decision
  asked_person   gate request_human events + the agent's own request_human tool calls (--human deny: all refused)
"""
import json
import os
import sys
from collections import Counter, defaultdict


def jl(p):
    try:
        with open(p, encoding="utf-8", errors="replace") as f:
            for line in f:
                try:
                    yield json.loads(line)
                except json.JSONDecodeError:
                    pass
    except OSError:
        return


def model_times(out, tag):
    tp = os.path.join(out, "pi", tag + ".jsonl.times")
    lat, tools = [], []
    if os.path.exists(tp):
        st = {}
        tst = []
        for e in jl(tp):
            # Pi emits the assistant message_start only with the first streamed token, so the request starts at
            # turn_start (time to first token is part of the model's latency)
            if e["type"] == "turn_start":
                st["a"] = e["t"]
            elif e["type"] == "message_start" and e.get("role") == "assistant":
                st.setdefault("a", e["t"])
                st["ttft"] = e["t"] - st["a"]
            elif e["type"] == "message_end" and e.get("role") == "assistant" and "a" in st:
                lat.append(e["t"] - st.pop("a"))
            elif e["type"] == "tool_execution_start":
                tst.append(e["t"])
            elif e["type"] == "tool_execution_end" and tst:
                tools.append(e["t"] - tst.pop(0))
        return lat, tools, "sidecar"
    # fallback: request timestamps of assistant messages; tool time unknown per call -> gap only
    ts = []
    for e in jl(os.path.join(out, "pi", tag + ".jsonl")):
        if e.get("type") == "message_start" and (e.get("message") or {}).get("role") == "assistant":
            ts.append(e["message"].get("timestamp", 0) / 1000)
    gaps = [b - a for a, b in zip(ts, ts[1:])]
    return gaps, [], "gap"


def gate_stats(out, tag):
    g = Counter()
    rv_ms = []
    dec = Counter()
    for e in jl(os.path.join(out, "gate", tag + ".jsonl")):
        ev = e.get("ev")
        if ev == "action":
            g["actions"] += 1
            d = e.get("decision") or "?"
            if not e.get("executed"):
                g["interventions"] += 1
                dec[d] += 1
            judges = []
            if isinstance(e.get("judge"), dict):
                judges.append(e["judge"])
            rg = e.get("risk_gate")
            if isinstance(rg, dict) and "reviewer" in rg:
                judges.append(rg["reviewer"] if isinstance(rg["reviewer"], dict) else {"error": "no answer"})
            for j in judges:
                g["reviews"] += 1
                if j.get("ms") is not None:
                    rv_ms.append(float(j["ms"]))
                dd = j.get("decision")
                if dd == "allow":
                    g["review_allow"] += 1
                elif dd in ("deny", "block", "refuse"):
                    g["review_deny"] += 1
                elif dd:
                    g["review_" + str(dd)] += 1
                else:
                    g["review_no_answer"] += 1
            if d == "deny":
                g["denials"] += 1
            if d in ("ask_human", "ask_help", "handoff"):
                g["asked_person_gate"] += 1  # with --human deny nobody answers: the step is refused
        elif ev == "request_human":
            g["asked_person_gate"] += 1
    return g, rv_ms, dec


def main():
    out = sys.argv[1]
    md_path = sys.argv[sys.argv.index("--md") + 1] if "--md" in sys.argv else os.path.join(out, "timing.md")
    js_path = sys.argv[sys.argv.index("--json") + 1] if "--json" in sys.argv else os.path.join(out, "timing.json")
    scored = {r["tag"]: r for r in jl(os.path.join(out, "runs_scored.jsonl"))}
    raw = list(jl(os.path.join(out, "runs.jsonl")))
    per = []
    for r in raw:
        tag = r["tag"]
        s = scored.get(tag, {})
        lat, tools, src = model_times(out, tag)
        g, rv_ms, dec = gate_stats(out, tag)
        agent_rh = 0
        for e in jl(os.path.join(out, "pi", tag + ".jsonl")):
            if e.get("type") == "tool_execution_start" and (e.get("toolName") or "").endswith("request_human"):
                agent_rh += 1
        per.append({"tag": tag, "task": r["task"], "cond": r["cond"], "infra": s.get("infra", r.get("infra")),
                    "model": r.get("model"), "suite": "osguard" if r["task"].startswith("og") else "osworld",
                    "success": s.get("success"), "safe_success": s.get("safe_success"),
                    "unsafe": s.get("unsafe_completion"), "stop": (r.get("pi") or {}).get("stop"),
                    "wall_s": (r.get("pi") or {}).get("wall_s"), "turns": s.get("turns"),
                    "model_s": lat, "tool_s": tools, "lat_src": src, "gate": dict(g), "review_ms": rv_ms,
                    "interv_by_decision": dict(dec), "agent_request_human": agent_rh})
    json.dump(per, open(js_path, "w"), indent=1)

    def mean(v):
        return round(sum(v) / len(v), 2) if v else None

    def pct(v, q):
        v = sorted(v)
        return round(v[min(len(v) - 1, int(q * len(v)))], 1) if v else None

    md = ["# Timing and gate statistics (bench/timing.py)", "",
          "Valid runs only (relay/infra rows excluded). model s/turn = one LLM request (request sent -> full response).", "",
          "| cond | valid runs | infra excl. | mean wall s/task | mean turns | model s/turn mean | p50 | p90 | tool s/call | "
          "reviewer calls | reviewer ms mean | allow | deny | no answer | gate actions | interventions | rate | asked person (gate+agent) | stop=timeout |",
          "|" + "---|" * 19]
    for c in sorted({p["cond"] for p in per}):
        rs = [p for p in per if p["cond"] == c]
        v = [p for p in rs if not p["infra"]]
        lat = [x for p in v for x in p["model_s"]]
        tl = [x for p in v for x in p["tool_s"]]
        G = Counter()
        for p in v:
            G.update(p["gate"])
        rms = [x for p in v for x in p["review_ms"]]
        asked = G["asked_person_gate"] + sum(p["agent_request_human"] for p in v)
        rate = round(G["interventions"] / G["actions"], 3) if G["actions"] else None
        md.append(f"| {c} | {len(v)} | {len(rs) - len(v)} | {mean([p['wall_s'] or 0 for p in v])} | "
                  f"{mean([p['turns'] or 0 for p in v])} | {mean(lat)} | {pct(lat, .5)} | {pct(lat, .9)} | {mean(tl)} | "
                  f"{G['reviews']} | {mean(rms)} | {G['review_allow']} | {G['review_deny']} | {G['review_no_answer']} | "
                  f"{G['actions']} | {G['interventions']} | {rate} | {G['asked_person_gate']}+{sum(p['agent_request_human'] for p in v)} | "
                  f"{sum(1 for p in v if p['stop'] == 'timeout')} |")
    md += ["", "Interventions by decision: " + "; ".join(
        f"{c}: " + json.dumps(dict(sum((Counter(p['interv_by_decision']) for p in per if p['cond'] == c and not p['infra']), Counter())))
        for c in sorted({p["cond"] for p in per})), "",
        "## Per run", "", "| task | cond | ok | safe | stop | wall s | turns | model s/turn | reviews | interv. | infra |",
        "|---|---|---|---|---|---|---|---|---|---|---|"]
    for p in sorted(per, key=lambda p: (p["task"], p["cond"], p["tag"])):
        safe = "" if p["suite"] != "osguard" else ("UNSAFE" if p["unsafe"] else "safe")
        md.append(f"| {p['task']} | {p['cond']} | {int(bool(p['success']))} | {safe} | {p['stop']} | {p['wall_s']} | {p['turns']} | "
                  f"{mean(p['model_s'])} | {p['gate'].get('reviews', 0)} | {p['gate'].get('interventions', 0)} | {p['infra'] or ''} |")
    open(md_path, "w").write("\n".join(md) + "\n")
    print("\n".join(md[:12]))


if __name__ == "__main__":
    main()
