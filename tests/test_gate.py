"""End-to-end tests of the gate over real stdio MCP, against tests/fake_driver.py.

Run:  python3 -m unittest discover -s tests -v
"""
import json
import os
import re
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
FAKE = f"{sys.executable} {os.path.join(HERE, 'fake_driver.py')}"


class Client:
    def __init__(self, *extra, env=None):
        self.log = tempfile.NamedTemporaryFile(suffix=".jsonl", delete=False).name
        cmd = [sys.executable, "-m", "gui_gate", "--child", FAKE, "--log", self.log, *extra]
        e = dict(os.environ, PYTHONPATH=ROOT, **(env or {}))
        self.p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=e, cwd=ROOT)
        self.i = 0
        self.init = self.rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                            "clientInfo": {"name": "t", "version": "0"}})
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        self.p.stdin.flush()

    def rpc(self, method, params):
        self.i += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.i, "method": method, "params": params}) + "\n")
        self.p.stdin.flush()
        while True:
            m = json.loads(self.p.stdout.readline())
            if m.get("id") == self.i:
                return m["result"]

    def call(self, name, **args):
        return self.rpc("tools/call", {"name": name, "arguments": args})

    def read(self, **a):
        r = self.call("get_window_state", pid=4242, window_id=777, **a)
        return r["structuredContent"]["snapshot_id"]

    def close(self):
        self.p.stdin.close()
        self.p.wait(timeout=5)

    def events(self):
        with open(self.log) as fh:
            return [json.loads(l) for l in fh]


def txt(r):
    return "\n".join(c.get("text", "") for c in r.get("content", []))


