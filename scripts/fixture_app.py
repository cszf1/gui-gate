#!/usr/bin/env python3
"""GTK3 test app with realistic GUI hazards, for exercising the gate on a real
desktop through Cua Driver (AT-SPI + X11).

Task: click the buttons in the order shown in "Sequence: ...".
Hazards after each click (seeded):
  relabel  - button captions are shuffled (same widgets, new labels)
  loading  - all buttons disabled for ~0.8 s with a "Loading..." caption
  popup    - a modal "Notice" dialog opens; main-window clicks are ignored until OK

Ground truth (which caption was really clicked) goes to --log as JSONL. The gate
never reads this file; it is only for scoring runs.
"""
import argparse
import json
import random
import time

import gi

gi.require_version("Gtk", "3.0")
from gi.repository import GLib, Gtk  # noqa: E402

FRUITS = ["Apple", "Banana", "Cherry", "Grape", "Kiwi", "Lemon", "Mango", "Peach"]

ap = argparse.ArgumentParser()
ap.add_argument("--seed", type=int, default=0)
ap.add_argument("--length", type=int, default=10)
ap.add_argument("--p-relabel", type=float, default=0.25)
ap.add_argument("--p-loading", type=float, default=0.2)
ap.add_argument("--p-popup", type=float, default=0.12)
ap.add_argument("--log", default="/tmp/fixture.jsonl")
a = ap.parse_args()
rng = random.Random(a.seed)
seq = [rng.choice(FRUITS) for _ in range(a.length)]
logf = open(a.log, "a")


def log(**ev):
    logf.write(json.dumps({"t": round(time.time(), 3), "seed": a.seed, **ev}) + "\n")
    logf.flush()


win = Gtk.Window(title="GateFixture")
win.set_default_size(560, 360)
box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, margin=12)
win.add(box)
box.pack_start(Gtk.Label(label="Sequence: " + ", ".join(seq)), False, False, 0)
status = Gtk.Label(label=f"Progress 0/{len(seq)}")
box.pack_start(status, False, False, 0)
grid = Gtk.Grid(row_spacing=6, column_spacing=6)
box.pack_start(grid, True, True, 0)
st = {"k": 0, "busy": False, "popup": None, "labels": FRUITS[:]}
buttons = []


def apply_labels():
    for b, lab in zip(buttons, st["labels"]):
        b.set_label("Loading..." if st["busy"] else lab)
        b.set_sensitive(not st["busy"])


def end_busy():
    st["busy"] = False
    apply_labels()
    return False


def close_popup(dlg, _resp):
    dlg.destroy()
    st["popup"] = None
    log(ev="popup_closed")


def hazards():
    if rng.random() < a.p_relabel:
        rng.shuffle(st["labels"])
        log(ev="relabel", labels=st["labels"])
    if rng.random() < a.p_loading:
        st["busy"] = True
        log(ev="loading")
        GLib.timeout_add(800, end_busy)
    apply_labels()
    if rng.random() < a.p_popup and st["popup"] is None:
        d = Gtk.MessageDialog(transient_for=win, modal=True, message_type=Gtk.MessageType.INFO,
                              buttons=Gtk.ButtonsType.OK, text="Notice")
        d.set_title("Notice")
        d.connect("response", close_popup)
        d.show_all()
        st["popup"] = d
        log(ev="popup_open")


def on_click(btn, i):
    shown = st["labels"][i]
    if st["busy"] or st["popup"] is not None:
        log(ev="ignored", caption=shown, why="busy" if st["busy"] else "popup")
        return
    if st["k"] >= len(seq):
        log(ev="click", caption=shown, expected=None, correct=False)
        return
    exp = seq[st["k"]]
    ok = shown == exp
    log(ev="click", caption=shown, expected=exp, correct=ok, k=st["k"])
    if ok:
        st["k"] += 1
        status.set_text(f"Progress {st['k']}/{len(seq)}")
        if st["k"] == len(seq):
            log(ev="done")
    hazards()


for i, lab in enumerate(FRUITS):
    b = Gtk.Button(label=lab)
    b.connect("clicked", on_click, i)
    grid.attach(b, i % 4, i // 4, 1, 1)
    buttons.append(b)

log(ev="start", sequence=seq)
win.connect("destroy", Gtk.main_quit)
win.show_all()
Gtk.main()
