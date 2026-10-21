"""v0.4: authorization, deny-and-continue, observation budget, server-side batching (run_actions)."""
import json
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from test_gate import Client, txt  # noqa: E402



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

    def test_task_wording_authorizes(self):
        self.c = Client("--human", "deny", "--task", "Please delete the file in this window")
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:1")
        self.assertFalse(r.get("isError"), txt(r))
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
        self.assertEqual(r["structuredContent"]["batch"]["read"], ['counter: text "count=1" [actions=[click]]'])

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


if __name__ == "__main__":
    unittest.main()


class ReviewerReusesTheAgentsModel(unittest.TestCase):
    """0.9: by default the reviewer is ONE context-free request to the model the user's agent runs."""

    def send_click(self, c):
        c.call("fake_mutate", elements=[["button", "Send"], ["button", "Increment"], ["text", "x"]])
        s = c.read()
        return c.call("click", element_token=f"{s}:0", gate_ack=True)

    def test_mcp_sampling(self):
        for decision, blocked in (("deny", True), ("allow", False)):
            ans = lambda m, d=decision: {"role": "assistant", "model": "client-model",
                                         "content": {"type": "text", "text": '{"decision": "%s", "reason": "x"}' % d}}
            c = Client("--human", "deny", "--task", "draft a reply to Bob", capabilities={"sampling": {}}, on_request=ans,
                       env={"GUI_GATE_REVIEWER_URL": ""})
            try:
                r = self.send_click(c)
                self.assertEqual(bool(r.get("isError")), blocked, txt(r))
                req = [m for m in c.requests if m["method"] == "sampling/createMessage"]
                self.assertEqual(len(req), 1)
                p = req[0]["params"]
                self.assertEqual(p["includeContext"], "none")  # no conversation, one stand-alone request
                self.assertEqual(len(p["messages"]), 1)
                self.assertIn("draft a reply to Bob", p["messages"][0]["content"]["text"])
                self.assertNotIn("gate_ack", p["messages"][0]["content"]["text"])
                rev = [e for e in c.events() if e.get("ev") == "client"][0]
                self.assertEqual(rev["reviewer"], "agent:sampling")
            finally:
                c.close()

    def test_agent_cli_one_shot(self):
        import tempfile, stat
        d = tempfile.mkdtemp()
        argf = os.path.join(d, "args.json")
        pi = os.path.join(d, "pi")
        with open(pi, "w") as f:
            f.write("#!%s\nimport json,os,sys\njson.dump({'argv': sys.argv[1:], 'child': os.environ.get('GUI_GATE_REVIEWER_CHILD')}, open(%r,'w'))\n"
                    "print('{\"decision\": \"deny\", \"reason\": \"not asked\"}')\n" % (sys.executable, argf))
        os.chmod(pi, os.stat(pi).st_mode | stat.S_IEXEC)
        c = Client("--human", "deny", "--task", "draft a reply to Bob", client_name="pi",
                   env={"PATH": d + os.pathsep + os.environ["PATH"], "GUI_GATE_REVIEWER_URL": ""})
        try:
            r = self.send_click(c)
            self.assertTrue(r.get("isError"))
            self.assertIn("not asked", txt(r))
            got = json.load(open(argf))
            for flag in ("-p", "--no-session", "--no-tools", "--no-mcp", "--no-context-files"):
                self.assertIn(flag, got["argv"])
            self.assertEqual(got["child"], "1")  # the nested agent never starts a reviewer of its own
            self.assertIn("draft a reply to Bob", got["argv"][-1])
        finally:
            c.close()

    def test_no_backend_refuses(self):
        c = Client("--human", "deny", "--task", "draft a reply to Bob", env={"GUI_GATE_REVIEWER_URL": "", "GUI_GATE_REVIEWER": "api"})  # api mode, no endpoint
        try:
            r = self.send_click(c)
            self.assertTrue(r.get("isError"))
            self.assertIn("no reviewer", txt(r))
        finally:
            c.close()
