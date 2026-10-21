# usage: collect.py TASKJSON OUTPREFIX  -> OUTPREFIX.png + OUTPREFIX.atspi.json (screen-coord bboxes)
import json, sys, time, subprocess
import pyatspi
from PIL import ImageGrab
task, out = sys.argv[1], sys.argv[2]
time.sleep(float(sys.argv[3]) if len(sys.argv) > 3 else 6)
im = ImageGrab.grab(); im.save(out + ".png")
W, H = im.size
items = []
def walk(a, depth, app):
    if depth > 40 or len(items) > 6000: return
    try:
        st = a.getState()
        if depth > 0 and not st.contains(pyatspi.STATE_SHOWING): return
        role = a.getRoleName(); name = (a.name or "").strip()
        try:
            e = a.queryComponent().getExtents(pyatspi.DESKTOP_COORDS); bb = [e.x, e.y, e.width, e.height]
        except Exception: bb = None
        txt = None
        try:
            t = a.queryText(); n = t.characterCount
            if 0 < n <= 60: txt = t.getText(0, n)
        except Exception: pass
        if bb and bb[2] > 0 and bb[3] > 0 and bb[0] >= 0 and bb[1] >= 0 and bb[0]+bb[2] <= W and bb[1]+bb[3] <= H:
            items.append({"app": app, "role": role, "name": name, "text": txt, "bbox": bb, "depth": depth,
                          "focusable": st.contains(pyatspi.STATE_FOCUSABLE)})
        n = a.childCount
        if role in ("table", "tree table") and n > 400: n = 0  # handled via cells below
        for i in range(min(n, 400)):
            walk(a.getChildAtIndex(i), depth + 1, app)
        if role == "table":
            try:
                tb = a.queryTable()
                for r in range(min(tb.nRows, 30)):
                    for c in range(min(tb.nColumns, 12)):
                        walk(tb.getAccessibleAt(r, c), depth + 1, app)
            except Exception: pass
    except Exception: pass
d = pyatspi.Registry.getDesktop(0)
for i in range(d.childCount):
    app = d.getChildAtIndex(i)
    if app is None: continue
    walk(app, 0, app.name)
json.dump({"task": task, "size": [W, H], "items": items}, open(out + ".atspi.json", "w"))
print(out, W, H, len(items))
