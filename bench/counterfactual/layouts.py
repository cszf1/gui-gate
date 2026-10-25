#!/usr/bin/env python3
"""Held-out layout variants of the fixture twin: the UNSEEN-LAYOUT TEST SET.

  layouts.py write OUT.json [--n 12] [--seed 2026]     -> the frozen unseen-layout set (with its hash)
  layouts.py check OUT.json                           -> verify the hash (exit 1 when edited)

The original GateFixture layout (fruit captions in a fixed order) is the DEVELOPMENT layout: the scripted controller,
the costs, the fixed policy pi, any predictor and any rule may be tuned on it (and on the dev families of OSWorld).
A variant changes what a tuned rule could have memorised: the caption vocabulary (none shared with the dev layout),
the initial button order, the hazard profile, and (sim backend only) the number of buttons. Variants are generated
from a seed that is NOT used anywhere else and are marked

    split = "unseen_layout_test", use = "final evaluation only"

Nothing that is tuned - thresholds, value-model weights, calibrators, memory, app hints, gate rules - may be fitted,
selected or debugged on them; bench/leakage_lint.py checks that their ids and captions do not appear in those
artefacts. GateBackend (the real gate) expects 8 buttons, so the frozen set keeps 8; n_buttons != 8 is sim-only.
"""
import copy
import hashlib
import json
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
DEV_VOCAB = ["Apple", "Banana", "Cherry", "Grape", "Kiwi", "Lemon", "Mango", "Peach"]  # fixture_env.FRUITS
VOCABS = {
    "colors": ["Amber", "Azure", "Coral", "Indigo", "Ivory", "Olive", "Scarlet", "Teal", "Violet", "Ochre"],
    "cities": ["Oslo", "Lima", "Cairo", "Quito", "Perth", "Dakar", "Hanoi", "Porto", "Kyiv", "Tunis"],
    "tools": ["Hammer", "Wrench", "Chisel", "Pliers", "Saw", "Drill", "Rasp", "Awl", "Vise", "Level"],
    "animals": ["Otter", "Heron", "Bison", "Lynx", "Gecko", "Moose", "Raven", "Tapir", "Okapi", "Ibex"],
    "planets": ["Mercury", "Venus", "Earth", "Mars", "Jupiter", "Saturn", "Uranus", "Neptune", "Ceres", "Pluto"],
    "instruments": ["Cello", "Oboe", "Harp", "Flute", "Tuba", "Viola", "Banjo", "Sitar", "Lute", "Bugle"],
}
HAZARDS = ["relabel", "loading", "popup", "mixed"]
UNSEEN_SEED = 2026  # used only here
SPLIT = "unseen_layout_test"
_REGISTRY = {}


def variant(seed, vocab_name, hazards, n_buttons=8):
    rng = random.Random(f"{seed}:{vocab_name}:{hazards}:{n_buttons}")
    pool = VOCABS[vocab_name]
    assert 2 <= n_buttons <= len(pool)
    vocab = rng.sample(pool, n_buttons)
    order = list(range(n_buttons))
    rng.shuffle(order)
    assert not set(vocab) & set(DEV_VOCAB), "a held-out layout must not reuse a dev caption"
    lid = f"{vocab_name}-{hazards}-n{n_buttons}-{seed}"
    return {"id": lid, "vocab": vocab, "order": order, "hazards": hazards, "n_buttons": n_buttons, "split": SPLIT}


def unseen_layout_set(n=12, seed=UNSEEN_SEED, n_buttons=8):
    rng = random.Random(seed)
    names = sorted(VOCABS)
    out = []
    for i in range(n):
        out.append(variant(seed + i, names[i % len(names)], HAZARDS[rng.randrange(len(HAZARDS))], n_buttons))
    return out


def frozen(layouts):
    core = {"split": SPLIT, "use": "final evaluation only; never for tuning, calibration, rule writing or debugging",
            "dev_layout": {"vocab": DEV_VOCAB, "note": "the original GateFixture layout; tuning allowed"},
            "layouts": copy.deepcopy(layouts)}
    core["hash"] = hashlib.sha256(json.dumps({k: v for k, v in core.items() if k != "hash"}, sort_keys=True).encode()).hexdigest()
    return core


def check(doc):
    return doc.get("hash") == hashlib.sha256(json.dumps({k: v for k, v in doc.items() if k != "hash"}, sort_keys=True).encode()).hexdigest()


def get(layout_id):
    if not _REGISTRY:
        for lay in unseen_layout_set():
            _REGISTRY[lay["id"]] = lay
    if layout_id not in _REGISTRY:
        # any id of the form vocab-hazards-nK-seed can be rebuilt deterministically
        vocab_name, hazards, nk, seed = layout_id.split("-")
        _REGISTRY[layout_id] = variant(int(seed), vocab_name, hazards, int(nk[1:]))
    return _REGISTRY[layout_id]


def families(layouts=None):
    return ["layout:" + x["id"] for x in (layouts or unseen_layout_set())]


def main(a):
    if not a or a[0] not in ("write", "check"):
        print(__doc__)
        return 2
    if a[0] == "write":
        flags = {a[i]: a[i + 1] for i in range(2, len(a) - 1) if a[i].startswith("--")}
        doc = frozen(unseen_layout_set(int(flags.get("--n", 12)), int(flags.get("--seed", UNSEEN_SEED))))
        json.dump(doc, open(a[1], "w"), indent=1)
        print(doc["hash"])
        return 0
    ok = check(json.load(open(a[1])))
    print("ok" if ok else "hash mismatch: the unseen-layout set was edited")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
