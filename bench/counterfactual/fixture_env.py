"""A serialisable twin of scripts/fixture_app.py (GTK GateFixture) for strict counterfactual branching.

Same task and hazards as the GTK app: click the fruit buttons in the order of "Sequence: ..."; after each accepted
click, with the app's probabilities, captions are shuffled (relabel), all buttons show "Loading..." for 0.8 s
(loading), or a modal "Notice" dialog opens that swallows main-window clicks until OK (popup). Differences: time is
VIRTUAL (seconds advance only through `advance`), so a branch is reproducible, and the whole state - including the
random generator - is a JSON document, so a checkpoint can be saved, restored and hashed exactly. The GTK app cannot
be snapshotted mid-run; this twin is what the P1 harness branches.
"""
import copy
import hashlib
import json
import random

FRUITS = ["Apple", "Banana", "Cherry", "Grape", "Kiwi", "Lemon", "Mango", "Peach"]


class FixtureEnv:
    def __init__(self, seed=0, length=10, p_relabel=0.25, p_loading=0.2, p_popup=0.12, loading_s=0.8, layout=None):
        """layout (layouts.py): a held-out layout variant - other captions (vocab), another initial button order, and
        optionally another number of buttons. None = the original GateFixture layout (state identical to before)."""
        self.rng = random.Random(seed)
        vocab = FRUITS if layout is None else list(layout["vocab"])
        labels = FRUITS[:] if layout is None else [vocab[i] for i in layout["order"]]
        self.st = {
            "seed": seed, "length": length, "p": [p_relabel, p_loading, p_popup], "loading_s": loading_s,
            "seq": [self.rng.choice(vocab) for _ in range(length)], "k": 0, "labels": labels,
            "busy_until": 0.0, "popup": False, "t": 0.0, "clicks": 0, "wrong": 0, "ignored": 0,
        }
        if layout is not None:
            self.st["layout"] = layout["id"]

    # ---- state
    def dump(self):
        d = copy.deepcopy(self.st)
        v, internal, g = self.rng.getstate()
        d["rng"] = [v, list(internal), g]
        return d

    @classmethod
    def load(cls, d):
        e = cls.__new__(cls)
        d = copy.deepcopy(d)
        v, internal, g = d.pop("rng")
        e.rng = random.Random()
        e.rng.setstate((v, tuple(internal), g))
        e.st = d
        return e

    def state_hash(self):
        return hashlib.sha256(json.dumps(self.dump(), sort_keys=True, separators=(",", ":")).encode()).hexdigest()[:16]

    # ---- environment
    @property
    def busy(self):
        return self.st["t"] < self.st["busy_until"]

    @property
    def done(self):
        return self.st["k"] >= self.st["length"]

    def advance(self, s):
        self.st["t"] = round(self.st["t"] + max(0.0, s), 6)

    def observe(self, popup_visible=True):
        """What a read shows: captions (or Loading...), progress, and the dialog if the read can see it.
        A tree read of the main window (look_light) does not include the modal dialog; a full read does."""
        st = self.st
        return {"labels": ["Loading..." if self.busy else lab for lab in st["labels"]], "k": st["k"],
                "seq": list(st["seq"]), "popup": bool(st["popup"]) and popup_visible, "t": st["t"]}

    def probe(self, index, caption):
        """The gate's cheap tree probe: does the element still show this caption (and is it clickable)?"""
        return (not self.busy) and not self.st["popup"] and self.st["labels"][index] == caption

    def click(self, index):
        """Click button `index` (0-7) or the dialog's OK (index = "ok"). Returns the app's ground-truth event."""
        st = self.st
        st["clicks"] += 1
        if index == "ok":
            if st["popup"]:
                st["popup"] = False
                return {"ev": "popup_closed", "progress": False, "useful": True}
            return {"ev": "nothing", "progress": False, "useful": False}
        if self.busy or st["popup"]:
            st["ignored"] += 1
            return {"ev": "ignored", "why": "busy" if self.busy else "popup", "progress": False, "useful": False}
        shown = st["labels"][index]
        if self.done:
            st["wrong"] += 1
            return {"ev": "click", "caption": shown, "correct": False, "progress": False, "useful": False}
        exp = st["seq"][st["k"]]
        ok = shown == exp
        if ok:
            st["k"] += 1
        else:
            st["wrong"] += 1
        self._hazards()
        return {"ev": "click", "caption": shown, "expected": exp, "correct": ok, "progress": ok, "useful": ok}

    def _hazards(self):
        st = self.st
        pr, pl, pp = st["p"]
        if self.rng.random() < pr:
            self.rng.shuffle(st["labels"])
        if self.rng.random() < pl:
            st["busy_until"] = round(st["t"] + st["loading_s"], 6)
        if self.rng.random() < pp and not st["popup"]:
            st["popup"] = True