class GateTests(unittest.TestCase):
    def setUp(self):
        self.c = Client()

    def tearDown(self):
        self.c.close()

    def test_tools_list_augmented(self):
        tools = {t["name"]: t for t in self.c.rpc("tools/list", {})["tools"]}
        self.assertIn("gate_report", tools)
        self.assertIn("gate_ack", tools["click"]["inputSchema"]["properties"])
        self.assertIn("[gui-gate]", self.c.init["instructions"])

    def test_fresh_click_runs_directly(self):
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:0")
        self.assertFalse(r.get("isError"), txt(r))
        self.assertIn("Increment", txt(r))
        ev = [e for e in self.c.events() if e["ev"] == "action"][-1]
        self.assertEqual(ev["decision"], "direct")
        self.assertTrue(ev["executed"])

    def test_stale_token_is_healed_without_model(self):
        s1 = self.c.read()
        self.c.read()  # agent re-read: s1 tokens are now stale in the driver
        r = self.c.call("click", element_token=f"{s1}:0")
        self.assertFalse(r.get("isError"), txt(r))
        self.assertIn("re-bound", txt(r))

    def test_stale_ambiguous_token_asks_light_look(self):
        s1 = self.c.read()
        self.c.read()
        r = self.c.call("click", element_token=f"{s1}:3")  # "OK" exists twice -> no unique re-bind
        self.assertTrue(r.get("isError"))
        self.assertIn("without a screenshot", txt(r))

    def test_high_risk_main_model_review(self):
        self.c.call("gate_config", risk_mode="model")
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:1")
        self.assertTrue(r.get("isError"))
        self.assertEqual(r["structuredContent"]["gate"]["decision"], "risk_review")
        r2 = self.c.call("click", element_token=f"{s}:1", gate_ack=True)
        self.assertFalse(r2.get("isError"), txt(r2))

    def test_model_decider_hands_grey_zone_to_main_model(self):
        self.c.call("gate_config", decider="model")
        s1 = self.c.read()
        self.c.read()
        r = self.c.call("click", element_token=f"{s1}:3")  # ambiguous stale token: formula would ask a re-read
        self.assertTrue(r.get("isError"))
        # a rule (stale, not healable) stays a rule; check the report fields exist
        rep = self.c.call("gate_report")["structuredContent"]
        self.assertEqual(rep["decider"], "model")
        self.assertIn("model_reviews", rep)

    def test_second_read_omits_screenshot(self):
        self.c.read()
        r = self.c.call("get_window_state", pid=4242, window_id=777)
        self.assertFalse(any(c.get("type") == "image" for c in r["content"]))
        self.assertIn("screenshot omitted", txt(r))
        r = self.c.call("get_window_state", pid=4242, window_id=777, include_screenshot=True)
        self.assertTrue(any(c.get("type") == "image" for c in r["content"]))

    def test_high_risk_needs_confirmation(self):
        self.c.call("gate_config", risk_mode="human")
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:1")
        self.assertTrue(r.get("isError"))
        code = r["structuredContent"]["gate"]["confirm_code"]
        r2 = self.c.call("click", element_token=f"{s}:1", gate_confirm="wrong")
        self.assertTrue(r2.get("isError"))
        code = r2["structuredContent"]["gate"]["confirm_code"]
        r3 = self.c.call("click", element_token=f"{s}:1", gate_confirm=code)
        self.assertFalse(r3.get("isError"), txt(r3))

    def test_pixel_click_after_popup_requires_screenshot(self):
        self.c.read()
        self.c.call("click", window_id=777, pid=4242, x=100, y=100)
        self.c.call("fake_mutate", popup=True)
        r = self.c.call("click", window_id=777, pid=4242, x=120, y=100)
        self.assertTrue(r.get("isError"))
        self.assertIn("WITH a screenshot", txt(r))

    def test_repeat_after_failure_is_blocked(self):
        s = self.c.read()
        self.c.call("fake_mutate", fail_next=1)
        r1 = self.c.call("click", element_token=f"{s}:0")
        self.assertTrue(r1.get("isError"))
        r2 = self.c.call("click", element_token=f"{s}:0")
        self.assertIn("[gate]", txt(r2))

    def test_unknown_window_requires_observation(self):
        r = self.c.call("click", element_token="s000000ff:0")
        self.assertIn("[gate]", txt(r))

    def test_grey_zone_runs_cheap_check(self):
        self.c.close()
        self.c = Client("--no-auto-observe")
        s = self.c.read()
        for _ in range(3):  # several actions since the read push P(direct) into the grey zone
            self.c.call("press_key", pid=4242, key="a")
        r = self.c.call("click", element_token=f"{s}:0")
        ev = [e for e in self.c.events() if e["ev"] == "action"][-1]
        self.assertIn("check", ev["trail"])
        self.assertTrue(ev["check"])
        self.assertFalse(r.get("isError"), txt(r))

    def test_grey_zone_refresh_catches_relabel(self):
        s = self.c.read()
        for _ in range(3):
            self.c.call("press_key", pid=4242, key="a")
        # UI relabels behind the agent's back: index 0 is no longer "Increment"
        self.c.call("fake_mutate", elements=[["button", "Plum"], ["button", "Delete file"], ["text", "count=0"],
                                             ["button", "Increment"], ["button", "OK"]])
        r = self.c.call("click", element_token=f"{s}:0")
        self.assertFalse(r.get("isError"), txt(r))
        self.assertIn('"Increment"', txt(r))  # re-bound to the element that really carries the label

    def test_report(self):
        s = self.c.read()
        self.c.call("click", element_token=f"{s}:0")
        r = self.c.call("gate_report")
        self.assertEqual(r["structuredContent"]["executed"], 1)
        self.assertEqual(r["structuredContent"]["screenshots"], 1)


