"""Security regressions (v2-sec), end to end: the real gate binary + tests/fake_driver.py.

Each test drives the dangerous path and checks on the fake driver whether the dangerous step really happened
(a click on "Delete file", a file written by a "read-only" tool, an action sent while a person answers, an
unmodelled action sent twice after its outcome was unknown).

  1. a conflicting approval form (allow: true AND decision: deny) is a refusal (fail closed)
  2. a person's approval does not survive a new operator task version
  3. a read-only tool that carries a write-path argument (screenshot_out_file ...) is not let through as a read
  4. while a person is answering, the agent cannot end the request (gate_reset) and unfreeze input
  5. an unmodelled (pass-through) action whose outcome was unknown is not re-sent blind

Run:  python3 -m unittest test_sec_regressions -v   (from tests/, after cargo build)
"""
import json
import os
os.environ.setdefault("GUI_GATE_DIALOG", "none")
import shutil
import sys
import tempfile
import time
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from test_gate import Client, txt  # noqa: E402
from test_actions_e2e import ALLOW, Modern, Person  # noqa: E402

CONFLICT = {"action": "accept", "content": {"allow": True, "decision": "deny"}}


def state(c):
    return json.loads(txt(c.call("fake_state")))


class ConflictingApprovalForm(unittest.TestCase):
    def test_modern_conflicting_form_is_a_refusal(self):
        p = Person(approve=CONFLICT)
        c = Modern(person=p)
        try:
            s = c.read()
            r = c.call("click", element_token=f"{s}:1")  # "Delete file": high risk -> a person
            self.assertEqual(len(p.asked), 1)
            self.assertTrue(r.get("isError"), txt(r))
            self.assertNotIn("Delete file", state(c)["clicked"], "a conflicting form must never approve")
        finally:
            c.close()

    def test_other_ambiguous_forms_are_refusals(self):
        for content in ({"decision": "allow", "allow": False}, {"decision": "allow", "allow": "true"},
                        {"decision": ["allow"]}, {"decision": "Allow"}, {"allow": 1}, {"decision": "allow", "choice": "deny"}):
            c = Modern(person=Person(approve={"action": "accept", "content": content}))
            try:
                s = c.read()
                r = c.call("click", element_token=f"{s}:1")
                self.assertTrue(r.get("isError"), (content, txt(r)))
                self.assertNotIn("Delete file", state(c)["clicked"], content)
            finally:
                c.close()

    def test_legacy_elicitation_conflicting_form_is_a_refusal(self):
        c = Client(capabilities={"elicitation": {}}, on_request=lambda m: CONFLICT)
        try:
            s = c.read()
            r = c.call("click", element_token=f"{s}:1")
            self.assertEqual(len(c.requests), 1, "the person was asked")
            self.assertTrue(r.get("isError"), txt(r))
            self.assertNotIn("Delete file", state(c)["clicked"])
        finally:
            c.close()


class ApprovalBoundToTaskVersion(unittest.TestCase):
    def test_answer_from_an_older_task_version_is_not_used(self):
        d = tempfile.mkdtemp(prefix="gg-tv-")
        tf = os.path.join(d, "task.txt")
        with open(tf, "w") as fh:
            fh.write("gui-pi-task/1 version=1\n--- user 1 2026-10-26T00:00:00Z initial request\ntidy up the window\n")
        p = Person(approve=ALLOW)
        c = Modern("--task-file", tf, person=p)
        try:
            s = c.read()
            m = c.raw("tools/call", {"name": "click", "arguments": {"element_token": f"{s}:1"}})
            self.assertEqual(m["result"].get("resultType"), "input_required", m)
            st = m["result"]["requestState"]
            # the user writes again: a new task version, while the approval form is still open
            with open(tf, "w") as fh:
                fh.write("gui-pi-task/1 version=2\n--- user 1 2026-10-26T00:00:00Z initial request\ntidy up the window\n"
                         "--- user 2 2026-10-26T00:01:00Z later message\nstop, do not delete anything\n")
            os.utime(tf, (time.time() + 10, time.time() + 10))
            r = c.raw("tools/call", {"name": "click", "arguments": {"element_token": f"{s}:1"},
                                     "inputResponses": {"human": ALLOW}, "requestState": st})
            self.assertNotIn("Delete file", state(c)["clicked"], "an approval given under task version 1 executed under version 2")
            res = r.get("result") or {}
            # the person is asked again (about the new version) or it is refused; never executed
            self.assertTrue(res.get("resultType") == "input_required" or res.get("isError") or "error" in r, r)
            if res.get("resultType") == "input_required":
                meta = res["inputRequests"]["human"]["params"]["_meta"]["io.github.cszf1/gui-gate"]
                self.assertIn("asked_again", meta)
        finally:
            c.close()
            shutil.rmtree(d, ignore_errors=True)


