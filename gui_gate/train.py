"""Fit P(direct succeeds) from gate logs and calibrate it.

Labels come only from actions that actually ran directly (best collected with
--policy direct --collect, so nothing filters which actions get executed).
An action counts as a failure when the driver errored, the token was stale,
or the agent immediately re-sent the same action (retro_repeat).

Split is by session (held-out sessions), so a model is never scored on the
same task run it was trained on.

    gui-gate-train LOG_DIR_OR_FILES... -o weights.json [--holdout 0.3]
"""
from __future__ import annotations

import argparse
import glob
import json
import math
import os
import random
from collections import defaultdict

from .decide import PRIOR_WEIGHTS, sigmoid
from .features import FEATURE_NAMES

# check_pass/check_fail are probe outcomes, not available for every action;
# they are fitted only on rows that have them.


def _outcomes_from_fixture(gate_events, fixture_path):
    """Label each executed click with what the environment says really happened.

    Environment outcomes are legitimate TRAINING LABELS (they are never features):
    a click succeeds when the app recorded a click on the caption the agent meant.
    Clicks on a disabled/blocked UI or with no recorded effect count as failures.
    """
    fx = [json.loads(l) for l in open(fixture_path, encoding="utf-8")]
    fx = [e for e in fx if e.get("ev") in ("click", "ignored")]
    out = {}
    prev_ts = 0.0
    for e in gate_events:
        if e.get("ev") != "action" or not e.get("executed"):
            continue
        ts = e["ts"]
        hits = [f for f in fx if prev_ts < f["t"] <= ts + 0.05]
        prev_ts = ts + 0.05
        if e.get("tool") not in ("click", "double_click"):
            continue
        want = (e.get("target") or {}).get("label")
        ok = any(f["ev"] == "click" and f.get("caption") == want for f in hits)
        out[e.get("aid")] = ok
    return out


def load(paths):
    files = []
    for p in paths:
        files += sorted(f for f in glob.glob(os.path.join(p, "*.jsonl")) if not f.endswith(".fixture.jsonl")) \
            if os.path.isdir(p) else [p]
    rows, retro, env = [], set(), {}
    for fn in files:
        evs = []
        for line in open(fn, encoding="utf-8"):
            try:
                evs.append(json.loads(line))
            except json.JSONDecodeError:
                continue
        fxp = fn[:-len(".jsonl")] + ".fixture.jsonl"
        if os.path.exists(fxp):
            for aid, ok in _outcomes_from_fixture(evs, fxp).items():
                env[(evs[0]["session"] if evs else None, aid)] = ok
        for e in evs:
            if e.get("ev") == "retro_repeat" and e.get("aid") is not None:
                retro.add((e["session"], e["aid"]))
            elif e.get("ev") == "action" and e.get("executed") and e.get("decision") == "direct":
                rows.append(e)
    data = []
    for e in rows:
        key = (e["session"], e.get("aid"))
        if key in env:
            ok = env[key] and not e.get("is_error")
        else:
            ok = not e.get("is_error") and not e.get("stale_error") and key not in retro
        f = dict(e["features"])
        if e.get("shadow_check") is not None and not (f.get("check_pass") or f.get("check_fail")):
            f["check_pass"] = float(e["shadow_check"] is True)
            f["check_fail"] = float(e["shadow_check"] is False)
        data.append((e["session"], f, 1.0 if ok else 0.0))
    return data


def fit(data, l2=0.05, epochs=400, lr=0.1, init=None):
    w = {k: (init or {}).get(k, 0.0) for k in FEATURE_NAMES}
    n = max(1, len(data))
    for _ in range(epochs):
        g = defaultdict(float)
        for _, f, y in data:
            p = sigmoid(sum(w[k] * f.get(k, 0.0) for k in FEATURE_NAMES))
            for k in FEATURE_NAMES:
                v = f.get(k, 0.0)
                if v:
                    g[k] += (p - y) * v
        for k in FEATURE_NAMES:
            reg = 0.0 if k == "bias" else l2 * (w[k] - (init or {}).get(k, 0.0))
            w[k] -= lr * (g[k] / n + reg)
    return w


def logit(w, f):
    return sum(w.get(k, 0.0) * f.get(k, 0.0) for k in FEATURE_NAMES)


