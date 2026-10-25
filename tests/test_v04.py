"""v0.4: authorization, deny-and-continue, observation budget, server-side batching (run_actions)."""
import json
import os
import subprocess
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from test_gate import BIN, Client, txt  # noqa: E402



class V04GateTests(unittest.TestCase):
    def tearDown(self):
        self.c.close()

    def test_denial_gives_safe_alternative_and_is_remembered(self):
        self.c = Client("--human", "deny")
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:1")  # "Delete file" -> high risk -> person (auto-deny)
        self.assertTrue(r.get("isError"))
        self.assertIn("Instead:", txt(r))
        self.assertNotIn("Stop and tell the user", txt(r))
        s = self.c.read()
        r2 = self.c.call("click", element_token=f"{s}:1")
        self.assertEqual(r2["structuredContent"]["gate"]["decision"], "deny")
        asks = [e for e in self.c.events() if e.get("ev") == "action" and e.get("human")]
        self.assertEqual(len(asks), 1)  # the person was not asked twice for the same effect

    def test_repeated_denials_escalate(self):
        self.c = Client("--human", "deny")
        last = None
        for _ in range(3):
            s = self.c.read()
            last = self.c.call("click", element_token=f"{s}:1")
        self.assertIn("stop here and tell the user", txt(last))

    def test_task_wording_without_a_matchable_target_does_not_authorize(self):
        # "delete the file" + a "Delete file" button: the gate cannot match the button's object to the request, so the
        # words alone never cover it (a person / the reviewer decides; here nobody approves)
        self.c = Client("--human", "deny", "--task", "Please delete the file in this window")
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:1")
        self.assertTrue(r.get("isError"), txt(r))
        ev = [e for e in self.c.events() if e.get("ev") == "action"][-1]
        self.assertNotIn("authorized", ev["trail"])
        self.assertIn("cannot match", ev["risk_gate"]["why"])

    def test_task_naming_the_file_authorizes(self):
        import tempfile, time
        d = tempfile.mkdtemp()
        f = os.path.join(d, "old-notes.txt")
        open(f, "w").write("x")
        os.utime(f, (time.time() - 3600, time.time() - 3600))
        self.c = Client("--human", "deny", "--task", f"Please delete {f}")
        r = self.c.call("run_command", command=f"rm {f}", cwd=d)
        self.assertFalse(r.get("isError"), txt(r))
        self.assertFalse(os.path.exists(f))
        ev = [e for e in self.c.events() if e.get("ev") == "action"][-1]
        self.assertIn("authorized", ev["trail"])

    def test_gate_task_set_once(self):
        self.c = Client("--human", "deny")
        self.assertFalse(self.c.call("gate_task", request="delete the file").get("isError"))
        self.assertTrue(self.c.call("gate_task", request="something else").get("isError"))

    def test_gate_task_only_before_first_action(self):
        self.c = Client("--human", "deny")
        s = self.c.read()
        self.c.call("click", element_token=f"{s}:0")
        self.assertIn("Too late", txt(self.c.call("gate_task", request="delete the file")))

    def test_agent_relayed_request_cannot_authorize_sending(self):
        self.c = Client("--human", "deny")
        self.c.call("gate_task", request="send the email to Bob")
        self.c.call("fake_mutate", elements=[["button", "Send"], ["button", "Increment"], ["text", "x"]])
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:0")
        self.assertTrue(r.get("isError"), txt(r))

    def test_duplicate_read_served_from_auto_observe(self):
        self.c = Client("--human", "deny")
        s = self.c.read()
        self.c.call("click", element_token=f"{s}:0")
        r = self.c.call("get_window_state", pid=4242, window_id=777)
        self.assertTrue(r["structuredContent"].get("cached"), txt(r))
        r2 = self.c.call("get_window_state", pid=4242, window_id=777, fresh=True)
        self.assertFalse(r2["structuredContent"].get("cached"))

    def test_screenshots_stop_after_pixel_aiming_ends(self):
        self.c = Client("--human", "deny")
        self.c.read()
        self.c.call("click", pid=4242, window_id=777, x=20, y=30, gate_ack=True)
        for _ in range(3):
            self.c.call("press_key", pid=4242, window_id=777, key="Down")
        shots = [e["screenshot"] for e in self.c.events() if e.get("ev") == "obs" and e.get("auto")]
        self.assertEqual(shots, [True, True, False, False])


