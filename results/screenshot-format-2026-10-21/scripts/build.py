# Build questions + encoded images from raw3/<task>.png (gate screenshot, exactly what Pi sees) and
# raw3/<task>.full.atspi.json (AT-SPI walk taken right after it). Frames mapped to screenshot pixels:
#   sx = (x - win.x) * frame_scale,  sy = (y + off - win.y) * frame_scale
# LibreOffice reports menu-bar menus and the first-run infobar in desktop coords but everything else relative to the
# decorated toplevel (title bar 27 px) -> off = 27 for those (verified by drawing boxes on the screenshots).
import json, os, random, re, io, glob, sys
from PIL import Image
R = "/data/workspace/shotexp/raw3"; O = "/data/workspace/shotexp"
OFF = json.loads(os.environ.get("OFFSETS", '{"soffice": "lo", "default": 0}'))
os.makedirs(O + "/img", exist_ok=True)
CLICK_ROLES = {"push button", "toggle button", "menu", "page tab", "combo box", "check box", "radio button", "button"}
FORMATS = {"png": None, "jpg80": ("JPEG", 1.0, 80), "jpg60": ("JPEG", 1.0, 60), "jpg80half": ("JPEG", 0.5, 80), "webp80": ("WEBP", 1.0, 80)}
def inter(a, b): return not (a[0]+a[2] <= b[0] or b[0]+b[2] <= a[0] or a[1]+a[3] <= b[1] or b[1]+b[3] <= a[1])
qs = []; shots = {}
for p in sorted(glob.glob(R + "/ow*.gate.json")):
    g = json.load(open(p)); task = g["task"]
    fa = R + f"/{task}.full.atspi.json"
    if not os.path.exists(R + f"/{task}.png") or not os.path.exists(fa): continue
    m = g["meta"]; wb = m["window_bounds"]; s = m.get("frame_scale", 1.0)
    raw = open(R + f"/{task}.png", "rb").read(); im = Image.open(io.BytesIO(raw)).convert("RGB"); W, H = im.size
    sizes = {}
    for f, spec in FORMATS.items():
        if spec is None: data = raw
        else:
            fmt, sc, q = spec; x = im if sc == 1 else im.resize((round(W*sc), round(H*sc)), Image.LANCZOS)
            b = io.BytesIO(); x.save(b, fmt, quality=q); data = b.getvalue()
        open(f"{O}/img/{task}.{f}", "wb").write(data); sizes[f] = len(data)
    shots[task] = {"w": W, "h": H, "sizes": sizes, "app": g["window"]["app_name"]}
    items = json.load(open(fa))["items"]
    lo = "libreoffice" in g["window"]["app_name"]
    appname = "soffice" if lo else None
    # app process name: the one owning most items inside the window
    if not lo:
        from collections import Counter
        c = Counter(i["app"] for i in items if i["app"] not in ("xfce4-panel", "xfdesktop", "xfwm4"))
        appname = c.most_common(1)[0][0] if c else None
    allit = items
    owner = []; cur = None
    for k, i in enumerate(allit):
        if i["depth"] == 1: cur = k
        i["top"] = cur
    tops = {k: allit[k] for k in set(i["top"] for i in allit) if k is not None}
    items = [i for i in items if i["app"] == appname]
    # occlusion: drop elements of a bigger top-level window that lie under a smaller (dialog) window of the same app
    mine = [k for k, f in tops.items() if f["app"] == appname]
    def area(b): return b[2] * b[3]
    def occluded(i):
        for k in mine:
            if k == i["top"]: continue
            f = tops[k]["bbox"]
            if i["top"] in tops and area(f) < area(tops[i["top"]]["bbox"]) and inter(f, i["bbox"]): return True
        return False
    items = [i for i in items if not occluded(i)]
    info = [i for i in items if (i["text"] or i["name"] or "").startswith("You are running version")]
    rn = [i for i in items if (i["name"] or "").strip() == "Release Notes"]
    iy = info[0]["bbox"][1] if info else (rn[0]["bbox"][1] if rn else None)
    def mapb(i):
        x, y, w, h = i["bbox"]
        if lo:
            if iy is not None and iy - 12 <= y <= iy + 30: return None  # infobar: skip (mixed frames)
            off = 0 if (i["role"] == "menu" and y < 80) else 27
        else:
            off = OFF.get(appname, OFF["default"])
        b = [(x - wb["x"]) * s, (y + off - wb["y"]) * s, w * s, h * s]
        if b[0] < -3 or b[1] < -3 or b[0] + b[2] > W + 4 or b[1] + b[3] > H + 4: return None
        b = [max(b[0], 0), max(b[1], 0), min(b[2], W - max(b[0], 0)), min(b[3], H - max(b[1], 0))]
        return [round(v, 1) for v in b]
    for i in items: i["sb"] = mapb(i)
    items = [i for i in items if i["sb"]]
    rnd = random.Random(task)
    cands = []
    for i in items:
        n = (i["name"] or "").strip(); w, h = i["bbox"][2:]
        if i["role"] not in CLICK_ROLES or not (2 <= len(n) <= 30) or w < 8 or h < 8 or w*h > 40000: continue
        if lo and not ((i["role"] == "menu" and i["bbox"][1] < 80) or 50 <= i["bbox"][1] <= 140): continue  # verified frames only
        same = [j for j in items if (j["name"] or "").strip().lower() == n.lower()]
        if all(inter(j["sb"], i["sb"]) for j in same): cands.append(i)
    seen = set(); cands = [c for c in cands if not (c["name"].strip().lower() in seen or seen.add(c["name"].strip().lower()))]
    rnd.shuffle(cands)
    menus = [c for c in cands if c["role"] == "menu"]; other = [c for c in cands if c["role"] != "menu"]
    pick = (other[:2] + menus[:1]) if len(other) >= 2 else (other + menus)[:3]
    for c in pick:
        n = c["name"].strip()
        bbs = [j["sb"] for j in items if (j["name"] or "").strip().lower() == n.lower()]
        qs.append({"id": f"{task}#click#{n}", "shot": task, "kind": "click", "name": n, "role": c["role"], "bboxes": bbs})
    tc = []
    for i in items:
        t = i["text"] if i["role"] == "table cell" else (i["text"] or i["name"])
        if not t: continue
        t = " ".join(t.split()); w, h = i["bbox"][2:]
        if lo and i["role"] != "table cell" and (i["bbox"][1] < 840 or ("impress" in g["window"]["app_name"] and "%" in t)): continue
        if i["role"] == "table cell":
            if not re.fullmatch(r"-?\d{1,7}", t): continue  # displayed == stored only for plain integers
        elif i["role"] not in ("label", "status bar", "heading", "static", "text"): continue
        if not (2 <= len(t) <= 32) or not re.search(r"[A-Za-z0-9]", t) or w < 15 or h < 8: continue
        tc.append((t, i))
    rnd.shuffle(tc); cells = [z for z in tc if z[1]["role"] == "table cell"]; labs = [z for z in tc if z[1]["role"] != "table cell"]
    labs.sort(key=lambda z: not re.search(r"\d", z[0]))
    chosen = []; used = set()
    for t, i in (cells[:1] + labs + cells[1:]):
        if t in used: continue
        used.add(t); chosen.append((t, i))
        if len(chosen) >= 2: break
    for t, i in chosen:
        qs.append({"id": f"{task}#text#{t}", "shot": task, "kind": "text", "truth": t, "role": i["role"], "bbox": i["sb"]})
json.dump({"shots": shots, "questions": qs, "formats": list(FORMATS)}, open(O + "/questions.json", "w"), indent=1)
from collections import Counter
print(len(shots), "shots", len(qs), "questions", Counter(q["kind"] for q in qs))
for t, v in shots.items(): print(t, v)
for q in qs: print(q["kind"], q["shot"][:12], q.get("name") or repr(q.get("truth")), q["role"], q.get("bbox") or q["bboxes"][0])
