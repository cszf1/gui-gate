#!/usr/bin/env python3
"""Offline evaluation of the value model v2 on one finished benchmark run (stdlib only, deterministic with a seed).

    python3 bench/offline_v2.py RESULTS_DIR -o OUT_DIR [--attempt first_valid|first] [--reps 200] [--seed 0]
                                [--bin target/release/gui-gate] [--stage all|audit|predictors|costs|replay|treatment]

Reads RESULTS_DIR (runs.jsonl, runs_scored.jsonl, retry/*.jsonl, setup/task_list_ovf.txt, gate/*.jsonl, pi/*.jsonl) and
never writes there. Writes OUT_DIR/report.json, report.md, exclusions.json (and the stage pieces).

What is excluded, and why (every exclusion is listed in exclusions.json):
* corrupt lines of any run file (a truncated JSON row): logged with file, line number and the tag it started with;
* tasks without a result row (e.g. the only row was the truncated one) and tasks whose selected attempt is infra
  (setup_failed / relay_error / eval_error): no task label exists for them;
* repeated attempts: per task ONE attempt is used - `first_valid` (default) is the first attempt that is not infra
  (the harness retried only infra failures, so this is the first real attempt and matches the report's valid
  denominator); `first` uses attempt 1 only and drops the task when attempt 1 was infra;
* later sessions inside one gate log (an aborted re-run appended to the file): only the file's first session is used.

Commands (run_command) and GUI actions are kept apart everywhere: a command has no window observation, so the GUI
predictor's no_obs weight scores it as risky; averaging both hides that. Calibration is reported for GUI, command and
(for comparison with older reports) all rows.

What the logs can and cannot say:
* "predicted the execution failure" (a step that ran and errored / was redone was flagged beforehand) IS measured;
* "the intervention avoided the failure" is NOT estimable: an intervened step was not executed as proposed, so its
  outcome under `direct` is unknown, and the next step's outcome mixes the intervention with the agent's own change.
  That needs paired counterfactual branches (bench/counterfactual) or a randomized online comparison
  (bench/online_plan.py). This script never reports it.
"""
import argparse
import bisect
import json
import math
import os
import random
import re
import subprocess
import sys

V1_FEATURES = ["bias", "is_pixel", "is_token", "is_name", "is_keyboard", "no_obs", "token_stale", "log_obs_age", "acts_since_obs",
               "obs_had_shot", "last_err", "last_unverif", "last_popup", "repeat", "edge_close", "small_target", "label_empty",
               "label_dup", "loading_cue", "window_changed", "window_added", "elem_unknown", "check_pass", "check_fail",
               "prev_none", "sess_fail", "consec_fail", "sig_failed", "role_menu", "role_cell", "role_toggle"]
# features::V2_FEATURES (src/features.rs): same names, same definitions
V2_EXTRA = ["tool_click", "tool_scroll", "tool_type", "tool_key", "tool_set_value", "scroll_pixel",
            "role_button", "role_text", "role_link_tab", "role_list_item", "role_combo"]
V2_FEATURES = V1_FEATURES + V2_EXTRA
CMD_FEATURES = ["bias", "cmd_grep", "cmd_test", "cmd_find", "cmd_ls", "cmd_cat", "cmd_python", "cmd_pipe", "cmd_chain",
                "cmd_redirect", "cmd_len", "prev_cmd_fail", "cmd_repeat"]
VALUE_OPTIONS = ["direct", "check", "heal", "wait", "look_light", "look_full", "review", "reason", "ask_help"]
NO_ANSWER = {"timeout", "expired", "cancelled", "declined", "window_closed", "no_response", "no_channel", "not_asked_again"}
NAV = re.compile(r"^(up|down|left|right|tab|page_?up|page_?down|pgup|pgdn|home|end|arrow_?(up|down|left|right))$")


# ------------------------------------------------------------------ run files

def _lines(path):
    with open(path, encoding="utf-8", errors="replace") as fh:
        yield from fh


def _load_json(path):
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def _dump_json(obj, path, indent=None):
    with open(path, "w", encoding="utf-8") as fh:
        json.dump(obj, fh, indent=indent)

def read_jsonl_robust(path):
    """(rows, bad): bad = [{line, bytes, prefix, tag}] for lines that are not valid JSON objects."""
    rows, bad = [], []
    with open(path, encoding="utf-8", errors="replace") as fh:
        for i, line in enumerate(fh, 1):
            if not line.strip():
                continue
            try:
                r = json.loads(line)
                if not isinstance(r, dict):
                    raise ValueError("not an object")
                rows.append(r)
            except ValueError:
                m = re.search(r'"tag"\s*:\s*"([^"]+)"', line)
                bad.append({"line": i, "bytes": len(line.encode()), "prefix": line[:120].rstrip("\n"), "tag": m.group(1) if m else None})
    return rows, bad


def expected_tasks(results_dir, scored):
    tl = os.path.join(results_dir, "setup", "task_list_ovf.txt")
    if os.path.exists(tl):
        out = []
        for line in _lines(tl):
            line = line.strip()
            if line:
                out.append(os.path.splitext(os.path.basename(line))[0] if line.endswith(".json") else line)
        return out
    return sorted({r["task"] for r in scored})


