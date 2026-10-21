# collect2.py TASK OUTPREFIX : gate screenshot (what Pi sees) + gate element frames
import json, sys, time, base64, os
sys.argv_saved = list(sys.argv)
exec(open('/data/workspace/shotexp/gg.py').read().split('if __name__')[0])
task, out = sys.argv_saved[1], sys.argv_saved[2]
time.sleep(7)
lw = call("list_windows", {"on_screen_only": True})["structuredContent"]["windows"]
cand = [w for w in lw if w["app_name"] not in ("Xfdesktop", "Xfce4-panel") and w["width"] > 300]
top = max(cand, key=lambda w: (w["z_index"] if w["z_index"] is not None else -1))
w = max([c for c in cand if c["pid"] == top["pid"]], key=lambda c: c["width"] * c["height"])
r = call("get_window_state", {"pid": w["pid"], "window_id": w["window_id"], "tree_format": "elements", "max_elements": 3000})
img = None; meta = {}
for c in r.get("content", []):
    if c.get("type") == "image": img = c
sc = r.get("structuredContent", {})
if img:
    open(out + ".png", "wb").write(base64.b64decode(img["data"]))
els = sc.get("elements", [])
keep = {k: v for k, v in sc.items() if k not in ("elements", "tree_markdown")}
json.dump({"task": task, "window": w, "meta": keep, "mime": img and img.get("mimeType"), "elements": els}, open(out + ".gate.json", "w"))
print(task, w["app_name"], w["width"], w["height"], img and img.get("mimeType"), len(img["data"])*3//4 if img else 0, len(els), list(keep)[:12], flush=True)
