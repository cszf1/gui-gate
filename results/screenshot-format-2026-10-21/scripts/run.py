# run.py MODE  (MODE: probe | main)  -> results.jsonl
import json, base64, time, urllib.request, random, re, sys, threading, os
from concurrent.futures import ThreadPoolExecutor
O = "/data/workspace/shotexp"; MODEL = os.environ.get("MODEL", "deepseek-v4.1-flash-1017")
K = open("/data/home/.pi/agent/tokenrhythm.key").read().strip()
Q = json.load(open(O + "/questions.json"))
MIME = {"png": "image/png", "jpg80": "image/jpeg", "jpg60": "image/jpeg", "jpg80half": "image/jpeg", "webp80": "image/webp"}
SC = {"jpg80half": 0.5}
SH = Q["shots"]; shots = sorted(SH)
def img(task, f): return base64.b64encode(open(f"{O}/img/{task}.{f}", "rb").read()).decode()
def prompt(q, f, n):
    s = SC.get(f, 1.0); W, H = round(SH[q["shot"]]["w"]*s), round(SH[q["shot"]]["h"]*s)
    which = "this screenshot" if n == 1 else f"the LAST of the {n} screenshots (the earlier ones are older states of other apps)"
    if q["kind"] == "click":
        return (f"Look at {which} ({W}x{H} pixels). Give the pixel coordinates in that image where I should click "
                f"to activate the UI element named \"{q['name']}\" ({q['role']}). Reply with only: x,y")
    x, y, w, h = [round(v*s) for v in q["bbox"]]
    return (f"Look at {which} ({W}x{H} pixels). Read the text displayed inside the rectangle from ({x},{y}) to ({x+w},{y+h}). "
            f"Reply with only that text, exactly as displayed.")
def call(q, f, n, rep):
    for att in range(8):
        r = call1(q, f, n, rep)
        if r["err"] and "429" in r["err"]:
            time.sleep(4 + 3 * att); continue
        r["attempts_429"] = att
        return r
    return r
def call1(q, f, n, rep):
    others = [t for t in shots if t != q["shot"]]
    rnd = random.Random(q["id"]); ctx = rnd.sample(others, n-1) + [q["shot"]]
    content = [{"type": "image_url", "image_url": {"url": f"data:{MIME[f]};base64,{img(t, f)}"}} for t in ctx]
    content.append({"type": "text", "text": prompt(q, f, n)})
    body = json.dumps({"model": MODEL, "messages": [{"role": "user", "content": content}], "max_tokens": 6000,
                       "reasoning_effort": "medium", "stream": True, "stream_options": {"include_usage": True}}).encode()
    req = urllib.request.Request("https://tokenrhythm.studio/v1/chat/completions", body,
                                 {"Authorization": "Bearer " + K, "Content-Type": "application/json"})
    t0 = time.time(); ttft = None; out = ""; usage = None; err = None
    try:
        for line in urllib.request.urlopen(req, timeout=240):
            if not line.startswith(b"data:") or line.strip() == b"data: [DONE]": continue
            try: e = json.loads(line[5:])
            except Exception: continue
            if e.get("usage"): usage = e["usage"]
            for ch in e.get("choices") or []:
                dl = ch.get("delta") or {}
                if ttft is None and (dl.get("content") or dl.get("reasoning_content") or dl.get("reasoning")): ttft = time.time()-t0
                out += dl.get("content") or ""
    except Exception as ex:
        err = str(ex)[:200]
        try: err += " " + ex.read().decode()[:300]
        except Exception: pass
    tot = time.time()-t0
    r = {"q": q["id"], "kind": q["kind"], "fmt": f, "n_img": n, "rep": rep, "ttft": ttft, "total": tot, "answer": out.strip()[-300:],
         "usage": usage, "err": err, "img_bytes": sum(os.path.getsize(f"{O}/img/{t}.{f}") for t in ctx), "ts": t0}
    r.update(score(q, f, out) if not err else {"ok": None})
    return r
def norm(s): return " ".join(s.strip().strip('"\'`').split()).rstrip(":").strip()
def score(q, f, out):
    a = re.sub(r"<think>.*?</think>", "", out, flags=re.S).strip()
    if q["kind"] == "text":
        return {"ok": norm(a) == norm(q["truth"]), "ok_ci": norm(a).lower() == norm(q["truth"]).lower()}
    m = re.findall(r"(-?\d+(?:\.\d+)?)\s*[, ]\s*(-?\d+(?:\.\d+)?)", a)
    if not m: return {"ok": False, "xy": None}
    s = SC.get(f, 1.0); x, y = float(m[-1][0])/s, float(m[-1][1])/s; tol = 2
    hit = any(b[0]-tol <= x <= b[0]+b[2]+tol and b[1]-tol <= y <= b[1]+b[3]+tol for b in q["bboxes"])
    return {"ok": hit, "xy": [round(x), round(y)]}
if __name__ == "__main__":
    mode = sys.argv[1]; outp = O + f"/results_{mode}.jsonl"
    qs = Q["questions"]; fmts = Q["formats"]
    if mode == "probe":
        jobs = [(qs[0], f, 1, 0) for f in fmts]; conc = 5
    else:
        sub = qs[::3]  # 3-image subset: every third question
        jobs = [(q, f, 1, r) for q in qs for f in fmts for r in range(2)] + [(q, f, 3, r) for q in sub for f in fmts for r in range(2)]
        random.Random(0).shuffle(jobs); conc = int(os.environ.get("CONC", 4))
    done = set()
    if os.path.exists(outp):
        for l in open(outp):
            r = json.loads(l)
            if not r.get("err"): done.add((r["q"], r["fmt"], r["n_img"], r["rep"]))
    jobs = [j for j in jobs if (j[0]["id"], j[1], j[2], j[3]) not in done]
    lock = threading.Lock(); print("jobs", len(jobs), flush=True)
    def run(j):
        r = call(*j)
        with lock:
            open(outp, "a").write(json.dumps(r) + "\n")
        return r
    with ThreadPoolExecutor(conc) as ex:
        for i, r in enumerate(ex.map(run, jobs)):
            if mode == "probe" or i % 25 == 0:
                print(i, r["fmt"], r["n_img"], r["ok"], round(r["ttft"] or -1, 1), round(r["total"], 1), (r["err"] or r["answer"])[:80], flush=True)
    print("RUNDONE", flush=True)
