#!/usr/bin/env python3
"""Statistics for fair baseline comparisons (stdlib only, deterministic with a seed).

wilson(k, n)                    -> (lo, hi) score interval of a binomial proportion
paired_bootstrap(a, b)          -> mean difference b - a over paired units with a percentile CI (resampling units)
mcnemar_exact(a, b)             -> two-sided exact McNemar p for paired binary outcomes
paired_permutation(a, b)        -> two-sided sign-flip permutation p for the mean paired difference
budget_curve(runs, budgets)     -> success-within-budget curve per policy (a run counts only if it succeeded AND its
                                   cost <= budget; failures and timeouts are never dropped)
"""
import math
import random


def wilson(k, n, z=1.959963984540054):
    if n <= 0:
        return (0.0, 1.0)
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (max(0.0, c - h), min(1.0, c + h))


def _quantile(xs, q):
    v = sorted(xs)
    if not v:
        return float("nan")
    pos = q * (len(v) - 1)
    i = int(math.floor(pos))
    f = pos - i
    return v[i] * (1 - f) + v[i + 1] * f if i + 1 < len(v) else v[i]


def paired_bootstrap(a, b, reps=2000, seed=0, level=0.95):
    """a, b: equal-length lists of per-unit values (e.g. task success rates over repeats). Returns dict."""
    assert len(a) == len(b), "paired samples must have the same units"
    n = len(a)
    if n == 0:
        return {"n": 0, "diff": None, "ci": [None, None]}
    d = [y - x for x, y in zip(a, b)]
    rng = random.Random(seed)
    boots = []
    for _ in range(reps):
        s = [d[rng.randrange(n)] for _ in range(n)]
        boots.append(sum(s) / n)
    al = (1 - level) / 2
    return {"n": n, "diff": sum(d) / n, "ci": [_quantile(boots, al), _quantile(boots, 1 - al)], "level": level, "reps": reps}


def _binom_cdf(k, n):
    return sum(math.comb(n, i) for i in range(0, k + 1)) / 2 ** n


def mcnemar_exact(a, b):
    """a, b: paired 0/1 outcomes. Discordant pairs: n01 (a=0, b=1), n10 (a=1, b=0). Exact two-sided binomial p."""
    n01 = sum(1 for x, y in zip(a, b) if not x and y)
    n10 = sum(1 for x, y in zip(a, b) if x and not y)
    m = n01 + n10
    if m == 0:
        return {"n01": 0, "n10": 0, "p": 1.0}
    p = min(1.0, 2 * _binom_cdf(min(n01, n10), m))
    return {"n01": n01, "n10": n10, "p": p}


def paired_permutation(a, b, reps=10000, seed=0):
    """Sign-flip permutation test of mean(b - a) = 0; exact when 2^n <= reps."""
    d = [y - x for x, y in zip(a, b)]
    n = len(d)
    if n == 0:
        return {"n": 0, "p": 1.0}
    obs = abs(sum(d))
    nz = [x for x in d if x != 0]
    if not nz:
        return {"n": n, "p": 1.0, "exact": True}
    if 2 ** len(nz) <= reps:
        hits = tot = 0
        for mask in range(2 ** len(nz)):
            s = sum(x if (mask >> i) & 1 else -x for i, x in enumerate(nz))
            hits += abs(s) >= obs - 1e-12
            tot += 1
        return {"n": n, "p": hits / tot, "exact": True}
    rng = random.Random(seed)
    hits = 0
    for _ in range(reps):
        s = sum(x if rng.random() < 0.5 else -x for x in nz)
        hits += abs(s) >= obs - 1e-12
    return {"n": n, "p": (hits + 1) / (reps + 1), "exact": False}


def budget_curve(runs, budgets, cost_key="wall_s"):
    """runs: dicts with policy, success, and the cost key. Returns {policy: [(budget, rate, lo, hi, n)]}.
    Every run counts in the denominator, including failures, timeouts and runs with a missing cost (= over budget)."""
    out = {}
    pols = sorted({r["policy"] for r in runs})
    for p in pols:
        rs = [r for r in runs if r["policy"] == p]
        pts = []
        for bdg in budgets:
            k = sum(1 for r in rs if r.get("success") and r.get(cost_key) is not None and r[cost_key] <= bdg)
            lo, hi = wilson(k, len(rs))
            pts.append((bdg, k / len(rs) if rs else 0.0, lo, hi, len(rs)))
        out[p] = pts
    return out