def audit_runs(results_dir, attempt_policy="first_valid"):
    """Which (task, attempt) rows the offline analysis uses, and every exclusion with its reason."""
    files = {}
    corrupt = []
    side_rows = []  # parsable rows of the other run files: earlier attempts the final files no longer keep
    for rel in ["runs.jsonl", "runs_scored.jsonl"] + sorted(os.path.join("retry", f) for f in (os.listdir(os.path.join(results_dir, "retry"))
                                                                                                 if os.path.isdir(os.path.join(results_dir, "retry")) else [])
                                                                                                 if f.endswith(".jsonl")):
        p = os.path.join(results_dir, rel)
        if not os.path.exists(p):
            continue
        rows, bad = read_jsonl_robust(p)
        files[rel] = {"rows": len(rows), "corrupt": len(bad)}
        corrupt += [dict(b, file=rel) for b in bad]
        if rel != "runs_scored.jsonl":
            side_rows += rows
    scored_path = os.path.join(results_dir, "runs_scored.jsonl")
    scored, _ = read_jsonl_robust(scored_path) if os.path.exists(scored_path) else ([], [])
    by_task = {}
    seen = set()
    for r in scored:
        by_task.setdefault(r["task"], []).append(r)
        seen.add((r["task"], int(r.get("attempt") or 1)))
    # an attempt only present in runs.jsonl / retry/*.jsonl (the final scored file keeps the last attempt of a task):
    # it is an earlier, infra-failed attempt; it is never used, but it decides `first` and is listed as excluded
    for r in side_rows:
        k = (r.get("task"), int(r.get("attempt") or 1))
        if k[0] and k not in seen and r.get("infra"):
            seen.add(k)
            by_task.setdefault(k[0], []).append(dict(r, _side=True))
    truncated_tags = {b["tag"] for b in corrupt if b["tag"]}
    tasks = {}
    for t in expected_tasks(results_dir, scored):
        rows = sorted(by_task.get(t, []), key=lambda r: int(r.get("attempt") or 1))
        if not rows:
            trunc = any(tag.startswith(t + "-") for tag in truncated_tags)
            tasks[t] = {"status": "excluded", "reason": "no_result_row (runs.jsonl line truncated)" if trunc else "no_result_row"}
            continue
        skipped = []
        chosen = None
        for r in rows:
            if attempt_policy == "first":
                chosen = r
                break
            if r.get("infra"):
                skipped.append({"attempt": int(r.get("attempt") or 1), "reason": f"infra:{r['infra']}"})
                continue
            chosen = r
            break
        if chosen is None:
            last = rows[-1]
            tasks[t] = {"status": "excluded", "reason": f"infra:{last.get('infra')}", "attempts": len(rows)}
            continue
        if chosen.get("infra"):
            tasks[t] = {"status": "excluded", "reason": f"infra:{chosen['infra']}", "attempts": len(rows)}
            continue
        later = [int(r.get("attempt") or 1) for r in rows if int(r.get("attempt") or 1) > int(chosen.get("attempt") or 1)]
        tasks[t] = {"status": "used", "attempt": int(chosen.get("attempt") or 1), "tag": chosen["tag"], "success": bool(chosen.get("success")),
                    "domain": t.split("-")[0], "excluded_attempts": skipped + [{"attempt": a, "reason": "later_attempt"} for a in later]}
    used = [v for v in tasks.values() if v["status"] == "used"]
    ex = {}
    for v in tasks.values():
        if v["status"] == "excluded":
            ex[v["reason"]] = ex.get(v["reason"], 0) + 1
    n_exp = len(tasks)
    succ = sum(1 for v in used if v["success"])
    counts = {"expected": n_exp, "used": len(used), "success_used": succ, "excluded_by_reason": ex,
              "official_rate": round(100 * succ / max(n_exp, 1), 2), "valid_rate": round(100 * succ / max(len(used), 1), 2),
              "excluded_attempt_rows": sum(len(v.get("excluded_attempts", [])) for v in used)}
    report = os.path.join(results_dir, "report.json")
    if os.path.exists(report):
        try:
            s = _load_json(report)["summary"]
            counts["report_json"] = {k: s.get(k) for k in ("expected", "rows", "valid", "infra", "success", "rate_official", "rate_valid", "infra_kinds")}
        except (ValueError, KeyError):
            counts["report_json"] = None
    return {"attempt_policy": attempt_policy, "files": files, "corrupt_lines": corrupt, "tasks": tasks, "counts": counts}


# ------------------------------------------------------------------ gate logs -> steps

def _stem(tag, attempt):
    """runs_scored tags of retried attempts already end in -tryN; older rows may not"""
    return tag if (attempt <= 1 or re.search(r"-try\d+$", tag)) else f"{tag}-try{attempt}"


def gate_file(results_dir, tag, attempt):
    return os.path.join(results_dir, "gate", _stem(tag, attempt) + ".jsonl")


def multi_session_logs(results_dir):
    """Every gate log (used or not) that holds more than one session."""
    out = []
    gd = os.path.join(results_dir, "gate")
    for f in sorted(os.listdir(gd)) if os.path.isdir(gd) else []:
        if not f.endswith(".jsonl"):
            continue
        sess = {}
        for line in _lines(os.path.join(gd, f)):
            m = re.search(r'"session"\s*:\s*"([^"]+)"', line)
            if m and '"ev":"action"' in line.replace(" ", ""):
                sess[m.group(1)] = sess.get(m.group(1), 0) + 1
        if len(sess) > 1:
            out.append({"file": f, "actions_by_session": sess})
    return out


def nav_signature(sig):
    parts = sig.split("|")
    if len(parts) < 3 or parts[0] not in ("press_key", "hotkey"):
        return False
    keys = [k.lower() for k in parts[-1].split("+") if k]
    return bool(keys) and any(NAV.match(k) for k in keys) and all(NAV.match(k) or k in ("shift", "ctrl", "control") for k in keys)


def option_taken(e):
    """Mirror of valuetrain::option_taken."""
    dec, tier = e.get("decision") or "", e.get("tier") or ""
    trail = [str(x) for x in (e.get("trail") or [])]
    if (e.get("human_outcome") or "") in NO_ANSWER:
        return None
    if e.get("executed"):
        if dec != "direct" or tier == "human":
            return None
        for k, o in (("healed", "heal"), ("wait", "wait"), ("check", "check")):
            if k in trail:
                return o
        return "review" if ("reviewer_allow" in trail or tier == "review") else "direct"
    if "reviewer_deny" in trail or (tier == "review" and dec in ("look_light", "deny")):
        return "review"
    return dec if dec in ("look_light", "look_full", "reason", "ask_help", "review") else None


def v2_extra(tool, kind, role):
    r = (role or "").lower()
    b = lambda x: 1.0 if x else 0.0  # noqa: E731
    scroll = tool == "scroll"
    return {"tool_click": b("click" in tool), "tool_scroll": b(scroll), "tool_type": b(tool == "type_text"),
            "tool_key": b(tool in ("press_key", "hotkey")), "tool_set_value": b(tool == "set_value"), "scroll_pixel": b(scroll and kind == "pixel"),
            "role_button": b("button" in r and "radio" not in r and "toggle" not in r and "menu" not in r),
            "role_text": b(any(k in r for k in ("text", "entry", "edit", "paragraph", "document"))),
            "role_link_tab": b("link" in r or "tab" in r),
            "role_list_item": b("list item" in r or "tree item" in r or "row" in r or r == "item"),
            "role_combo": b("combo" in r)}


