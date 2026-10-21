"""One package: `pip install gui-gate` gives the gate, Cua Driver and batch mode in one MCP server.

Cua Driver comes in as the `cua-driver` package (its official, prebuilt, MIT-licensed wheels for macOS,
Linux and Windows, pinned to the version gui-gate is tested with), so nothing is downloaded or compiled at
run time and there is no second install step. This module only finds that binary, wires the server into
MCP clients (`gui-gate setup`) and checks the machine end to end (`gui-gate doctor`).
"""
from __future__ import annotations

import json
import os
import platform
import shutil
import subprocess
import sys
from typing import Optional

DRIVER_VERSION = "0.37.0"  # keep equal to the cua-driver pin in pyproject.toml
CLIENTS = ("claude", "codex", "cursor", "pi", "print")


def log(msg: str) -> None:
    sys.stderr.write(f"[gui-gate] {msg}\n")
    sys.stderr.flush()


def bundled_driver() -> Optional[str]:
    """The binary inside the cua-driver package installed with gui-gate."""
    try:
        from cua_driver.wrapper import get_binary_path  # type: ignore
        p = str(get_binary_path())
        return p if os.path.exists(p) else None
    except Exception:
        return None


def find_driver() -> Optional[str]:
    """GUI_GATE_DRIVER, then the bundled binary, then a cua-driver already on this machine."""
    env = os.environ.get("GUI_GATE_DRIVER")
    if env:
        return env
    b = bundled_driver()
    if b:
        return b
    p = shutil.which("cua-driver")
    if p:
        return p
    home = os.path.expanduser("~")
    for c in (os.path.join(home, ".local", "bin", "cua-driver"),
              os.path.join(home, ".cua-driver", "packages", "current", "cua-driver")):
        if os.path.exists(c):
            return c
    return None


def driver_version(path: str) -> Optional[str]:
    try:
        out = subprocess.run([path, "--version"], capture_output=True, text=True, timeout=15).stdout
    except Exception:
        return None
    parts = out.strip().split()
    return parts[-1] if parts else None


def ensure_driver() -> Optional[str]:
    return find_driver()


# ------------------------------------------------------------------ MCP client configuration
def server_command() -> list[str]:
    exe = shutil.which("gui-gate")
    return [exe] if exe else [sys.executable, "-m", "gui_gate.server"]


def snippet(name: str = "gui") -> str:
    cmd = server_command()
    return json.dumps({"mcpServers": {name: {"command": cmd[0], "args": cmd[1:]}}}, indent=2)


def _merge_json(path: str, name: str, entry: dict) -> str:
    data = {}
    if os.path.exists(path):
        with open(path, encoding="utf-8") as f:
            try:
                data = json.load(f)
            except json.JSONDecodeError:
                return f"{path} is not valid JSON; add this yourself:\n{snippet(name)}"
        shutil.copyfile(path, path + ".bak")
    data.setdefault("mcpServers", {})[name] = entry
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)
    return f"wrote {path}"


def configure(client: str, name: str = "gui", home: Optional[str] = None) -> str:
    home = home or os.path.expanduser("~")
    cmd = server_command()
    if client == "print":
        return snippet(name)
    if client == "cursor":
        return _merge_json(os.path.join(home, ".cursor", "mcp.json"), name, {"command": cmd[0], "args": cmd[1:]})
    if client == "codex":
        path = os.path.join(home, ".codex", "config.toml")
        os.makedirs(os.path.dirname(path), exist_ok=True)
        old = open(path, encoding="utf-8").read() if os.path.exists(path) else ""
        if f"[mcp_servers.{name}]" in old:
            return f"{path} already has [mcp_servers.{name}]"
        if old:
            shutil.copyfile(path, path + ".bak")
        block = (f"\n[mcp_servers.{name}]\ncommand = {json.dumps(cmd[0])}\n"
                 f"args = {json.dumps(cmd[1:])}\nstartup_timeout_sec = 120\n")
        with open(path, "a", encoding="utf-8") as f:
            f.write(block)
        return f"wrote {path}"
    if client in ("claude", "pi"):
        cli = shutil.which(client)
        if not cli:
            return f"`{client}` is not on PATH; add the server yourself:\n{snippet(name)}"
        args = [cli, "mcp", "add", name, "--", *cmd]
        r = subprocess.run(args, capture_output=True, text=True)
        return (r.stdout + r.stderr).strip() or f"{client} mcp add {name}: exit {r.returncode}"
    return f"unknown client {client}; choose one of {', '.join(CLIENTS)}"


# ------------------------------------------------------------------ checks
def doctor(driver: Optional[str] = None, cmd: Optional[list] = None) -> dict:
    """Real end-to-end check: start the driver's MCP server, initialize, list tools."""
    from .jsonrpc import ChildMCP
    rep = {"platform": f"{platform.system()} {platform.machine()}", "python": platform.python_version(),
           "bundled_driver": bundled_driver() is not None}
    d = driver or (None if cmd else find_driver())
    rep["driver"] = d or (" ".join(cmd) if cmd else None)
    rep["driver_version"] = driver_version(d) if d else None
    rep["driver_tools"] = None
    if d or cmd:
        child = None
        try:
            child = ChildMCP(cmd or [d, "mcp"], lambda m: None)
            init = child.call("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                             "clientInfo": {"name": "gui-gate-doctor", "version": "0"}}, timeout=30)
            if "error" in init:
                raise RuntimeError(init["error"].get("message"))
            child.forward({"jsonrpc": "2.0", "method": "notifications/initialized"})
            tl = child.call("tools/list", {}, timeout=30)
            rep["driver_tools"] = len((tl.get("result") or {}).get("tools") or [])
            rep["driver_server"] = ((init.get("result") or {}).get("serverInfo") or {}).get("name")
        except Exception as e:
            rep["driver_error"] = repr(e)
        finally:
            if child is not None:
                child.close()
    if platform.system() == "Linux":
        rep["display"] = bool(os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"))
        rep["at_spi"] = bool(shutil.which("at-spi-bus-launcher") or os.path.exists("/usr/libexec/at-spi-bus-launcher")
                             or os.path.exists("/usr/lib/at-spi2-core/at-spi-bus-launcher"))
    rep["ok"] = bool(rep["driver_tools"])
    return rep


def setup_main(argv=None) -> int:
    import argparse
    ap = argparse.ArgumentParser(prog="gui-gate setup", description="add gui-gate to your MCP client(s) and check it works")
    ap.add_argument("--client", action="append", choices=CLIENTS, default=[],
                    help="configure this MCP client (repeatable); default: print a config snippet")
    ap.add_argument("--name", default="gui", help="MCP server name in the client config (default gui)")
    a = ap.parse_args(argv)
    for c in (a.client or ["print"]):
        print(configure(c, a.name))
    rep = doctor()
    print(json.dumps(rep, indent=2))
    if not rep["ok"]:
        log("not ready: see the report above")
        return 1
    log(f"ready: {rep['driver_tools']} driver tools behind the gate, single-step and batch (run_actions)")
    return 0


def doctor_main(argv=None) -> int:
    rep = doctor()
    print(json.dumps(rep, indent=2))
    return 0 if rep["ok"] else 1
