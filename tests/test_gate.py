"""End-to-end tests of the gate over real stdio MCP, against tests/fake_driver.py.

Run:  python3 -m unittest discover -s tests -v
"""
import json
import os
os.environ.setdefault("GUI_GATE_NO_UPDATE_CHECK", "1")  # unit tests never touch the network
import re
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
FAKE = f"{sys.executable} {os.path.join(HERE, 'fake_driver.py')}"
# the gate under test: the Rust binary (GUI_GATE_BIN, default target/{debug,release}/gui-gate)
# a reviewer model is required (>= 0.9.3): tests get a dummy one that is never reachable (fails closed) unless a
# test points GUI_GATE_REVIEWER_URL at its own fake server; GUI_GATE_REVIEWER_ENV keeps the user's file out
FAKE_REVIEWER = {"GUI_GATE_REVIEWER_URL": "http://127.0.0.1:9/v1/chat/completions", "GUI_GATE_REVIEWER_MODEL": "fake-reviewer",
                 "GUI_GATE_REVIEWER_KEY": "test-key", "GUI_GATE_REVIEWER_TIMEOUT": "2",
                 "GUI_GATE_REVIEWER_ENV": os.path.join(HERE, "no-such-reviewer.env"),
                 "GUI_GATE_ALLOW_CONFIG": "1"}  # tests flip runtime settings; production sessions cannot
BIN = os.environ.get("GUI_GATE_BIN") or next(
    (p for p in (os.path.join(ROOT, "target", "release", "gui-gate"), os.path.join(ROOT, "target", "debug", "gui-gate"),
                 "/tmp/gg-target/release/gui-gate", "/tmp/gg-target/debug/gui-gate") if os.path.exists(p)), None)