def history(prev, sig):
    """features::history_feats"""
    n = len(prev)
    fails = sum(1 for p in prev if p[1])
    consec = 0
    for p in reversed(prev):
        if not p[1]:
            break
        consec += 1
    return {"prev_none": 1.0 if (prev and prev[-1][2]) else 0.0, "sess_fail": math.log((fails + 1.0) / (n - fails + 4.0)),
            "consec_fail": float(min(consec, 3)), "sig_failed": 1.0 if any(p[1] and p[0] == sig for p in prev) else 0.0}


def cmd_feats(text, prev_fail, repeat):
    t = text or ""
    first = t.strip().split(" ")[0] if t.strip() else ""
    b = lambda x: 1.0 if x else 0.0  # noqa: E731
    return {"bias": 1.0, "cmd_grep": b(re.search(r"\b(grep|rg|egrep)\b", t)), "cmd_test": b(re.search(r"(^|\s)(test|\[)\s", t) or first in ("test", "[")),
            "cmd_find": b(first == "find" or " find " in t), "cmd_ls": b(first in ("ls", "stat", "file")),
            "cmd_cat": b(first in ("cat", "head", "tail", "wc")), "cmd_python": b(first.startswith("python") or "python3 -c" in t),
            "cmd_pipe": b("|" in t), "cmd_chain": b("&&" in t or "||" in t or ";" in t), "cmd_redirect": b(">" in t),
            "cmd_len": math.log1p(len(t)) / 6.0, "prev_cmd_fail": b(prev_fail), "cmd_repeat": b(repeat)}


def load_steps(path, task, domain, first_session_only=True):
    """Rows of one gate log. Executed rows carry y (step ok); hand-back rows carry y_next (the next executed GUI action
    of the session within 180 s). f1 = the logged decision-time features (the gate's v1 view, commands included in
    its history); f2 = v2 view (GUI-only history and last_*, plus V2_EXTRA)."""
    evs = []
    for line in _lines(path):
        try:
            evs.append(json.loads(line))
        except ValueError:
            continue
    first = next((e.get("session") for e in evs if e.get("session")), None)
    st = {"later_sessions_excluded_actions": 0, "later_sessions": 0}
    later = {e.get("session") for e in evs if e.get("session") and e.get("session") != first}
    st["later_sessions"] = len(later)
    retro = {e.get("aid") for e in evs if e.get("ev") == "retro_repeat" and e.get("session") == first}
    rows = []
    prev, last, last_gui, pending = [], None, None, []
    cprev_fail, csig = False, None
    for e in evs:
        if e.get("ev") != "action":
            continue
        if first_session_only and e.get("session") != first:
            st["later_sessions_excluded_actions"] += 1
            continue
        tool = e.get("tool") or ""
        tg = e.get("target") or {}
        kind, role = tg.get("kind") or "", tg.get("role") or ""
        sig = e.get("sig") or ""
        is_cmd = tool == "run_command"
        ts = float(e.get("ts") or 0.0)
        f1 = {k: float(v) for k, v in (e.get("features") or {}).items() if isinstance(v, (int, float))}
        f1["bias"] = 1.0
        risky = e.get("risk_level") is not None or e.get("handoff") is not None
        r = {"task": task, "domain": domain, "session": e.get("session"), "aid": e.get("aid"), "ts": ts, "tool": tool, "kind": kind,
             "is_cmd": is_cmd, "executed": bool(e.get("executed")), "decision": e.get("decision"), "tier": e.get("tier"),
             "option": option_taken(e), "risky": risky, "p_logged": e.get("p_direct"), "f1": f1, "gate_ms": float(e.get("gate_ms") or 0.0),
             "human_ms": float(((e.get("human") or {}).get("ms")) or 0.0), "trail": e.get("trail") or []}
        if is_cmd:
            text = sig.split("|", 2)[2] if sig.count("|") >= 2 else sig
            r["fc"] = cmd_feats(text, cprev_fail, sig == csig)
        else:
            f2 = {k: f1.get(k, 0.0) for k in V1_FEATURES}
            f2.update(history(prev, sig))
            le, lu, lp = last_gui or (False, False, False)
            f2.update({"last_err": float(le), "last_unverif": float(lu), "last_popup": float(lp)})
            f2.update(v2_extra(tool, kind, role))
            r["f2"] = f2
        if r["executed"]:
            err = bool(e.get("is_error"))
            ok = not err and not e.get("stale_error") and not (e.get("aid") in retro and not nav_signature(sig))
            r["y"] = 1.0 if ok else 0.0
            if is_cmd:
                cprev_fail, csig = not ok, sig
            else:
                for i, t0 in pending:
                    if ts - t0 <= 180.0:
                        rows[i]["y_next"] = r["y"]
                pending = []
                if last and last[0] == sig and not last[2] and ts - last[1] < 60.0 and prev:
                    prev[-1] = (prev[-1][0], True, prev[-1][2])
                none = (e.get("effect") or "") == "none"
                prev.append((sig, err or bool(e.get("stale_error")) or none, none))
                last_gui = (err, (e.get("effect") or "") == "unverifiable", bool(e.get("popup")))
                last = (sig, ts, err)
        elif not is_cmd:
            pending.append((len(rows), ts))
        rows.append(r)
    return rows, st


def load_all_steps(results_dir, audit, first_session_only=True):
    rows, st = [], {"files": 0, "missing_gate_logs": [], "later_sessions_excluded_actions": 0, "files_with_later_sessions": []}
    for t, v in sorted(audit["tasks"].items()):
        if v["status"] != "used":
            continue
        p = gate_file(results_dir, v["tag"], v["attempt"])
        if not os.path.exists(p):
            st["missing_gate_logs"].append(os.path.basename(p))
            continue
        r, s = load_steps(p, t, v["domain"], first_session_only)
        st["files"] += 1
        st["later_sessions_excluded_actions"] += s["later_sessions_excluded_actions"]
        if s["later_sessions"]:
            st["files_with_later_sessions"].append({"file": os.path.basename(p), "later_sessions": s["later_sessions"],
                                                    "actions_excluded": s["later_sessions_excluded_actions"]})
        for x in r:
            x["success"] = v["success"]
        rows += r
    return rows, st


# ------------------------------------------------------------------ models and metrics

def sigmoid(z):
    return 1.0 / (1.0 + math.exp(-z)) if z >= 0 else math.exp(z) / (1.0 + math.exp(z))


def predict(w, x):
    return sigmoid(sum(w.get(k, 0.0) * v for k, v in x.items()))