class StallHintTests(unittest.TestCase):
    def test_third_read_in_a_row_gets_a_stall_hint_and_an_action_resets(self):
        c = Client("--human", "deny")
        try:
            s = c.read(fresh=True)
            c.call("click", element_token=f"{s}:2")           # reads before the first action are orientation
            rs = [c.call("get_window_state", pid=4242, window_id=777, fresh=True) for _ in range(3)]
            self.assertNotIn("reads in a row", txt(rs[1]))
            self.assertIn("3 reads in a row", txt(rs[2]))
            s = c.read(fresh=True)                         # 4th
            c.call("click", element_token=f"{s}:2")
            rs = [c.call("get_window_state", pid=4242, window_id=777, fresh=True) for _ in range(2)]
            self.assertNotIn("reads in a row", txt(rs[-1]))
            self.assertEqual(len([e for e in c.events() if e.get("ev") == "streak_hint"]), 1)
        finally:
            c.close()


class TextOnlyTests(unittest.TestCase):
    def test_text_only_never_captures_and_points_at_tokens(self):
        c = Client("--human", "deny", "--text-only", env=dict(os.environ, FAKE_NO_FRAME="1"))
        try:
            r = c.call("get_window_state", pid=4242, window_id=777, include_screenshot=True)
            self.assertFalse(any(x.get("type") == "image" for x in r["content"]))
            self.assertIn("text-only session", txt(r))
            s = c.read(fresh=True)
            c.call("click", element_token=f"{s}:2")
            r = c.call("click", pid=4242, window_id=777, x=20, y=30, gate_ack=True)   # no frame -> tokens, not a screenshot
            self.assertNotIn("fresh screenshot", txt(r))
            obs = [e for e in c.events() if e.get("ev") == "obs"]
            self.assertTrue(obs and not any(e.get("screenshot") for e in obs), obs)
        finally:
            c.close()


class _Rev:
    """A local OpenAI-compatible endpoint that answers with a fixed decision and records what it was shown."""
    def __init__(self, decision):
        import http.server
        import json as _j
        import threading
        seen = self.seen = []

        class H(http.server.BaseHTTPRequestHandler):
            def do_POST(self):
                body = _j.loads(self.rfile.read(int(self.headers["Content-Length"])))
                seen.append(body)
                out = _j.dumps({"choices": [{"message": {"content": _j.dumps(
                    {"decision": decision, "reason": "test reviewer"})}}]}).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(out)

            def log_message(self, *a):
                pass
        self.srv = http.server.HTTPServer(("127.0.0.1", 0), H)
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()
        self.url = f"http://127.0.0.1:{self.srv.server_port}/v1/chat/completions"

    def close(self):
        self.srv.shutdown()