class Client:
    def __init__(self, *extra, env=None, capabilities=None, client_name="t", on_request=None):
        self.log = tempfile.NamedTemporaryFile(suffix=".jsonl", delete=False).name
        if not BIN:
            raise unittest.SkipTest("build the gate first: cargo build")
        cmd = [BIN, "--child", FAKE, "--log", self.log, *extra]
        e = dict(os.environ, PYTHONPATH=ROOT)
        e.update({k: v for k, v in FAKE_REVIEWER.items() if not (env or {}).get(k) and not os.environ.get(k)})
        e.update(env or {})
        self.p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=e, cwd=ROOT)
        self.i = 0
        self.extra = []
        self.on_request = on_request  # answers requests the gate sends to the client (elicitation)
        self.requests = []
        self.init = self.rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": capabilities or {},
                                            "clientInfo": {"name": client_name, "version": "0"}})
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        self.p.stdin.flush()

    def rpc(self, method, params):
        self.i += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.i, "method": method, "params": params}) + "\n")
        self.p.stdin.flush()
        while True:
            m = json.loads(self.p.stdout.readline())
            if m.get("method") and "id" in m and self.on_request:
                self.requests.append(m)
                self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": m["id"], "result": self.on_request(m)}) + "\n")
                self.p.stdin.flush()
                continue
            if m.get("id") == self.i:
                return m["result"]
            self.extra.append(m)

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
        self.assertNotIn("gate_ack", tools["click"]["inputSchema"]["properties"])  # the agent never approves its own step
        self.assertIn("gate_confirm", tools["click"]["inputSchema"]["properties"])
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
        self.assertIn("fresh tree is attached", txt(r))  # the gate read the window itself: no extra turn
        self.assertIn("nothing was executed", txt(r))

    def test_reviewer_mode_with_unreachable_reviewer_goes_to_a_person_not_the_agent(self):
        self.c.call("gate_config", risk_mode="reviewer")
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:1")
        self.assertTrue(r.get("isError"))
        # fail closed: the (dummy) reviewer does not answer -> a person (attended), never the acting agent
        self.assertEqual(r["structuredContent"]["gate"]["decision"], "ask_human")
        self.assertIn("Not executed", txt(r))
        r2 = self.c.call("click", element_token=f"{s}:1", gate_ack=True)  # ignored
        self.assertTrue(r2.get("isError"))
        rep = self.c.call("gate_report")["structuredContent"]
        self.assertEqual(rep["reviewer"], "api:chat:fake-reviewer")
        self.assertNotIn("decider", rep)

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
        self.assertIn("fresh screenshot of the window is attached", txt(r))
        self.assertTrue(any(c.get("type") == "image" for c in r["content"]), [c.get("type") for c in r["content"]])

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

    def test_somewhat_old_read_does_not_cost_a_turn(self):
        # v2: no fixed grey band; a few keystrokes since the read are not worth a probe or a model turn
        # (logged probes flagged 2 of 82 later failures), so the click goes through at once
        self.c.close()
        self.c = Client("--no-auto-observe")
        s = self.c.read()
        for _ in range(3):
            self.c.call("press_key", pid=4242, key="a")
        r = self.c.call("click", element_token=f"{s}:0")
        ev = [e for e in self.c.events() if e["ev"] == "action"][-1]
        self.assertTrue(ev["executed"])
        self.assertEqual(ev["model"], "osworld_v3")
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

    def test_medium_risk_with_unreachable_reviewer_is_not_executed(self):
        self.c.call("fake_mutate", elements=[["button", "Send"], ["button", "Plum"]])
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:0")
        self.assertTrue(r.get("isError"))
        self.assertEqual(r["structuredContent"]["gate"]["decision"], "ask_human")  # reviewer down + attended: a person
        self.assertIn("Not executed", txt(r))
        r2 = self.c.call("click", element_token=f"{s}:0", gate_ack=True)  # self-approval does nothing
        self.assertTrue(r2.get("isError"))

    def test_secret_field_handoff(self):
        self.c.call("fake_mutate", elements=[["password text", "Password"], ["button", "Plum"]])
        s = self.c.read()
        r = self.c.call("type_text", element_token=f"{s}:0", text="hunter2")
        self.assertEqual(r["structuredContent"]["gate"]["decision"], "handoff")

    def test_dangerous_command_with_unreachable_reviewer_is_not_executed(self):
        self.c.read()
        r = self.c.call("type_text", pid=4242, text="rm -rf ~/work")  # medium risk, needs review: reviewer down
        self.assertTrue(r.get("isError"))
        self.assertEqual(r["structuredContent"]["gate"]["decision"], "ask_human")
        self.assertIn("Not executed", txt(r))

    def test_enter_after_dangerous_command(self):
        c = Client("--risk-mode", "human")  # every risky step goes to a person instead of the reviewer
        try:
            c.read()
            r = c.call("type_text", pid=4242, text="rm -rf ~/work")
            self.assertEqual(r["structuredContent"]["gate"]["decision"], "ask_human")
            c.call("type_text", pid=4242, text="rm -rf ~/work", gate_confirm=r["structuredContent"]["gate"]["confirm_code"])
            r = c.call("press_key", pid=4242, key="Return")
            self.assertEqual(r["structuredContent"]["gate"]["decision"], "ask_human")
        finally:
            c.close()

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
            r = c.call("click", element_token=f"{s}:0")
            self.assertFalse(r.get("isError"), txt(r))
            ev = [e for e in c.events() if e["ev"] == "action"][-1]
            self.assertTrue(ev["executed"])
            self.assertTrue(ev["shadow_check"])
            # the baseline policy never bypasses the safety rules
            s = c.read()
            r = c.call("click", element_token=f"{s}:1")
            self.assertTrue(r.get("isError"), txt(r))
            self.assertIn("high-risk", txt(r))
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


def _img(r):
    return [c for c in r.get("content", []) if c.get("type") == "image"]


