"""OSWorld checkpoints for the P1 harness (CODE ONLY: never run in tests, no live run has used it yet).

A native OSWorld setup/reset (bench/osworld_native.py) is NOT an arbitrary mid-task snapshot. A checkpoint at step n
of a logged run is therefore rebuilt, per branch, as:
  1. osworld_native.py reset + setup TASK (the task's own config: files, apps, windows);
  2. a fresh gui-gate with the run's exact flags/env, replaying the logged action prefix (the Pi stream's tool calls
     1..n-1, in order; element tokens of the original run are stale, so each token action is preceded by a fresh read
     and re-bound by role + label - the gate's own heal does the same);
  3. a state fingerprint (files under the task's paths, window titles, focused window, clipboard, the app's own
     modified flag where readable) compared with the fingerprint the LOGGING run recorded at step n. Without a recorded
     fingerprint the restore is "unverified" and the branch must not enter the analysis.
What this cannot restore: app-internal state that leaves no trace in files/windows (undo stacks, scroll positions,
transient popups, timers), the model's sampling state, and wall-clock-dependent content. Screenshots being equal is
not evidence of equal state. Use the fixture twin (fixture_env.py) for strict branching; use OSWorld for whole-task
validation.
"""
import hashlib
import json
import os
import subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
BENCH = os.path.dirname(HERE)
GUI_TOOLS = {"click", "double_click", "right_click", "type_text", "set_value", "press_key", "hotkey", "scroll", "drag",
             "run_actions", "run_steps", "run_command"}


def prefix_from_pi_stream(path, n):
    """The first n tool calls of a Pi JSON event stream (bench/pi_monitor.py output): [(name, arguments)].
    The mcp__gui__ prefix is dropped; read-only calls are kept too (they move snapshot ids and the gate's history)."""
    calls = []
    for line in open(path, encoding="utf-8", errors="replace"):
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
        if e.get("type") != "message_end":
            continue
        m = e.get("message") or {}
        if m.get("role") != "assistant":
            continue
        for c in m.get("content") or []:
            if c.get("type") == "toolCall":
                name = (c.get("name") or "").replace("mcp__gui__", "")
                calls.append((name, c.get("arguments") or {}))
                if len(calls) >= n:
                    return calls
    return calls


def fingerprint(paths, run=None):
    """Hash of what a restore must reproduce. `run(cmd) -> str` executes on the box (default: local shell)."""
    run = run or (lambda cmd: subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=30).stdout)
    parts = {}
    for p in paths:
        parts["files:" + p] = run(f"find {p} -xdev -type f -printf '%P %s\\n' 2>/dev/null | sort | head -2000; "
                                  f"find {p} -xdev -type f -newer /proc/1 -exec sha256sum {{}} + 2>/dev/null | sort | head -500")
    parts["windows"] = run("wmctrl -l 2>/dev/null | awk '{$1=\"\";$2=\"\";$3=\"\";print}' | sort")
    parts["focus"] = run("xdotool getactivewindow getwindowname 2>/dev/null")
    parts["clipboard"] = hashlib.sha256(run("xclip -o -selection clipboard 2>/dev/null").encode()).hexdigest()
    blob = json.dumps(parts, sort_keys=True)
    return hashlib.sha256(blob.encode()).hexdigest()[:16], parts


class OSWorldCheckpoint:
    """Restore = setup + replay. Branch execution reuses harness.apply_treatment with a backend that sends the
    candidate / treatment through gui-gate on the box; that backend is not written yet (needs the box)."""

    def __init__(self, task_json, pi_stream, step, gate_cmd, recorded_fp=None, paths=("/home/user",)):
        self.task_json, self.pi_stream, self.step = task_json, pi_stream, step
        self.gate_cmd, self.recorded_fp, self.paths = gate_cmd, recorded_fp, list(paths)
        self.prefix = prefix_from_pi_stream(pi_stream, step - 1)
        self.candidate = (prefix_from_pi_stream(pi_stream, step) or [None])[-1] if step >= 1 else None

    def restore(self, mcp_client_factory, run=None):
        """mcp_client_factory(gate_cmd) -> object with .call(name, **args). Returns (client, restored_ok, fp)."""
        py = os.environ.get("OSWORLD_PY", "python3")
        subprocess.run([py, os.path.join(BENCH, "osworld_native.py"), "reset"], check=False)
        r = subprocess.run([py, os.path.join(BENCH, "osworld_native.py"), "setup", self.task_json, "--out", "/tmp/cf-setup.json"], check=False)
        if r.returncode != 0:
            return None, False, None
        cli = mcp_client_factory(self.gate_cmd)
        for name, args in self.prefix:
            cli.call(name, **args)
        fp, _ = fingerprint(self.paths, run)
        ok = None if self.recorded_fp is None else fp == self.recorded_fp
        return cli, ok, fp