class ReviewerTests(unittest.TestCase):
    def run_send(self, decision):
        rev = _Rev(decision)
        c = Client("--human", "deny", "--task", "draft a reply to Bob",
                   env=dict(os.environ, GUI_GATE_REVIEWER_URL=rev.url))
        try:
            c.call("fake_mutate", elements=[["button", "Send"], ["button", "Increment"], ["text", "x"]])
            s = c.read()
            r = c.call("click", element_token=f"{s}:0", gate_ack=True)  # self-ack must not bypass the reviewer
            return r, rev.seen
        finally:
            c.close()
            rev.close()

    def test_reviewer_deny(self):
        r, seen = self.run_send("deny")
        self.assertEqual(r["structuredContent"]["gate"]["decision"], "deny")
        self.assertIn("independent reviewer", txt(r))
        prompt = seen[0]["messages"][1]["content"]
        self.assertIn("draft a reply to Bob", prompt)
        self.assertNotIn("gate_ack", prompt)

    def test_reviewer_down_refuses_unattended(self):
        c = Client("--human", "deny", env=dict(os.environ, GUI_GATE_REVIEWER_URL="http://127.0.0.1:9/x",
                                               GUI_GATE_REVIEWER_TIMEOUT="2"))
        try:
            c.call("fake_mutate", elements=[["button", "Send"], ["button", "Increment"], ["text", "x"]])
            s = c.read()
            r = c.call("click", element_token=f"{s}:0", gate_ack=True)  # no self-approval fallback any more
            self.assertTrue(r.get("isError"))
            self.assertEqual(r["structuredContent"]["gate"]["decision"], "deny")
        finally:
            c.close()

    def test_reviewer_sees_recent_actions_and_logs_what_it_got(self):
        rev = _Rev("deny")
        c = Client("--human", "deny", "--task", "draft a reply to Bob", env=dict(os.environ, GUI_GATE_REVIEWER_URL=rev.url))
        try:
            c.call("fake_mutate", elements=[["button", "Send"], ["button", "Increment"], ["text", "x"]])
            s = c.read()
            c.call("click", element_token=f"{s}:1")
            c.call("type_text", pid=4242, window_id=777, text="hello Bob")
            s = c.read(fresh=True)
            c.call("click", element_token=f"{s}:0")
            prompt = rev.seen[-1]["messages"][1]["content"]
            self.assertIn("recent_actions", prompt)
            self.assertIn("hello Bob", prompt)
            self.assertIn("Increment", prompt)
            ev = [e for e in c.events() if e.get("ev") == "action"][-1]
            self.assertEqual(len(ev["risk_gate"]["reviewer_sent"]["recent_actions"]), 2)
        finally:
            c.close()
            rev.close()

    def test_step_review_sees_rows_and_a_resent_doubted_step_runs(self):
        # m3 run: the pre-click re-read is tree_format "elements" -> the reviewer used to get no rows (25/30 calls);
        # a step the reviewer doubted and the agent re-sent after the fresh look was doubted again (ow16 x3)
        rev = _Rev("deny")
        c = Client("--human", "deny", "--task", "increment the counter", env=dict(os.environ, GUI_GATE_REVIEWER_URL=rev.url))
        try:
            c.call("fake_mutate", fail_next=1)
            s = c.read(fresh=True)
            c.call("click", element_token=f"{s}:2")           # fails -> next try is doubted
            s = c.read(fresh=True)
            r = c.call("click", element_token=f"{s}:2")
            self.assertEqual(r["structuredContent"]["gate"]["decision"], "look_light", txt(r))
            window = json.loads(rev.seen[-1]["messages"][1]["content"]) if rev.seen[-1]["messages"][1]["content"].startswith("{") else rev.seen[-1]["messages"][1]["content"]
            self.assertIn("Increment", json.dumps(window))   # element rows reached the reviewer
            s = c.read(fresh=True)
            r = c.call("click", element_token=f"{s}:2")       # re-sent after the fresh look -> runs
            self.assertFalse(r.get("isError"), txt(r))
            ev = [e for e in c.events() if e.get("ev") == "action"][-1]
            self.assertIn("reviewer_insist", ev["trail"])
            self.assertEqual(len(rev.seen), 1)
        finally:
            c.close()
            rev.close()

    def test_reviewer_allow(self):
        r, _ = self.run_send("allow")
        self.assertFalse(r.get("isError"), txt(r))


