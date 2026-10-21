"""Cua Driver stays a separate package; gui-gate only depends on it, notices new releases and updates on request.

* pyproject: `cua-driver>=0.37.0,<1` (installed together with gui-gate, never vendored).
* TESTED is the driver version this gui-gate release was tested with. A newer installed driver works but
  is flagged as untested until gui-gate's own tests pass on it.
* The check is one HTTPS GET to PyPI, at most once a day (cached), in the background, 3 s timeout, never
  blocking the server. GUI_GATE_NO_UPDATE_CHECK=1 turns it off.
* Nothing is ever installed automatically. `gui-gate update` installs the newest driver only when the user
  runs it, checks it end to end (doctor + the tools the gate needs) and goes back to the previous version
  when the check fails.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import time
import urllib.request
from typing import Optional

TESTED = "0.37.0"
PYPI_URL = "https://pypi.org/pypi/cua-driver/json"
REQUIRED_DRIVER_TOOLS = {"get_window_state", "list_windows", "click", "type_text", "press_key"}
CACHE_TTL = 24 * 3600


def vtuple(v: Optional[str]) -> tuple:
    out = []
    for part in str(v or "").strip().lstrip("v").split("."):
        num = "".join(ch for ch in part if ch.isdigit())
        out.append(int(num) if num else 0)
        if not part.isdigit():
            break
    return tuple(out) or (0,)


def installed() -> Optional[str]:
    try:
        from importlib.metadata import version
        return version("cua-driver")
    except Exception:
        try:
            from .install import driver_version, find_driver
            d = find_driver()
            return driver_version(d) if d else None
        except Exception:
            return None


def _cache_path() -> str:
    base = os.environ.get("XDG_CACHE_HOME") or os.path.join(os.path.expanduser("~"), ".cache")
    return os.path.join(base, "gui-gate", "driver-latest.json")


def latest(timeout: float = 3.0, use_cache: bool = True) -> Optional[str]:
    if os.environ.get("GUI_GATE_LATEST_OVERRIDE"):  # tests / offline mirrors
        return os.environ["GUI_GATE_LATEST_OVERRIDE"]
    if os.environ.get("GUI_GATE_NO_UPDATE_CHECK"):
        return None
    p = _cache_path()
    if use_cache:
        try:
            with open(p) as f:
                c = json.load(f)
            if time.time() - c.get("t", 0) < CACHE_TTL:
                return c.get("latest")
        except Exception:
            pass
    try:
        req = urllib.request.Request(PYPI_URL, headers={"User-Agent": "gui-gate-update-check"})
        with urllib.request.urlopen(req, timeout=timeout) as r:
            v = json.load(r)["info"]["version"]
    except Exception:
        return None
    try:
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "w") as f:
            json.dump({"t": time.time(), "latest": v}, f)
    except Exception:
        pass
    return v


def status(use_cache: bool = True, inst: Optional[str] = None) -> dict:
    """inst: the version the running driver reported (serverInfo), which beats package metadata."""
    inst, lat = inst or installed(), latest(use_cache=use_cache)
    return {"installed": inst, "tested_with": TESTED, "latest": lat,
            "update_available": bool(inst and lat and vtuple(lat) > vtuple(inst)),
            "untested": bool(inst and vtuple(inst) > vtuple(TESTED))}


def note(st: dict) -> str:
    """One line for the user (via the agent / client), or '' when nothing to say."""
    if st.get("update_available"):
        return (f"Cua Driver {st['latest']} is available (installed {st['installed']}). "
                f"Tell the user they can run `gui-gate update`.")
    if st.get("untested"):
        return (f"Cua Driver {st['installed']} is newer than the version this gui-gate was tested with "
                f"({st['tested_with']}); if GUI actions misbehave, `gui-gate update --to {st['tested_with']}`.")
    return ""


def missing_tools(names) -> list:
    return sorted(REQUIRED_DRIVER_TOOLS - set(names or []))


def _pip(*args) -> int:
    return subprocess.run([sys.executable, "-m", "pip", "install", "-q", *args]).returncode


def update_main(argv=None) -> int:
    import argparse
    from .install import doctor
    ap = argparse.ArgumentParser(prog="gui-gate update", description="check for / install a newer Cua Driver")
    ap.add_argument("--check", action="store_true", help="only report, install nothing")
    ap.add_argument("--to", help="install this driver version (e.g. go back to the tested one)")
    ap.add_argument("-y", "--yes", action="store_true", help="do not ask before installing")
    a = ap.parse_args(argv)
    st = status(use_cache=False)
    print(json.dumps(st, indent=2))
    target = a.to or st.get("latest")
    if a.check or not target or (not a.to and not st["update_available"]):
        print(note(st) or "Cua Driver is up to date.")
        return 0
    if vtuple(target) >= (1,):
        print(f"Cua Driver {target} is a new major version; wait for a gui-gate release that supports it.")
        return 1
    if not a.yes:
        if input(f"Install Cua Driver {target} (now {st['installed']})? [y/N] ").strip().lower() not in ("y", "yes"):
            return 0
    prev = st["installed"]
    if _pip(f"cua-driver=={target}") != 0:
        print("pip could not install it. With pipx use: pipx inject gui-gate cua-driver==" + target)
        return 1
    rep = doctor()
    miss = rep.get("missing_tools") or []
    if rep.get("ok") and not miss:
        print(f"Cua Driver {target} installed and checked ({rep['driver_tools']} tools)."
              + ("" if vtuple(target) <= vtuple(TESTED) else f" Not yet tested with this gui-gate (tested {TESTED})."))
        return 0
    print(f"Cua Driver {target} failed the check ({rep.get('driver_error') or 'missing tools: ' + ', '.join(miss)}); "
          f"going back to {prev}.")
    if prev:
        _pip(f"cua-driver=={prev}")
    return 1