def platt(zs, ys, epochs=500, lr=0.05):
    a, b = 1.0, 0.0
    n = max(1, len(zs))
    for _ in range(epochs):
        ga = gb = 0.0
        for z, y in zip(zs, ys):
            p = sigmoid(a * z + b)
            ga += (p - y) * z
            gb += (p - y)
        a -= lr * ga / n
        b -= lr * gb / n
    return a, b


def metrics(ps, ys, bins=10):
    n = len(ys)
    if n == 0:
        return {}
    brier = sum((p - y) ** 2 for p, y in zip(ps, ys)) / n
    nll = -sum(y * math.log(max(p, 1e-6)) + (1 - y) * math.log(max(1 - p, 1e-6)) for p, y in zip(ps, ys)) / n
    ece, table = 0.0, []
    for i in range(bins):
        lo, hi = i / bins, (i + 1) / bins
        idx = [j for j, p in enumerate(ps) if lo <= p < hi or (i == bins - 1 and p == 1.0)]
        if not idx:
            continue
        conf = sum(ps[j] for j in idx) / len(idx)
        acc = sum(ys[j] for j in idx) / len(idx)
        ece += len(idx) / n * abs(conf - acc)
        table.append({"bin": f"{lo:.1f}-{hi:.1f}", "n": len(idx), "mean_p": round(conf, 3), "success": round(acc, 3)})
    pos = [p for p, y in zip(ps, ys) if y == 1]
    neg = [p for p, y in zip(ps, ys) if y == 0]
    auc = None
    if pos and neg:
        auc = sum((1.0 if a > b else 0.5 if a == b else 0.0) for a in pos for b in neg) / (len(pos) * len(neg))
    return {"n": n, "base_rate": round(sum(ys) / n, 3), "brier": round(brier, 4), "nll": round(nll, 4),
            "ece": round(ece, 4), "auc": None if auc is None else round(auc, 3), "reliability": table}


def main(argv=None):
    ap = argparse.ArgumentParser(prog="gui-gate-train")
    ap.add_argument("paths", nargs="+")
    ap.add_argument("-o", "--out", default="weights.json")
    ap.add_argument("--holdout", type=float, default=0.3, help="fraction of sessions held out")
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--l2", type=float, default=0.05)
    a = ap.parse_args(argv)
    data = load(a.paths)
    sessions = sorted({s for s, _, _ in data})
    if len(data) < 20 or len(sessions) < 2:
        print(json.dumps({"error": "need at least 20 direct-executed actions from 2+ sessions", "rows": len(data),
                          "sessions": len(sessions)}))
        return
    random.Random(a.seed).shuffle(sessions)
    k = max(1, int(round(len(sessions) * a.holdout)))
    test_s = set(sessions[:k])
    train = [d for d in data if d[0] not in test_s]
    test = [d for d in data if d[0] in test_s]
    # calibration split inside training sessions
    tr_s = sorted({s for s, _, _ in train})
    cal_s = set(tr_s[: max(1, len(tr_s) // 4)]) if len(tr_s) > 2 else set()
    fit_rows = [d for d in train if d[0] not in cal_s] or train
    cal_rows = [d for d in train if d[0] in cal_s] or train
    w = fit(fit_rows, l2=a.l2, init=PRIOR_WEIGHTS)
    A, B = platt([logit(w, f) for _, f, _ in cal_rows], [y for _, _, y in cal_rows])
    ys = [y for _, _, y in test]
    res = {
        "rows": len(data), "sessions": len(sessions), "test_sessions": len(test_s),
        "prior_on_test": metrics([sigmoid(logit(PRIOR_WEIGHTS, f)) for _, f, _ in test], ys),
        "fitted_on_test": metrics([sigmoid(logit(w, f)) for _, f, _ in test], ys),
        "fitted_calibrated_on_test": metrics([sigmoid(A * logit(w, f) + B) for _, f, _ in test], ys),
    }
    json.dump({"name": os.path.splitext(os.path.basename(a.out))[0], "weights": w, "calib": {"a": A, "b": B},
               "eval": res}, open(a.out, "w"), indent=1)
    print(json.dumps(res, indent=1))


if __name__ == "__main__":
    main()