class RunActionsTests(unittest.TestCase):
    def setUp(self):
        self.c = Client("--human", "deny")

    def tearDown(self):
        self.c.close()

    def test_listed(self):
        names = {t["name"] for t in self.c.rpc("tools/list", {})["tools"]}
        self.assertIn("run_actions", names)
        self.assertIn("gate_task", names)

    def test_batch_runs_with_one_observation(self):
        s = self.c.read()
        r = self.c.call("run_actions", pid=4242, window_id=777, steps=[
            {"tool": "click", "args": {"element_token": f"{s}:0"}},
            {"tool": "type_text", "args": {"text": "abc"}},
            {"tool": "press_key", "args": {"key": "Tab"}}])
        self.assertFalse(r.get("isError"), txt(r))
        self.assertIn("3 GUI action(s) ran", txt(r))
        auto = [e for e in self.c.events() if e.get("ev") == "obs" and e.get("auto")]
        self.assertEqual(len(auto), 1)
        acts = [e for e in self.c.events() if e.get("ev") == "action"]
        self.assertTrue(all(e.get("batch") for e in acts[-3:]))

    def test_batch_stops_at_refused_step(self):
        s = self.c.read()
        r = self.c.call("run_actions", pid=4242, window_id=777, steps=[
            {"tool": "click", "args": {"element_token": f"{s}:0"}},
            {"tool": "click", "args": {"element_token": f"{s}:1"}},   # Delete file -> refused
            {"tool": "press_key", "args": {"key": "Tab"}}])
        self.assertTrue(r.get("isError"))
        self.assertIn("STOPPED", txt(r))
        self.assertIn("REFUSED", txt(r))
        self.assertEqual(r["structuredContent"]["batch"]["executed"], 1)

    def test_pixel_only_in_first_step(self):
        self.c.read()
        r = self.c.call("run_actions", pid=4242, window_id=777, steps=[
            {"tool": "press_key", "args": {"key": "Tab"}},
            {"tool": "click", "args": {"x": 10, "y": 10}}])
        self.assertIn("stale", txt(r))
        self.assertEqual(r["structuredContent"]["batch"]["executed"], 1)

    def test_unexpected_dialog_stops_batch(self):
        self.c.call("fake_mutate", elements=[["button", "Open dialog"], ["button", "Increment"], ["text", "x"]])
        s = self.c.read()
        r = self.c.call("run_actions", pid=4242, window_id=777, steps=[
            {"tool": "click", "args": {"element_token": f"{s}:0"}},
            {"tool": "press_key", "args": {"key": "Tab"}}])
        self.assertIn("new window/dialog", txt(r))
        self.assertEqual(r["structuredContent"]["batch"]["executed"], 1)

    def test_menu_then_item_by_label(self):
        self.c.call("fake_mutate", elements=[["button", "Open menu"], ["button", "Increment"], ["text", "x"]])
        self.c.read()
        r = self.c.call("run_actions", pid=4242, window_id=777, steps=[
            {"tool": "click", "args": {"target": {"label": "Open menu"}}},
            {"tool": "click", "args": {"target": {"label": "Save As", "role": "menu item"}}}])
        self.assertFalse(r.get("isError"), txt(r))
        self.assertEqual(r["structuredContent"]["batch"]["executed"], 2)

    def test_token_into_new_menu_is_refused(self):
        self.c.call("fake_mutate", elements=[["button", "Open menu"], ["button", "Increment"], ["text", "x"]])
        s = self.c.read()
        r = self.c.call("run_actions", pid=4242, window_id=777, steps=[
            {"tool": "click", "args": {"element_token": f"{s}:0"}},
            {"tool": "click", "args": {"element_token": f"{s}:1"}}])
        self.assertIn("target:{label}", txt(r))
        self.assertEqual(r["structuredContent"]["batch"]["executed"], 1)

    def test_missing_label_stops(self):
        self.c.read()
        r = self.c.call("run_actions", pid=4242, window_id=777, steps=[
            {"tool": "click", "args": {"target": {"label": "Nope"}}}])
        self.assertIn("no element named", txt(r))
        self.assertEqual(r["structuredContent"]["batch"]["executed"], 0)

    def test_tools_not_allowed_in_batch(self):
        r = self.c.call("run_actions", steps=[{"tool": "run_steps", "args": {}}])
        self.assertIn("cannot be used inside a batch", txt(r))


class ProgramTests(unittest.TestCase):
    """run_actions control steps: find / for_each / if / read (the codemode replacement, no JS engine)."""

    def setUp(self):
        self.c = Client("--human", "deny")

    def tearDown(self):
        self.c.close()

    def test_find_and_for_each_clicks_each_match(self):
        self.c.call("fake_mutate", elements=[["button", "Inc A"], ["button", "Inc B"], ["text", "x"], ["button", "Other"]])
        self.c.read()
        r = self.c.call("run_actions", pid=4242, window_id=777, steps=[
            {"find": {"label_contains": "inc", "role": "button"}, "as": "incs"},
            {"for_each": "incs", "do": [{"tool": "click", "args": {"target": "$item"}}]}])
        self.assertFalse(r.get("isError"), txt(r))
        self.assertEqual(r["structuredContent"]["batch"]["executed"], 2)
        self.assertIn("2 match(es)", txt(r))

    def test_if_branches(self):
        self.c.call("fake_mutate", elements=[["button", "Increment"], ["button", "Plum"], ["text", "x"]])
        self.c.read()
        r = self.c.call("run_actions", pid=4242, window_id=777, steps=[
            {"if": {"exists": {"label": "Nope"}},
             "then": [{"tool": "click", "args": {"target": {"label": "Plum"}}}],
             "else": [{"tool": "click", "args": {"target": {"label": "Increment"}}}]}])
        self.assertIn("if exists -> else", txt(r))
        self.assertIn('"Increment"', txt(r))

    def test_read_reports_values(self):
        self.c.read()
        r = self.c.call("run_actions", pid=4242, window_id=777, steps=[
            {"tool": "click", "args": {"target": {"label": "Increment"}}},
            {"read": {"label_regex": "^count="}, "as": "counter"}])
        self.assertIn("count=1", txt(r))
        self.assertEqual(r["structuredContent"]["batch"]["read"], ['counter: text "count=1"'])

    def test_loop_cannot_run_away(self):
        self.c.read()
        r = self.c.call("run_actions", pid=4242, window_id=777, steps=[
            {"find": {"role": "button"}, "as": "b"},
            {"for_each": "b", "do": [{"for_each": "b", "do": [{"for_each": "b", "do": [
                {"for_each": "b", "do": [{"tool": "press_key", "args": {"key": "Tab"}}]}]}]}]}])
        self.assertTrue(r.get("isError"))
        self.assertIn("nesting deeper", txt(r))

    def test_action_limit(self):
        self.c.read()
        steps = [{"tool": "press_key", "args": {"key": "Tab"}} for _ in range(25)]
        r = self.c.call("run_actions", pid=4242, window_id=777, steps=steps)
        self.assertIn("limit of 20", txt(r))

    def test_risky_step_inside_loop_is_still_gated(self):
        self.c.read()
        r = self.c.call("run_actions", pid=4242, window_id=777, steps=[
            {"find": {"role": "button"}, "as": "b"},
            {"for_each": "b", "do": [{"tool": "click", "args": {"target": "$item"}}]}])
        self.assertIn("REFUSED", txt(r))  # "Delete file" is the second button