class ScreenshotFormatTests(unittest.TestCase):
    """v0.9.4: screenshots go back to the agent as full-size JPEG q60 (GUI_GATE_SCREENSHOT_FORMAT=png keeps PNG)."""

    def _read(self, env, *extra):
        c = Client(*extra, env=dict(env, FAKE_PNG="1"))
        try:
            r = c.call("get_window_state", pid=4242, window_id=777, include_screenshot=True)
            return r, c.events(), c.call("gate_report")
        finally:
            c.close()

    def test_default_is_jpeg_full_size(self):
        import base64, struct
        for policy in ("gate", "direct", "full", "light"):
            with self.subTest(policy=policy):
                r, evs, rep = self._read({}, "--policy", policy)
                imgs = _img(r)
                self.assertEqual(len(imgs), 1)
                self.assertEqual(imgs[0]["mimeType"], "image/jpeg")
                raw = base64.b64decode(imgs[0]["data"])
                self.assertEqual(raw[:3], b"\xff\xd8\xff")
                # SOF0/SOF2 frame header carries the size: never downscaled
                i = 2
                while i < len(raw):
                    marker, ln = raw[i + 1], struct.unpack(">H", raw[i + 2:i + 4])[0]
                    if marker in (0xC0, 0xC1, 0xC2):
                        h, w = struct.unpack(">HH", raw[i + 5:i + 9])
                        break
                    i += 2 + ln
                self.assertEqual((w, h), (500, 300))
                ev = [e for e in evs if e["ev"] == "img"][-1]
                self.assertEqual(ev["converted"], 1)
                self.assertLess(ev["bytes_out"], ev["bytes_in"])
                self.assertEqual(ev["format"], "jpeg q60")
                self.assertEqual(rep["structuredContent"]["screenshot_format"], "jpeg q60")

    def test_png_setting_keeps_png(self):
        import base64
        r, evs, _ = self._read({"GUI_GATE_SCREENSHOT_FORMAT": "png"})
        imgs = _img(r)
        self.assertEqual(imgs[0]["mimeType"], "image/png")
        self.assertEqual(base64.b64decode(imgs[0]["data"])[:4], b"\x89PNG")
        ev = [e for e in evs if e["ev"] == "img"][-1]
        self.assertEqual((ev["converted"], ev["bytes_in"]), (0, ev["bytes_out"]))

    def test_quality_clamped_and_undecodable_passes_through(self):
        r, evs, _ = self._read({"GUI_GATE_SCREENSHOT_QUALITY": "5"})
        self.assertEqual([e for e in evs if e["ev"] == "img"][-1]["format"], "jpeg q30")
        c = Client()  # placeholder "AAAA" image: not a PNG -> untouched
        try:
            r = c.call("get_window_state", pid=4242, window_id=777, include_screenshot=True)
            self.assertEqual(_img(r)[0], {"type": "image", "data": "AAAA", "mimeType": "image/png"})
        finally:
            c.close()


class DeliveryTests(unittest.TestCase):
    def test_auto_falls_back_to_foreground_and_sticks(self):
        env = dict(os.environ, FAKE_NO_BG="1")
        c = Client("--human", "deny", env=env)
        try:
            s = c.read()
            r = c.call("press_key", pid=4242, key="a", gate_ack=True)
            self.assertFalse(r.get("isError"), txt(r))
            self.assertIn("delivery=foreground", txt(r))
            r2 = c.call("type_text", pid=4242, text="x", gate_ack=True)
            self.assertIn("delivery=foreground", txt(r2))
        finally:
            c.close()

    def test_background_mode_keeps_driver_error(self):
        env = dict(os.environ, FAKE_NO_BG="1")
        c = Client("--human", "deny", "--delivery", "background", env=env)
        try:
            c.read()
            r = c.call("press_key", pid=4242, key="a", gate_ack=True)
            self.assertIn("Background delivery is not available", txt(r))
        finally:
            c.close()


class PixelFrameTests(unittest.TestCase):
    def test_pixel_clicks_keep_screenshot_frame(self):
        c = Client("--human", "deny", env=dict(os.environ, FAKE_NO_FRAME="1"))
        try:
            c.read()
            r1 = c.call("click", pid=4242, window_id=777, x=20, y=30, gate_ack=True)
            self.assertFalse(r1.get("isError"), txt(r1))
            r2 = c.call("click", pid=4242, window_id=777, x=25, y=35, gate_ack=True)
            self.assertFalse(r2.get("isError"), txt(r2))  # auto-observe kept the screenshot for this window
            self.assertTrue(any(x.get("type") == "image" for x in r2["content"]))
        finally:
            c.close()


if __name__ == "__main__":
    unittest.main()