def _solve(a, b):
    n = len(b)
    m = [row[:] + [b[i]] for i, row in enumerate(a)]
    for c in range(n):
        p = max(range(c, n), key=lambda r: abs(m[r][c]))
        m[c], m[p] = m[p], m[c]
        if abs(m[c][c]) < 1e-12:
            continue
        for r in range(n):
            if r != c and m[r][c] != 0.0:
                f = m[r][c] / m[c][c]
                for k in range(c, n + 1):
                    m[r][k] -= f * m[c][k]
    return [m[i][n] / m[i][i] if abs(m[i][i]) > 1e-12 else 0.0 for i in range(n)]


def fit_logistic(xs, ys, names, l2=1.0, iters=25):
    """Ridge logistic regression by Newton steps (bias not penalised). xs: list of {name: value}."""
    k = len(names)
    idx = {n: i for i, n in enumerate(names)}
    dense = [[(idx[n], v) for n, v in x.items() if n in idx and v != 0.0] for x in xs]
    w = [0.0] * k
    for _ in range(iters):
        g = [0.0] * k
        h = [[0.0] * k for _ in range(k)]
        for x, y in zip(dense, ys):
            p = sigmoid(sum(w[i] * v for i, v in x))
            d, s = p - y, max(p * (1 - p), 1e-9)
            for i, v in x:
                g[i] += d * v
                hi = h[i]
                for j, u in x:
                    hi[j] += s * v * u
        for i in range(k):
            if names[i] != "bias":
                g[i] += l2 * w[i]
                h[i][i] += l2
            h[i][i] += 1e-6
        step = _solve(h, g)
        w = [w[i] - step[i] for i in range(k)]
        if max(abs(s) for s in step) < 1e-7:
            break
    return {n: w[i] for i, n in enumerate(names)}


def auc(ps, ys):
    pos = sum(1 for y in ys if y == 1)
    neg = len(ys) - pos
    if pos == 0 or neg == 0:
        return None
    order = sorted(range(len(ps)), key=lambda i: ps[i])
    ranks = [0.0] * len(ps)
    i = 0
    while i < len(order):
        j = i
        while j + 1 < len(order) and ps[order[j + 1]] == ps[order[i]]:
            j += 1
        for t in range(i, j + 1):
            ranks[order[t]] = (i + j) / 2.0 + 1.0
        i = j + 1
    sp = sum(ranks[i] for i in range(len(ps)) if ys[i] == 1)
    return (sp - pos * (pos + 1) / 2.0) / (pos * neg)