class ReviewerIsRequired(unittest.TestCase):
    """0.9.3: a reviewer model is required; the gate never runs without one and the agent's own model is never used."""

    def send_click(self, c):
        c.call("fake_mutate", elements=[["button", "Send"], ["button", "Increment"], ["text", "x"]])
        s = c.read()
        return c.call("click", element_token=f"{s}:0", gate_ack=True)

    def serve(self, env, *extra):
        e = dict(os.environ, GUI_GATE_REVIEWER_ENV=os.path.join(os.path.dirname(os.path.abspath(__file__)), "no-such", "reviewer.env"))
        for k in ("GUI_GATE_REVIEWER", "GUI_GATE_REVIEWER_URL", "GUI_GATE_REVIEWER_MODEL", "GUI_GATE_REVIEWER_KEY",
                  "GUI_GATE_REVIEWER_KEY_FILE", "GUI_GATE_REVIEWER_PROTOCOL"):
            e.pop(k, None)
        e.update(env)
        return subprocess.run([BIN, "--child", "true", "--log", "none", *extra], input="", capture_output=True,
                              text=True, env=e, timeout=30)

    def test_no_reviewer_server_refuses_to_start(self):
        for env in ({}, {"GUI_GATE_REVIEWER_URL": "http://127.0.0.1:9/v1"},
                    {"GUI_GATE_REVIEWER_URL": "http://127.0.0.1:9/v1", "GUI_GATE_REVIEWER_MODEL": "m"},
                    {"GUI_GATE_REVIEWER_MODEL": "m", "GUI_GATE_REVIEWER_KEY": "k"}):
            p = self.serve(env)
            self.assertNotEqual(p.returncode, 0, env)
            self.assertEqual(p.stdout, "")  # nothing served
            self.assertIn("refusing to start", p.stderr)
            self.assertIn("GUI_GATE_REVIEWER_URL", p.stderr)
            self.assertIn("reviewer.env", p.stderr)
            self.assertIn("gui-gate check-reviewer", p.stderr)
        for pol in ("direct", "full", "light"):  # baselines need it too: no way to run without a reviewer
            self.assertNotEqual(self.serve({}, "--policy", pol).returncode, 0, pol)

    def test_off_is_rejected(self):
        for mode in ("off", "none", "0", "agent", "auto"):
            p = self.serve({"GUI_GATE_REVIEWER": mode, "GUI_GATE_REVIEWER_URL": "http://127.0.0.1:9/v1",
                            "GUI_GATE_REVIEWER_MODEL": "m", "GUI_GATE_REVIEWER_KEY": "k"})
            self.assertNotEqual(p.returncode, 0, mode)
            self.assertIn(f"GUI_GATE_REVIEWER={mode} is not supported", p.stderr)
            self.assertIn("cannot be turned off", p.stderr)

    def test_empty_or_missing_key_file_refuses(self):
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            kf = os.path.join(d, "reviewer.key")
            open(kf, "w").close()
            base = {"GUI_GATE_REVIEWER_URL": "http://127.0.0.1:9/v1", "GUI_GATE_REVIEWER_MODEL": "m"}
            p = self.serve(dict(base, GUI_GATE_REVIEWER_KEY_FILE=kf))
            self.assertNotEqual(p.returncode, 0)
            self.assertIn("is empty", p.stderr)
            p = self.serve(dict(base, GUI_GATE_REVIEWER_KEY_FILE=kf + ".gone"))
            self.assertNotEqual(p.returncode, 0)
            self.assertIn("cannot read the reviewer key file", p.stderr)

    def test_reviewer_env_file_configures_the_gate(self):
        import tempfile
        rev = _Rev("allow")
        with tempfile.TemporaryDirectory() as d:
            kf = os.path.join(d, "reviewer.key")
            with open(kf, "w") as f:
                f.write("secret-test-key\n")
            ef = os.path.join(d, "reviewer.env")
            with open(ef, "w") as f:
                f.write(f"# reviewer\nexport GUI_GATE_REVIEWER_URL=\"{rev.url}\"\nGUI_GATE_REVIEWER_MODEL='file-model'\n"
                        f"GUI_GATE_REVIEWER_KEY_FILE={kf}\n")
            # empty env vars count as unset: the values come from the file
            env = {"GUI_GATE_REVIEWER_ENV": ef, "GUI_GATE_REVIEWER_URL": "", "GUI_GATE_REVIEWER_MODEL": "",
                   "GUI_GATE_REVIEWER_KEY": ""}
            e = {k: v for k, v in os.environ.items() if not k.startswith("GUI_GATE_REVIEWER")}
            e.update({"GUI_GATE_REVIEWER_ENV": ef, "PYTHONPATH": os.path.dirname(os.path.dirname(os.path.abspath(__file__)))})
            p = subprocess.run([BIN, "check-reviewer", "--config-only"], capture_output=True, text=True, env=e, timeout=30)
            self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
            self.assertEqual(json.loads(p.stdout)["reviewer"], "api:chat:file-model")
            self.assertNotIn("secret-test-key", p.stdout + p.stderr)
            # the server, with only the file: it starts and its reviewer is the file's model
            c = Client("--human", "deny", "--task", "draft a reply to Bob", env=env)
            try:
                self.assertFalse(self.send_click(c).get("isError"))
                self.assertEqual(rev.seen[0]["model"], "file-model")
            finally:
                c.close()
                rev.close()

    def test_check_reviewer_not_configured(self):
        e = {k: v for k, v in os.environ.items() if not k.startswith("GUI_GATE_REVIEWER")}
        e["GUI_GATE_REVIEWER_ENV"] = "/nonexistent/reviewer.env"
        for args in ([], ["--config-only"]):
            p = subprocess.run([BIN, "check-reviewer", *args], capture_output=True, text=True, env=e, timeout=30)
            self.assertEqual(p.returncode, 1)
            out = json.loads(p.stdout)
            self.assertFalse(out["ok"])
            self.assertFalse(out["configured"])
            self.assertIn("not configured", out["error"])
            self.assertIn("will not start", out["error"])
            self.assertNotIn("agent", out["error"])
            self.assertIn("GUI_GATE_REVIEWER_URL", p.stderr)

    def test_sampling_client_is_never_asked(self):
        # a client that offers sampling: the reviewer is still only the configured API (here unreachable -> refused)
        ans = lambda m: {"role": "assistant", "model": "client-model",
                         "content": {"type": "text", "text": '{"decision": "allow", "reason": "x"}'}}
        c = Client("--human", "deny", "--task", "draft a reply to Bob", capabilities={"sampling": {}}, on_request=ans,
                   client_name="pi")
        try:
            r = self.send_click(c)
            self.assertTrue(r.get("isError"))
            self.assertEqual(r["structuredContent"]["gate"]["decision"], "deny")
            self.assertIn("reviewer did not answer", txt(r))
            self.assertEqual([m for m in c.requests if m["method"] == "sampling/createMessage"], [])
        finally:
            c.close()


