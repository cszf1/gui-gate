#!/usr/bin/env python3
"""Facts about Pi's default model, for run_bench.sh.

  pi_model_info.py text_only   -> prints 1 when the default model's models.json entry has no "image" input, else 0
  pi_model_info.py name        -> provider/model

PI_AGENT_DIR overrides ~/.pi/agent (tests)."""
import json
import os
import sys


def load(d):
    try:
        return json.load(open(os.path.join(d, "settings.json"))), json.load(open(os.path.join(d, "models.json")))
    except (OSError, ValueError):
        return {}, {}


def model_entry(settings, models):
    prov, mid = settings.get("defaultProvider", ""), settings.get("defaultModel", "")
    for m in ((models.get("providers") or {}).get(prov) or {}).get("models") or []:
        if m.get("id") == mid:
            return m
    return None


def main(argv):
    d = os.environ.get("PI_AGENT_DIR", os.path.expanduser("~/.pi/agent"))
    s, m = load(d)
    what = argv[1] if len(argv) > 1 else "text_only"
    if what == "name":
        print(f"{s.get('defaultProvider', '')}/{s.get('defaultModel', '')}")
        return 0
    e = model_entry(s, m)
    # unknown model (built-in provider list, no models.json entry): assume it can see images, as before
    print(1 if e is not None and "image" not in (e.get("input") or ["text", "image"]) else 0)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