class ToolTests(unittest.TestCase):
    """Speed / stability / risk / handoff features that live in the tool layer."""

    def setUp(self):
        self.c = Client("--human", "chat")

    def tearDown(self):
        self.c.close()

    def test_core_toolset_hides_rare_tools(self):
        names = {t["name"] for t in self.c.rpc("tools/list", {})["tools"]}
        self.assertIn("click", names)
        self.assertIn("request_human", names)
        self.assertNotIn("verify_state", names)
        self.assertNotIn("gate_config", names)

    def test_action_returns_fresh_tree(self):
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:0")
        t = txt(r)
        self.assertIn("Window right after the action", t)
        self.assertIn("count=1", t)
        new = re.search(r"snapshot (s[0-9a-f]{8})", t).group(1)
        r2 = self.c.call("click", element_token=f"{new}:0")  # agent uses the attached tokens directly
        self.assertFalse(r2.get("isError"), txt(r2))
        self.assertIn("count=2", txt(r2))

    def test_high_risk_goes_to_person(self):
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:1")  # "Delete file"
        self.assertEqual(r["structuredContent"]["gate"]["decision"], "ask_human")
        r2 = self.c.call("click", element_token=f"{s}:1", gate_ack=True)  # the model cannot self-approve high risk
        self.assertTrue(r2.get("isError"))

    def test_medium_risk_model_rethinks(self):
        self.c.call("fake_mutate", elements=[["button", "Send"], ["button", "Plum"]])
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:0")
        self.assertEqual(r["structuredContent"]["gate"]["decision"], "risk_review")
        r2 = self.c.call("click", element_token=f"{s}:0", gate_ack=True)
        self.assertFalse(r2.get("isError"), txt(r2))

    def test_secret_field_handoff(self):
        self.c.call("fake_mutate", elements=[["password text", "Password"], ["button", "Plum"]])
        s = self.c.read()
        r = self.c.call("type_text", element_token=f"{s}:0", text="hunter2")
        self.assertEqual(r["structuredContent"]["gate"]["decision"], "handoff")

    def test_enter_after_dangerous_command(self):
        self.c.read()
        r = self.c.call("type_text", pid=4242, text="rm -rf ~/work")
        self.assertEqual(r["structuredContent"]["gate"]["decision"], "risk_review")
        self.c.call("type_text", pid=4242, text="rm -rf ~/work", gate_ack=True)
        r = self.c.call("press_key", pid=4242, key="Return")
        self.assertEqual(r["structuredContent"]["gate"]["decision"], "ask_human")

    def test_request_human_chat_fallback(self):
        r = self.c.call("request_human", question="Which file?", kind="answer")
        self.assertIn("Ask your user", txt(r))

    def test_driver_crash_is_survived(self):
        self.c.call("fake_crash")
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:0")
        self.assertFalse(r.get("isError"), txt(r))
        self.assertRegex(txt(self.c.call("gate_report")), r"driver restarts=[12]")


class PolicyTests(unittest.TestCase):
    def test_full_policy_demands_screenshot_every_step(self):
        c = Client("--policy", "full")
        try:
            s = c.read(include_screenshot=False)
            r = c.call("click", element_token=f"{s}:0")
            self.assertIn("WITH a screenshot", txt(r))
            s = c.read()
            r = c.call("click", element_token=f"{s}:0")
            self.assertFalse(r.get("isError"), txt(r))
            s2 = s
            r = c.call("click", element_token=f"{s2}:0")
            self.assertIn("[gate]", txt(r))  # an action happened since the screenshot
        finally:
            c.close()

    def test_direct_policy_never_intervenes_but_logs(self):
        c = Client("--policy", "direct", "--collect")
        try:
            s = c.read()
            r = c.call("click", element_token=f"{s}:1")  # even a risky one
            self.assertFalse(r.get("isError"), txt(r))
            ev = [e for e in c.events() if e["ev"] == "action"][-1]
            self.assertTrue(ev["executed"])
            self.assertTrue(ev["shadow_check"])
        finally:
            c.close()

    def test_runtime_config(self):
        c = Client()
        try:
            r = c.call("gate_config", policy="light", **{"lambda": 2.5})
            self.assertEqual(r["structuredContent"]["policy"], "light")
            self.assertEqual(r["structuredContent"]["lambda"], 2.5)
        finally:
            c.close()


if __name__ == "__main__":
    unittest.main()