class TerminalLineTests(unittest.TestCase):
    """0.9.5: a refused Enter in a terminal clears the typed command from the input line (og04 loop)."""

    def setUp(self):
        import shutil, tempfile, time
        self.tmp = tempfile.mkdtemp()
        self.old = os.path.join(self.tmp, "userdir")
        os.makedirs(self.old)
        old_t = time.time() - 3600
        os.utime(self.old, (old_t, old_t))
        fake_term = os.path.join(self.tmp, "fake-xterm")
        shutil.copy("/bin/sleep", fake_term)
        self.term = subprocess.Popen([fake_term, "300"])
        self.c = None

    def tearDown(self):
        self.term.kill()
        if self.c:
            self.c.close()

    def test_refused_enter_clears_line(self):
        pid = self.term.pid
        self.c = Client("--human", "deny", "--task", "clone the repo to my home folder", env={"FAKE_PID": str(pid)})
        self.c.call("get_window_state", pid=pid, window_id=777)
        cmd = f"mv {self.old} {self.old}.bak"
        r = self.c.call("type_text", pid=pid, window_id=777, text=cmd)
        self.assertFalse(r.get("isError"), txt(r))
        r = self.c.call("press_key", pid=pid, window_id=777, key="Return")
        self.assertTrue(r.get("isError"), txt(r))
        self.assertIn("cleared the refused command", txt(r))
        self.assertIn("do not move", txt(r))
        keys = json.loads(txt(self.c.call("fake_state")))["keys"]
        self.assertEqual(keys[-2:], ["ctrl+e", "ctrl+u"])
        ev = [e for e in self.c.events() if e.get("ev") == "line_cleared"]
        self.assertEqual([e["text"] for e in ev], [cmd])
        # the next (safe) command is checked on its own, not appended to the refused one
        self.c.call("type_text", pid=pid, window_id=777, text="ls")
        r = self.c.call("press_key", pid=pid, window_id=777, key="Return")
        self.assertFalse(r.get("isError"), txt(r))

    def test_rm_of_own_scratch_runs_rm_of_user_data_refused(self):
        import time
        pid = self.term.pid
        self.c = Client("--human", "deny", "--task", "clone the repo", env={"FAKE_PID": str(pid)})
        self.c.call("get_window_state", pid=pid, window_id=777)
        time.sleep(0.05)
        scratch = os.path.join(self.tmp, "scratch-clone")
        os.makedirs(scratch)  # created after the gate started: the agent's own work
        r = self.c.call("type_text", pid=pid, window_id=777, text=f"rm -rf {scratch}")
        self.assertFalse(r.get("isError"), txt(r))
        r = self.c.call("press_key", pid=pid, window_id=777, key="Return")
        self.assertFalse(r.get("isError"), txt(r))
        self.c.call("type_text", pid=pid, window_id=777, text=f"rm -rf {self.old}")
        r = self.c.call("press_key", pid=pid, window_id=777, key="Return")
        self.assertTrue(r.get("isError"), txt(r))
        self.assertIn("`rm` deletes", txt(r))

    def test_rename_task_authorizes_move(self):
        pid = self.term.pid
        self.c = Client("--human", "deny", "--task", f"rename the folder userdir to userdir2", env={"FAKE_PID": str(pid)})
        self.c.call("get_window_state", pid=pid, window_id=777)
        self.c.call("type_text", pid=pid, window_id=777, text=f"mv {self.old} {self.old}2")
        r = self.c.call("press_key", pid=pid, window_id=777, key="Return")
        self.assertFalse(r.get("isError"), txt(r))


