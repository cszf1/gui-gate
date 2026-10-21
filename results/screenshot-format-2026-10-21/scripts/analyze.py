import json, statistics as st, sys, collections
O = "/data/workspace/shotexp" if len(sys.argv) < 2 else sys.argv[1]
Q = json.load(open(O + "/questions.json")); rows = [json.loads(l) for l in open(O + "/results_main.jsonl")]
F = Q["formats"]; SH = Q["shots"]
def med(x): return round(st.median(x), 1) if x else None
def mean(x): return round(st.mean(x), 1) if x else None
out = []
out.append("| format | bytes/image mean (KB) | click hit (1 img) | text exact (1 img) | text case-insens. | all (1 img) | all (3 img) | TTFT 1 img mean / p50 s | total 1 img mean s | TTFT 3 img mean / p50 s | total 3 img mean s | prompt tok/image | failed attempts (429/503, retried) |")
out.append("|---|---|---|---|---|---|---|---|---|---|---|---|---|")
LAT = []
summ = {}
for f in F:
    kb = mean([v["sizes"][f] / 1024 for v in SH.values()])
    r1 = [r for r in rows if r["fmt"] == f and r["n_img"] == 1 and not r["err"]]
    r3 = [r for r in rows if r["fmt"] == f and r["n_img"] == 3 and not r["err"]]
    err = sum(1 for r in rows if r["fmt"] == f and r["err"])
    c = [r for r in r1 if r["kind"] == "click"]; t = [r for r in r1 if r["kind"] == "text"]
    acc = lambda rr, k="ok": f"{sum(1 for r in rr if r.get(k))}/{len(rr)} ({100*sum(1 for r in rr if r.get(k))/max(len(rr),1):.0f}%)"
    pt = [r["usage"]["prompt_tokens"] for r in r1 if r.get("usage") and r["usage"].get("prompt_tokens")]
    t1 = [r["ttft"] for r in r1 if r["ttft"]]; t3 = [r["ttft"] for r in r3 if r["ttft"]]
    out.append(f"| {f} | {kb} | {acc(c)} | {acc(t)} | {acc(t,'ok_ci')} | {acc(r1)} | {acc(r3)} | {mean(t1)} / {med(t1)} | {mean([r['total'] for r in r1])} | {mean(t3)} / {med(t3)} | {mean([r['total'] for r in r3])} | {med(pt)} | {err} |")
    cold = lambda rr: [r for r in rr if r.get("usage") and not ((r["usage"].get("prompt_tokens_details") or {}).get("cached_tokens"))]
    c1, c3 = cold(r1), cold(r3)
    LAT.append(f"| {f} | {len(c1)} | {mean([r['ttft'] for r in c1 if r['ttft']])} / {med([r['ttft'] for r in c1 if r['ttft']])} | {mean([r['total'] for r in c1])} | {len(c3)} | {mean([r['ttft'] for r in c3 if r['ttft']])} / {med([r['ttft'] for r in c3 if r['ttft']])} | {mean([r['total'] for r in c3])} | {mean([r['usage']['completion_tokens'] for r in r1 if r.get('usage')])} |")
    summ[f] = {"cold_ttft1": mean([r['ttft'] for r in c1 if r['ttft']]), "cold_ttft3": mean([r['ttft'] for r in c3 if r['ttft']]),"kb": kb, "acc1": sum(1 for r in r1 if r["ok"]) / max(len(r1), 1), "ttft1": mean(t1), "ttft3": mean(t3)}
out.append("\nCold calls only (provider reported 0 cached prompt tokens, i.e. this image was not served from the prefix cache):")
out.append("| format | n (1 img) | TTFT 1 img mean / p50 s | total 1 img mean s | n (3 img) | TTFT 3 img mean / p50 s | total 3 img mean s | output tok mean (1 img, mostly reasoning) |")
out.append("|---|---|---|---|---|---|---|---|")
out += LAT
# paired vs png: per (question, n_img, rep)
key = lambda r: (r["q"], r["n_img"], r["rep"])
base = {key(r): r["ok"] for r in rows if r["fmt"] == "png" and not r["err"]}
out.append("\nPaired vs PNG (same question, same image count, same repeat): format right & PNG wrong / format wrong & PNG right")
for f in F:
    if f == "png": continue
    a = b = n = 0
    for r in rows:
        if r["fmt"] != f or r["err"] or key(r) not in base: continue
        n += 1; a += (r["ok"] and not base[key(r)]); b += (not r["ok"] and base[key(r)])
    out.append(f"- {f}: n={n}, +{a} / -{b}")
# per app
out.append("\nAccuracy by app (1 image, all repeats):")
apps = sorted(set(v["app"] for v in SH.values()))
out.append("| format | " + " | ".join(apps) + " |"); out.append("|---" * (len(apps) + 1) + "|")
qshot = {q["id"]: q["shot"] for q in Q["questions"]}
for f in F:
    cells = []
    for a in apps:
        rr = [r for r in rows if r["fmt"] == f and r["n_img"] == 1 and not r["err"] and SH[qshot[r["q"]]]["app"] == a]
        cells.append(f"{sum(1 for r in rr if r['ok'])}/{len(rr)}")
    out.append(f"| {f} | " + " | ".join(cells) + " |")
s = "\n".join(out); print(s); open(O + "/table.md", "w").write(s + "\n"); json.dump(summ, open(O + "/summary.json", "w"), indent=1)