def point(ps, ys, bins=10):
    n = len(ys)
    if n == 0:
        return {"n": 0}
    order = sorted(range(n), key=lambda i: ps[i])
    b = min(bins, n)
    ece = 0.0
    table = []
    for k in range(b):
        ix = order[k * n // b:(k + 1) * n // b]
        if not ix:
            continue
        mp = sum(ps[i] for i in ix) / len(ix)
        my = sum(ys[i] for i in ix) / len(ix)
        ece += abs(mp - my) * len(ix) / n
        table.append({"n": len(ix), "p_lo": round(ps[ix[0]], 3), "p_hi": round(ps[ix[-1]], 3), "mean_p": round(mp, 3), "success": round(my, 3)})
    brier = sum((p - y) ** 2 for p, y in zip(ps, ys)) / n
    return {"n": n, "base_rate": round(sum(ys) / n, 4), "mean_p": round(sum(ps) / n, 4), "auc": auc(ps, ys), "ece_mass": ece, "brier": brier, "table": table}


def _q(v, q):
    v = sorted(v)
    if not v:
        return None
    pos = q * (len(v) - 1)
    i = int(pos)
    f = pos - i
    return v[i] * (1 - f) + v[i + 1] * f if i + 1 < len(v) else v[i]


def metrics(ps, ys, groups=None, reps=0, seed=0):
    m = point(ps, ys)
    out = {k: (round(v, 4) if isinstance(v, float) else v) for k, v in m.items()}
    if reps and groups and m["n"]:
        by = {}
        for i, g in enumerate(groups):
            by.setdefault(g, []).append(i)
        keys = sorted(by)
        rng = random.Random(seed)
        acc = {"auc": [], "ece_mass": [], "brier": []}
        for _ in range(reps):
            ix = [i for _ in keys for i in by[keys[rng.randrange(len(keys))]]]
            mm = point([ps[i] for i in ix], [ys[i] for i in ix])
            for k in acc:
                if mm.get(k) is not None:
                    acc[k].append(mm[k])
        for k, v in acc.items():
            out[k + "_ci95"] = [round(_q(v, 0.025), 4), round(_q(v, 0.975), 4)] if len(v) >= 10 else None
        out["bootstrap"] = {"reps": reps, "resample": "sessions"}
    return out


def _r(x, d=4):
    return None if x is None else round(x, d)


def paired_auc_diff(pa, pb, ys, groups, reps=200, seed=0):
    """AUC(b) - AUC(a) on the same rows with a paired session-bootstrap 95% CI."""
    d0 = (auc(pb, ys) or 0.0) - (auc(pa, ys) or 0.0)
    by = {}
    for i, g in enumerate(groups):
        by.setdefault(g, []).append(i)
    keys = sorted(by)
    rng = random.Random(seed ^ 0x5A)
    ds = []
    for _ in range(reps):
        ix = [i for _ in keys for i in by[keys[rng.randrange(len(keys))]]]
        a, b = auc([pa[i] for i in ix], [ys[i] for i in ix]), auc([pb[i] for i in ix], [ys[i] for i in ix])
        if a is not None and b is not None:
            ds.append(b - a)
    return {"auc_diff": round(d0, 4), "ci95": [round(_q(ds, 0.025), 4), round(_q(ds, 0.975), 4)] if len(ds) >= 10 else None, "reps": len(ds)}


def builtin_calibration(rows, reps=200, seed=0):
    """The built-in osworld_v3 predictor as the live gate logged it (p_direct), on executed actions."""
    ex = [r for r in rows if r["executed"] and r["p_logged"] is not None and not r["risky"]]
    out = {}
    for name, sel in (("gui", [r for r in ex if not r["is_cmd"]]), ("command", [r for r in ex if r["is_cmd"]]), ("all", ex)):
        out[name] = metrics([float(r["p_logged"]) for r in sel], [r["y"] for r in sel], [r["session"] for r in sel], reps, seed)
    cm = [r for r in ex if r["is_cmd"]]
    if cm:
        out["command"]["no_obs_share"] = round(sum(1 for r in cm if r["f1"].get("no_obs", 0) == 1.0) / len(cm), 3)
    out["note"] = ("p_direct as logged live by gui-gate 0.9.9 (built-in osworld_v3, fitted on logs up to 2026-10-22, so this run is "
                   "out-of-sample for it). 'all' mixes commands into the GUI predictor as the 2026-10-25 check did; commands are "
                   "scored with no_obs=1 (weight -2.41) although they never have a window observation")
    return out


def cross_fit(rows, feat_key, names, l2, folds_by="domain"):
    """Leave-one-domain-out predictions (no task family is ever scored by a model that saw it)."""
    doms = sorted({r[folds_by] for r in rows})
    preds = [None] * len(rows)
    for d in doms:
        tr = [r for r in rows if r[folds_by] != d]
        if len({r["y"] for r in tr}) < 2:
            continue
        w = fit_logistic([r[feat_key] for r in tr], [r["y"] for r in tr], names, l2=l2)
        for i, r in enumerate(rows):
            if r[folds_by] == d:
                preds[i] = predict(w, r[feat_key])
    return preds


def flag_stats(ps, ys, budgets=(0.05, 0.10, 0.20, 0.323)):
    """Flag the share `b` of steps with the highest predicted failure (lowest p): precision / recall of failures."""
    n = len(ps)
    order = sorted(range(n), key=lambda i: ps[i])
    fails = sum(1 for y in ys if y == 0)
    out = []
    for b in budgets:
        k = max(1, int(round(b * n)))
        flagged = order[:k]
        caught = sum(1 for i in flagged if ys[i] == 0)
        out.append({"budget": b, "flagged": k, "failed_flagged": caught, "precision": round(caught / k, 4),
                    "recall": round(caught / max(fails, 1), 4), "base_failure_rate": round(fails / max(n, 1), 4)})
    return out


def predictors(rows, reps=200, seed=0, l2=1.0):
    ex = [r for r in rows if r["executed"] and not r["risky"]]
    gui = [r for r in ex if not r["is_cmd"]]
    cmd = [r for r in ex if r["is_cmd"]]
    res = {"builtin": builtin_calibration(rows, reps, seed), "split": "leave-one-domain-out cross-fitting (each domain scored by a model fitted on the others); CIs: session bootstrap"}
    ys = [r["y"] for r in gui]
    gs = [r["session"] for r in gui]
    models = {}
    for name, key, names in (("gui_v1_refit", "f1", V1_FEATURES), ("gui_v2", "f2", V2_FEATURES)):
        p = cross_fit(gui, key, names, l2)
        keep = [i for i, x in enumerate(p) if x is not None]
        models[name] = {"metrics": metrics([p[i] for i in keep], [ys[i] for i in keep], [gs[i] for i in keep], reps, seed),
                        "flags": flag_stats([p[i] for i in keep], [ys[i] for i in keep])}
    bp = [float(r["p_logged"]) for r in gui]
    models["gui_builtin"] = {"metrics": res["builtin"]["gui"], "flags": flag_stats(bp, ys)}
    res["gui"] = models
    p1 = cross_fit(gui, "f1", V1_FEATURES, l2)
    p2 = cross_fit(gui, "f2", V2_FEATURES, l2)
    keep = [i for i in range(len(gui)) if p1[i] is not None and p2[i] is not None]
    res["gui_v2_minus_v1"] = paired_auc_diff([p1[i] for i in keep], [p2[i] for i in keep], [ys[i] for i in keep], [gs[i] for i in keep], reps, seed)
    by_tool = {}
    for tool in sorted({r["tool"] for r in gui}):
        ix = [i for i in keep if gui[i]["tool"] == tool]
        if len(ix) < 30:
            continue
        yy = [ys[i] for i in ix]
        by_tool[tool] = {"n": len(ix), "failure_rate": round(1 - sum(yy) / len(yy), 4),
                         "auc_builtin": _r(auc([bp[i] for i in ix], yy)), "auc_v1_refit": _r(auc([p1[i] for i in ix], yy)),
                         "auc_v2": _r(auc([p2[i] for i in ix], yy))}
    res["gui_by_tool"] = by_tool
    if cmd:
        cy = [r["y"] for r in cmd]
        cg = [r["session"] for r in cmd]
        pc = cross_fit(cmd, "fc", CMD_FEATURES, l2)
        keep = [i for i, x in enumerate(pc) if x is not None]
        base = sum(cy) / len(cy)
        res["command"] = {"command_model": metrics([pc[i] for i in keep], [cy[i] for i in keep], [cg[i] for i in keep], reps, seed),
                          "constant_base_rate": metrics([base] * len(cy), cy, cg, reps, seed),
                          "builtin_gui_predictor": res["builtin"]["command"],
                          "label_note": "command label = exit status 0 (and not an immediate repeat); a non-zero exit is often intended (grep without a match, test -f), so it is a weak failure signal"}
    w = fit_logistic([r["f2"] for r in gui], ys, V2_FEATURES, l2=l2) if gui else {}
    res["gui_v2_weights_all_rows"] = {k: round(v, 4) for k, v in w.items()}
    return res


# ------------------------------------------------------------------ interventions (what the logs can say)

def interventions(rows):
    hb = [r for r in rows if not r["executed"] and not r["is_cmd"]]
    by = {}
    for r in hb:
        k = r["decision"] or "?"
        d = by.setdefault(k, {"n": 0, "value_option": r["option"], "next_gui_step_ok": 0, "next_gui_step_labelled": 0})
        d["n"] += 1
        if "y_next" in r:
            d["next_gui_step_labelled"] += 1
            d["next_gui_step_ok"] += int(r["y_next"] == 1.0)
    ex_gui = sum(1 for r in rows if r["executed"] and not r["is_cmd"])
    return {"gui_actions": ex_gui + len(hb), "executed_gui": ex_gui, "refused_gui": len(hb),
            "refused_commands": sum(1 for r in rows if not r["executed"] and r["is_cmd"]), "by_decision": by,
            "note": "next_gui_step_ok is the outcome of the agent's NEXT executed GUI action after a refusal - observational: the agent may have "
                    "changed the step, so it is neither the outcome the refused step would have had nor evidence that the refusal avoided a failure"}


# ------------------------------------------------------------------ costs from the logs

def pi_turns(results_dir, tag, attempt):
    p = os.path.join(results_dir, "pi", _stem(tag, attempt) + ".jsonl")
    out = []
    if not os.path.exists(p):
        return out
    for line in _lines(p):
        if '"turn_end"' not in line:
            continue
        try:
            e = json.loads(line)
        except ValueError:
            continue
        m = e.get("message") or {}
        if e.get("type") != "turn_end" or m.get("role") != "assistant" or not m.get("timestamp"):
            continue
        u = m.get("usage") or {}
        out.append((m["timestamp"] / 1000.0, float(u.get("input") or 0), float(u.get("output") or 0), float(u.get("cacheRead") or 0),
                    float(m.get("durationMs") or 0) / 1000.0))
    out.sort()
    return out


def costs(results_dir, audit, first_session_only=True):
    """Per refused option: wall seconds from the refusal to the session's next action (the model's extra turn), the real
    model tokens of the assistant turns that ended in between (Pi turn_end usage), and the person seconds. Gate-side options
    (executed after wait / check / heal / reviewer allow): their gate_ms over the median direct gate_ms."""
    acc, gate_side, direct_ms, turns_all = {}, {}, [], []
    for t, v in sorted(audit["tasks"].items()):
        if v["status"] != "used":
            continue
        p = gate_file(results_dir, v["tag"], v["attempt"])
        if not os.path.exists(p):
            continue
        turns = pi_turns(results_dir, v["tag"], v["attempt"])
        turns_all += turns
        tts = [x[0] for x in turns]
        evs = []
        for line in _lines(p):
            try:
                evs.append(json.loads(line))
            except ValueError:
                pass
        first = next((e.get("session") for e in evs if e.get("session")), None)
        acts = [e for e in evs if e.get("ev") == "action" and (not first_session_only or e.get("session") == first)]
        for i, e in enumerate(acts):
            is_cmd = e.get("tool") == "run_command"
            opt = option_taken(e)
            dec = e.get("decision") or ""
            if e.get("executed"):
                if is_cmd:
                    continue
                if opt == "direct":
                    direct_ms.append(float(e.get("gate_ms") or 0))
                elif opt:
                    gate_side.setdefault(opt, []).append(float(e.get("gate_ms") or 0))
                continue
            key = opt or (dec if dec in ("ask_human", "handoff", "deny", "stop_loop", "risk_review") else None)
            if key is None:
                continue
            if is_cmd:
                key = "command:" + key
            nxt = acts[i + 1] if i + 1 < len(acts) else None
            if nxt is None:
                continue
            t0, t1 = float(e["ts"]), float(nxt["ts"])
            gap = t1 - t0
            if gap < 0 or gap > 600:
                continue
            lo, hi = bisect.bisect_right(tts, t0), bisect.bisect_right(tts, t1)
            sel = turns[lo:hi]
            person = float(((e.get("human") or {}).get("ms")) or 0) / 1000.0
            acc.setdefault(key, []).append((max(gap - person, 0.0), sum(x[1] for x in sel), sum(x[2] for x in sel), sum(x[3] for x in sel), len(sel), person))
    med = _q(direct_ms, 0.5) or 0.0
    out = {}
    for k, v in sorted(acc.items()):
        n = len(v)
        out[k] = {"n": n, "extra_s_mean": round(sum(x[0] for x in v) / n, 3), "extra_s_median": round(_q([x[0] for x in v], 0.5), 3),
                  "tokens_input_mean": round(sum(x[1] for x in v) / n, 1), "tokens_output_mean": round(sum(x[2] for x in v) / n, 1),
                  "tokens_cache_read_mean": round(sum(x[3] for x in v) / n, 1), "model_turns_mean": round(sum(x[4] for x in v) / n, 2),
                  "person_s_mean": round(sum(x[5] for x in v) / n, 2), "src": "gate ts + pi turn_end usage"}
    for k, v in sorted(gate_side.items()):
        name = k if k not in out else f"{k}_executed"  # review: allowed by the reviewer (gate-side) vs refused (hand-back)
        out[name] = {"n": len(v), "extra_s_mean": round(max(sum(v) / len(v) - med, 0.0) / 1000.0, 4), "tokens_input_mean": 0.0,
                     "tokens_output_mean": 0.0, "src": "gate_ms over the median direct gate_ms"}
    if "review" in out and "review_executed" in out:
        # the option's expected cost mixes both outcomes in the logged proportions
        a, b = out["review"], out["review_executed"]
        n = a["n"] + b["n"]
        out["review_combined"] = {"n": n, "extra_s_mean": round((a["extra_s_mean"] * a["n"] + b["extra_s_mean"] * b["n"]) / n, 3),
                                  "tokens_input_mean": round(a["tokens_input_mean"] * a["n"] / n, 1),
                                  "tokens_output_mean": round(a["tokens_output_mean"] * a["n"] / n, 1),
                                  "src": "review refused (hand-back) and allowed (gate-side), weighted by their counts"}
    tm = {"turns": len(turns_all)}
    if turns_all:
        tm.update({"duration_s_median": round(_q([x[4] for x in turns_all], 0.5), 2), "duration_s_mean": round(sum(x[4] for x in turns_all) / len(turns_all), 2),
                   "input_median": _q([x[1] for x in turns_all], 0.5), "output_median": _q([x[2] for x in turns_all], 0.5),
                   "cache_read_median": _q([x[3] for x in turns_all], 0.5)})
    return {"options": out, "direct_gate_ms_median": round(med, 2), "direct_n": len(direct_ms), "model_turn": tm,
            "costs_v1_for_gate": {"kind": "costs_v1", "costs": dict({"direct": {"extra_s": 0.0, "tokens": 0.0, "person_s": 0.0, "n": len(direct_ms)}},
                                                                     **{k: {"extra_s": v["extra_s_mean"], "tokens": v["tokens_input_mean"] + v["tokens_output_mean"],
                                                                            "person_s": v.get("person_s_mean", 0.0), "n": v["n"]}
                                                                        for k, v in ((("review" if k == "review_combined" else k), v) for k, v in out.items()
                                                                                     if k in VALUE_OPTIONS and k != "review" or k == "review_combined")})},
            "note": "means are expected costs; hand-back tokens are the real tokens of the model turns that ended between the refusal and the next action (the prompt grows with the conversation, so input tokens include the whole context of that turn; cache_read is reported apart)"}


# ------------------------------------------------------------------ the Rust tools on the same selection

def selected_logs(results_dir, audit):
    return [gate_file(results_dir, v["tag"], v["attempt"]) for _, v in sorted(audit["tasks"].items())
            if v["status"] == "used" and os.path.exists(gate_file(results_dir, v["tag"], v["attempt"]))]


def rust_replay(bin_path, logs):
    """`gui-gate replay` on the selected logs: default config, and with a reviewer configured (as the 2026-10-25 check)."""
    out = {}
    for name, extra in (("default", []), ("with_reviewer", ["--reviewer"])):
        r = subprocess.run([bin_path, "replay", *logs, "--human", "deny", *extra], capture_output=True, text=True, timeout=110)
        out[name] = json.loads(r.stdout) if r.returncode == 0 else {"error": r.stderr[-400:]}
    return out


def rust_treatment(bin_path, logs, out_json, task_labels=None, seed=0):
    """gui-gate train --model treatment --features v2 on the selected logs, family split: IPW-weighted (its coverage
    diagnostics decide which options the model may choose) and unweighted (same split, for comparison)."""
    out = {}
    for name, extra in (("ipw", ["--ipw"]), ("unweighted", [])):
        args = [bin_path, "train", "--model", "treatment", *logs, "-o", out_json.replace(".json", f"_{name}.json"), "--split", "family",
                "--features", "v2", "--commands", "exclude", "--boot", "30", "--seed", str(seed), *extra]
        r = subprocess.run(args, capture_output=True, text=True, timeout=115)
        out[name] = _treatment_summary(r)
    return out


def _treatment_summary(r):
    if r.returncode != 0:
        return {"error": r.stderr[-400:] or r.stdout[-400:]}
    ev = json.loads(r.stdout)
    keep = {k: ev.get(k) for k in ("split", "train_rows", "test_rows", "coverage", "intervals", "load_opts", "ipw", "load")}
    keep["treatment_on_test"] = {k: v for k, v in (ev.get("treatment_on_test") or {}).items() if k != "reliability_equal_mass"}
    keep["by_option"] = {o: {k: v for k, v in m.items() if k in ("n", "base_rate", "auc", "ece_mass", "brier")}
                         for o, m in (ev.get("treatment_on_test_by_option") or {}).items()}
    return keep


# ------------------------------------------------------------------ report

def claims():
    return {
        "predicted_execution_failure": {"status": "measured",
                                        "meaning": "an executed step that errored or was immediately redone had a low predicted success beforehand (AUC, precision/recall at an intervention budget)"},
        "avoided_failure_after_intervention": {"status": "not_estimable_from_logs",
                                               "meaning": "whether an intervention made a step succeed that would otherwise have failed; the refused step's outcome under direct is unobserved and the agent's next step is a different action. Needs paired counterfactual branches or a randomized online comparison (bench/online_plan.py)"},
    }


def _fmt(m, k):
    v = m.get(k)
    ci = m.get(k + "_ci95")
    if v is None:
        return "n/a"
    return f"{v:.3f}" + (f" [{ci[0]:.3f}, {ci[1]:.3f}]" if ci else "")


def render_md(rep):
    a, c = rep["audit"], rep["audit"]["counts"]
    L = [f"# Offline evaluation v2: {rep['results_dir']}", "",
         f"Generated by bench/offline_v2.py (seed {rep['seed']}, {rep['reps']} session-bootstrap replicates). Input results are read only.", "",
         "## Data and exclusions", "",
         f"- expected tasks {c['expected']}, used {c['used']} (attempt policy `{a['attempt_policy']}`), success {c['success_used']}: "
         f"{c['official_rate']} % of expected, {c['valid_rate']} % of used",
         f"- excluded tasks by reason: {json.dumps(c['excluded_by_reason'])}",
         f"- corrupt lines: {len(a['corrupt_lines'])} " + "; ".join(f"{b['file']}:{b['line']} ({b['tag']})" for b in a["corrupt_lines"]),
         f"- repeated attempts not used (rows of used tasks): {c['excluded_attempt_rows']}",
         f"- gate logs read {rep['steps']['files']}, later sessions inside a log excluded: {rep['steps']['later_sessions_excluded_actions']} actions "
         f"in {len(rep['steps']['files_with_later_sessions'])} file(s); missing logs: {len(rep['steps']['missing_gate_logs'])}; "
         f"logs with more than one session (all attempts, used or not): {json.dumps(rep['steps']['multi_session_logs_all'])}",
         f"- executed actions: GUI {rep['steps']['executed_gui']}, command {rep['steps']['executed_cmd']}; refused GUI {rep['interventions']['refused_gui']}", ""]
    if c.get("report_json"):
        L += [f"report.json of the run: {json.dumps(c['report_json'])}", ""]
    if "predictors" in rep:
        P = rep["predictors"]
        L += ["## Predictor calibration (executed actions; label = step ran without error and was not immediately redone)", "",
              "| set | model | n | base rate | AUC [95% CI] | ECE equal-mass [95% CI] | Brier |", "|---|---|---|---|---|---|---|"]
        b = P["builtin"]
        for name in ("gui", "command", "all"):
            m = b[name]
            L.append(f"| {name} | built-in osworld_v3 (logged) | {m['n']} | {m.get('base_rate')} | {_fmt(m, 'auc')} | {_fmt(m, 'ece_mass')} | {_fmt(m, 'brier')} |")
        for name in ("gui_v1_refit", "gui_v2"):
            m = P["gui"][name]["metrics"]
            L.append(f"| gui | {name} (leave-one-domain-out) | {m['n']} | {m.get('base_rate')} | {_fmt(m, 'auc')} | {_fmt(m, 'ece_mass')} | {_fmt(m, 'brier')} |")
        if "command" in P:
            for name in ("command_model", "constant_base_rate"):
                m = P["command"][name]
                L.append(f"| command | {name} | {m['n']} | {m.get('base_rate')} | {_fmt(m, 'auc')} | {_fmt(m, 'ece_mass')} | {_fmt(m, 'brier')} |")
        L += ["", P["builtin"]["note"], "",
              "### Flagging executed GUI steps by predicted failure (what the logs CAN say: the step was predicted to fail and did)", "",
              "| model | budget | flagged | failed among flagged | precision | recall | base failure rate |", "|---|---|---|---|---|---|---|"]
        for name in ("gui_builtin", "gui_v1_refit", "gui_v2"):
            for f in P["gui"][name]["flags"]:
                L.append(f"| {name} | {f['budget']:.1%} | {f['flagged']} | {f['failed_flagged']} | {f['precision']} | {f['recall']} | {f['base_failure_rate']} |")
        if "gui_v2_minus_v1" in P:
            d = P["gui_v2_minus_v1"]
            L += ["", f"AUC v2 - v1 refit (paired session bootstrap): {d['auc_diff']} {d['ci95']}", "",
                  "| tool | n | failure rate | AUC built-in | AUC v1 refit | AUC v2 |", "|---|---|---|---|---|---|"]
            for t, v in P.get("gui_by_tool", {}).items():
                L.append(f"| {t} | {v['n']} | {v['failure_rate']} | {v['auc_builtin']} | {v['auc_v1_refit']} | {v['auc_v2']} |")
        L.append("")
    if "replay" in rep and rep["replay"]:
        L += ["### gui-gate replay (current default rules + legacy prior, commands counted apart)", "", "```", json.dumps(rep["replay"], indent=1)[:3000], "```", ""]
    L += ["## Live interventions (observational)", "", rep["interventions"]["note"], "",
          "| decision | value option | n | next GUI step labelled | next GUI step ok |", "|---|---|---|---|---|"]
    for k, d in sorted(rep["interventions"]["by_decision"].items(), key=lambda x: -x[1]["n"]):
        L.append(f"| {k} | {d['value_option']} | {d['n']} | {d['next_gui_step_labelled']} | {d['next_gui_step_ok']} |")
    L.append("")
    if "costs" in rep:
        C = rep["costs"]
        L += ["## Costs from the logs", "", f"model turn: {json.dumps(C['model_turn'])}", "",
              "| option | n | extra s mean | extra s median | input tokens | output tokens | cache read | turns | person s |", "|---|---|---|---|---|---|---|---|---|"]
        for k, v in C["options"].items():
            L.append(f"| {k} | {v['n']} | {v['extra_s_mean']} | {v.get('extra_s_median', '')} | {v['tokens_input_mean']} | {v['tokens_output_mean']} | "
                     f"{v.get('tokens_cache_read_mean', '')} | {v.get('model_turns_mean', '')} | {v.get('person_s_mean', '')} |")
        L += ["", C["note"], ""]
    if rep.get("treatment"):
        L += ["## Treatment model v2 (gui-gate train --model treatment --features v2, family split; IPW and unweighted)", "", "```",
              json.dumps(rep["treatment"], indent=1)[:4000], "```", ""]
    L += ["## Claims", ""]
    for k, v in rep["claims"].items():
        L.append(f"- **{k}**: {v['status']} - {v['meaning']}")
    L += ["", "\"Avoided a failure because of the intervention\" is not estimable from these logs and is not claimed.", ""]
    return "\n".join(L)


def run_all(results_dir, out_dir, reps=200, seed=0, bin_path=None, attempt_policy="first_valid", stages=("audit", "predictors", "costs", "replay", "treatment")):
    os.makedirs(out_dir, exist_ok=True)
    piece = lambda n: os.path.join(out_dir, f"_{n}.json")  # noqa: E731
    audit = audit_runs(results_dir, attempt_policy)
    rows, st = load_all_steps(results_dir, audit)
    st["executed_gui"] = sum(1 for r in rows if r["executed"] and not r["is_cmd"])
    st["executed_cmd"] = sum(1 for r in rows if r["executed"] and r["is_cmd"])
    st["multi_session_logs_all"] = multi_session_logs(results_dir)
    rep = {"kind": "offline_v2", "results_dir": os.path.relpath(results_dir), "seed": seed, "reps": reps, "audit": {k: v for k, v in audit.items() if k != "tasks"},
           "steps": st, "interventions": interventions(rows), "claims": claims()}
    if "predictors" in stages:
        rep["predictors"] = predictors(rows, reps, seed)
        _dump_json(rep["predictors"], piece("predictors"))
    elif os.path.exists(piece("predictors")):
        rep["predictors"] = _load_json(piece("predictors"))
    if "costs" in stages:
        rep["costs"] = costs(results_dir, audit)
        _dump_json(rep["costs"], piece("costs"))
    elif os.path.exists(piece("costs")):
        rep["costs"] = _load_json(piece("costs"))
    logs = selected_logs(results_dir, audit)
    for name, fn in (("replay", lambda: rust_replay(bin_path, logs)),
                     ("treatment", lambda: rust_treatment(bin_path, logs, os.path.join(out_dir, "treatment_v2.json"), seed=seed))):
        if name in stages and bin_path:
            rep[name] = fn()
            _dump_json(rep[name], piece(name))
        elif os.path.exists(piece(name)):
            rep[name] = _load_json(piece(name))
    excl = {"attempt_policy": attempt_policy, "corrupt_lines": audit["corrupt_lines"],
            "tasks_excluded": {t: v for t, v in sorted(audit["tasks"].items()) if v["status"] != "used"},
            "attempts_not_used": {t: v["excluded_attempts"] for t, v in sorted(audit["tasks"].items()) if v["status"] == "used" and v["excluded_attempts"]},
            "later_sessions": st["files_with_later_sessions"], "multi_session_logs_all": st["multi_session_logs_all"],
            "missing_gate_logs": st["missing_gate_logs"]}
    with open(os.path.join(out_dir, "exclusions.json"), "w") as fh:
        json.dump(excl, fh, indent=1)
    with open(os.path.join(out_dir, "report.json"), "w") as fh:
        json.dump(rep, fh, indent=1)
    with open(os.path.join(out_dir, "report.md"), "w", encoding="utf-8") as fh:
        fh.write(render_md(rep))
    return rep


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("results_dir")
    ap.add_argument("-o", "--out", required=True)
    ap.add_argument("--attempt", default="first_valid", choices=["first_valid", "first"])
    ap.add_argument("--reps", type=int, default=200)
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--bin", default=None, help="gui-gate binary for the replay / treatment stages")
    ap.add_argument("--stage", action="append", help="audit | predictors | costs | replay | treatment (repeatable; default all)")
    a = ap.parse_args(argv)
    stages = tuple(a.stage) if a.stage else ("audit", "predictors", "costs", "replay", "treatment")
    rep = run_all(a.results_dir, a.out, a.reps, a.seed, a.bin, a.attempt, stages)
    print(json.dumps({"out": a.out, "used": rep["audit"]["counts"]["used"], "stages": stages}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