class OcclusionHealTests(unittest.TestCase):
    """0.9.5: a token click refused because another control owns its point is retried once by the gate."""

    def tearDown(self):
        self.c.close()

    def state(self):
        return json.loads(txt(self.c.call("fake_state")))

    def test_open_menu_covering_target_escape_and_retry(self):
        self.c = Client("--human", "deny")
        s = self.c.read()
        self.c.call("fake_mutate", occlude_next=["menu", "Insert", "noaction"])
        r = self.c.call("click", element_token=f"{s}:0")
        self.assertFalse(r.get("isError"), txt(r))
        self.assertIn("pressed Escape", txt(r))
        st = self.state()
        self.assertIn("Escape", st["keys"])
        self.assertEqual(st["clicked"], ["Increment"])
        ev = [e for e in self.c.events() if e.get("ev") == "action"][-1]
        self.assertEqual(ev.get("occlusion_heal"), "escape_retry")
        self.assertFalse(ev.get("is_error"))

    def test_duplicate_object_same_label_clicked_by_desktop_point(self):
        self.c = Client("--human", "deny")
        s = self.c.read()
        self.c.call("fake_mutate", occlude_next=["menu", "Increment", "noaction"])
        r = self.c.call("click", element_token=f"{s}:0")
        self.assertFalse(r.get("isError"), txt(r))
        self.assertEqual(self.state()["clicked"], ["xy:161,71:desktop"])

    def test_focus_lost_token_click_retried_focus_free(self):
        # focus_within_target=true (a popup of the active window): bring_to_front never helped (m3: 0/41)
        self.c = Client("--human", "deny")
        s = self.c.read()
        self.c.call("fake_mutate", fg_fail_next=1)
        r = self.c.call("click", element_token=f"{s}:0")
        self.assertFalse(r.get("isError"), txt(r))
        self.assertIn("focus-free", txt(r))
        st = self.state()
        self.assertEqual(st["front"], [])
        self.assertEqual(st["modes"][-1], "background")
        self.assertEqual(st["clicked"], ["Increment"])

    def test_focus_lost_key_brought_to_front_and_retried_once(self):
        self.c = Client("--human", "deny")
        self.c.read()
        self.c.call("fake_mutate", fg_fail_next=1)
        r = self.c.call("press_key", pid=4242, window_id=777, key="Tab")
        self.assertFalse(r.get("isError"), txt(r))
        self.assertIn("brought it to the front", txt(r))
        self.assertEqual(self.state()["front"], [4242])
        self.c.call("fake_mutate", fg_fail_next=3)         # focus-free retry and front retry both fail -> error
        s = self.c.read(fresh=True)
        r = self.c.call("click", element_token=f"{s}:0")
        self.assertTrue(r.get("isError"))
        self.assertEqual(self.state()["clicked"], [])

    def test_heal_inside_batch_lets_the_batch_go_on(self):
        self.c = Client("--human", "deny")
        s = self.c.read()
        self.c.call("fake_mutate", occlude_next=["menu", "Insert", "noaction"])
        r = self.c.call("run_actions", pid=4242, window_id=777, steps=[
            {"tool": "click", "args": {"element_token": f"{s}:0"}},
            {"tool": "press_key", "args": {"key": "Tab"}}])
        self.assertFalse(r.get("isError"), txt(r))
        self.assertIn("2 GUI action(s) ran", txt(r))

    def test_other_owner_not_retried(self):
        self.c = Client("--human", "deny")
        s = self.c.read()
        self.c.call("fake_mutate", occlude_next=["table cell", "B4", "noaction"])
        r = self.c.call("click", element_token=f"{s}:0")
        self.assertTrue(r.get("isError"))
        self.assertEqual(self.state()["clicked"], [])

    def test_occluded_token_click_activated_focus_free(self):
        # m3: 66 token clicks refused for occlusion, many by unnamed owners the old heals never matched
        self.c = Client("--human", "deny")
        s = self.c.read()
        self.c.call("fake_mutate", occlude_next=["panel", "(unnamed)"])
        r = self.c.call("click", element_token=f"{s}:0")
        self.assertFalse(r.get("isError"), txt(r))
        self.assertIn("accessibility action", txt(r))
        st = self.state()
        self.assertEqual(st["modes"][-1], "background")
        self.assertEqual(st["clicked"], ["Increment"])
        ev = [e for e in self.c.events() if e.get("ev") == "action"][-1]
        self.assertEqual(ev.get("occlusion_heal"), "background")