class ReadOnlyToolsDoNotWrite(unittest.TestCase):
    def test_write_path_on_a_read_is_refused_and_nothing_is_written(self):
        d = tempfile.mkdtemp(prefix="gg-ro-")
        c = Client("--human", "deny")
        try:
            victim = os.path.join(d, "important.txt")
            with open(victim, "w") as fh:
                fh.write("keep me")
            out = os.path.join(d, "shot.png")
            for name, a in (("get_window_state", {"pid": 4242, "window_id": 777, "screenshot_out_file": out}),
                            ("get_window_state", {"pid": 4242, "window_id": 777, "screenshot_out_file": victim}),
                            ("list_windows", {"output_path": out}),
                            ("verify_state", {"save_path": out, "expect": [{"element": {"selector": {"label_contains": "x"}}}]})):
                r = c.call(name, **a)
                self.assertTrue(r.get("isError"), (name, txt(r)))
                self.assertEqual(r["structuredContent"]["status"], "blocked", (name, txt(r)))
            self.assertFalse(os.path.exists(out), "a read-only tool wrote a file")
            with open(victim) as fh:
                self.assertEqual(fh.read(), "keep me", "a read-only tool overwrote a user file")
            # plain reads still work
            self.assertFalse(c.call("get_window_state", pid=4242, window_id=777).get("isError"))
            self.assertFalse(c.call("list_windows").get("isError"))
        finally:
            c.close()
            shutil.rmtree(d, ignore_errors=True)

    def test_write_path_inside_run_actions_is_refused(self):
        d = tempfile.mkdtemp(prefix="gg-ro-")
        c = Client("--human", "deny")
        try:
            out = os.path.join(d, "shot.png")
            c.call("run_actions", actions=[{"tool": "get_window_state", "args": {"pid": 4242, "window_id": 777, "screenshot_out_file": out}}])
            self.assertFalse(os.path.exists(out), "a read inside run_actions wrote a file")
        finally:
            c.close()
            shutil.rmtree(d, ignore_errors=True)


class AgentCannotEndAPendingRequest(unittest.TestCase):
    def test_gate_reset_does_not_unfreeze_or_approve(self):
        c = Modern(person=Person(approve=ALLOW))
        try:
            s = c.read()
            m = c.raw("tools/call", {"name": "click", "arguments": {"element_token": f"{s}:1"}})  # Delete file -> person
            st = m["result"]["requestState"]
            # the agent (no client marker) tries to cancel the open question
            c.call("gate_reset")
            s2 = c.read()
            r = c.call("click", element_token=f"{s2}:0")
            self.assertEqual(r["structuredContent"]["status"], "blocked", txt(r))
            self.assertIn("a person is answering", txt(r))
            r = c.call("set_clipboard", text="x")
            self.assertEqual(r["structuredContent"]["status"], "blocked", txt(r))
            self.assertEqual(state(c)["clicked"], [], "input was sent while the person was still answering")
            # the old request was voided by the reset: even the person's allow does not execute it now
            c.raw("tools/call", {"name": "click", "arguments": {"element_token": f"{s}:1"}, "inputResponses": {"human": ALLOW}, "requestState": st})
            self.assertNotIn("Delete file", state(c)["clicked"])
        finally:
            c.close()

    def test_wrong_or_answerless_retries_do_not_end_it(self):
        c = Modern(person=Person(approve=ALLOW))
        try:
            s = c.read()
            m = c.raw("tools/call", {"name": "click", "arguments": {"element_token": f"{s}:1"}})
            st = m["result"]["requestState"]
            # a call for another target carrying the state, then the right call twice WITHOUT an answer
            c.raw("tools/call", {"name": "click", "arguments": {"element_token": f"{s}:0"}, "inputResponses": {"human": ALLOW}, "requestState": st})
            for _ in range(2):
                bad = c.raw("tools/call", {"name": "click", "arguments": {"element_token": f"{s}:1"}, "requestState": st})
                self.assertIn("error", bad)
            s2 = c.read()
            r = c.call("click", element_token=f"{s2}:0")
            self.assertEqual(r["structuredContent"]["status"], "blocked", txt(r))
            self.assertEqual(state(c)["clicked"], [], "the question was taken away from the person")
        finally:
            c.close()

    def test_freeze_ends_on_timeout(self):
        c = Modern("--human-timeout", "1", person=Person(approve=ALLOW))
        try:
            s = c.read()
            c.raw("tools/call", {"name": "click", "arguments": {"element_token": f"{s}:1"}})
            c.call("gate_reset")
            time.sleep(1 + 5 + 0.5)  # deadline + grace: the timeout ends it (= refusal)
            s = c.read()
            r = c.call("click", element_token=f"{s}:0")
            self.assertFalse(r.get("isError"), txt(r))
            self.assertEqual(state(c)["clicked"], ["Increment"])
        finally:
            c.close()


class UnknownOutcomeNotReSent(unittest.TestCase):
    def test_unmodelled_action_is_not_re_sent_after_unknown(self):
        c = Client("--human", "deny")
        try:
            c.call("fake_mutate", garbage_on="set_clipboard")
            r = c.call("set_clipboard", text="pay 100")
            self.assertEqual(r["structuredContent"]["status"], "unknown", txt(r))
            r = c.call("set_clipboard", text="pay 100")
            self.assertEqual(r["structuredContent"]["status"], "blocked", txt(r))
            self.assertEqual(state(c)["passed"], [], "an action with an unknown outcome was re-sent blind")
            # after the gate made the agent look, a deliberate re-send is decided afresh
            r = c.call("set_clipboard", text="pay 100")
            self.assertEqual(r["structuredContent"]["status"], "executed", txt(r))
            self.assertEqual(len(state(c)["passed"]), 1)
        finally:
            c.close()

    def test_same_unmodelled_effect_with_reordered_args_is_caught(self):
        c = Client("--human", "deny")
        try:
            c.call("fake_mutate", garbage_on="launch_app")
            r = c.call("launch_app", app_name="gedit", args=["a.txt"])
            self.assertEqual(r["structuredContent"]["status"], "unknown", txt(r))
            self.assertEqual(state(c)["passed"], [])
            r = c.call("launch_app", args=["a.txt"], app_name="gedit")
            self.assertEqual(r["structuredContent"]["status"], "blocked", txt(r))
            self.assertEqual(state(c)["passed"], [])
        finally:
            c.close()


if __name__ == "__main__":
    unittest.main()