class TreeDiffTests(unittest.TestCase):
    """0.9.5: auto-observe sends only the rows of the window the agent has not seen yet."""

    def tearDown(self):
        self.c.close()

    def test_auto_observe_sends_changed_rows_only(self):
        self.c = Client("--human", "deny")
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:0")  # Increment: only the count row changes
        t = txt(r)
        self.assertIn("Only rows that changed", t)
        self.assertIn('count=1', t)
        self.assertNotIn('"Delete file"', t)
        obs = [e for e in self.c.events() if e.get("ev") == "obs" and e.get("auto")][-1]
        self.assertTrue(obs["diff"])
        self.assertLess(obs["rows_sent"], obs["rows"])
        # the unchanged row's index is valid with the new snapshot id
        newsnap = t.split("(snapshot ")[1].split(";")[0]
        r2 = self.c.call("click", element_token=f"{newsnap}:0")
        self.assertFalse(r2.get("isError"), txt(r2))
        self.assertIn("count=2", txt(r2))

    def test_diff_off_by_env(self):
        self.c = Client("--human", "deny", env={"GUI_GATE_TREE_DIFF": "0"})
        s = self.c.read()
        t = txt(self.c.call("click", element_token=f"{s}:0"))
        self.assertNotIn("Only rows that changed", t)
        self.assertIn('"Delete file"', t)


if __name__ == "__main__":
    unittest.main()
