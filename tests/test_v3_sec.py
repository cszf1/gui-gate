"""v0.10.3 security / person / role regressions (review 2026-10-26 v2, sections 3, 4, 7), end to end.

The real gate binary + tests/fake_driver.py. Every test counts what the DRIVER received (FAKE_ACTION_LOG: one line
per state-changing call the fake driver got, written before it acts or dies, so it survives driver restarts), not
only what the gate replied. FAKE_CTL plays a person changing the desktop outside the agent.

  1. every real send (incl. the background -> foreground retry) passes the same final check
  2. an unknown outcome is a ledger entry: not cleared before a successful re-read, never by a failed one, and a
     non-idempotent effect needs a person's confirmation even after a read
  3. an approval is bound to the business premise it was given on (recipient / body rows): changed -> void, ask again
  5. a review the value model chose that gets no answer: a person when attended, refused when not (never direct)
  6. person burden: waiting != active time; an expired MRTR request with no retry is not asked again; one record and
     one end per request; after a takeover the gate re-reads and never replays, stale reads are not acted on
  7. an explicit GUI_GATE_ROLE_<NAME>_ENABLE=0 wins over a configured endpoint; role calls have a total deadline
  9. structured status values

Run:  python3 -m unittest test_v3_sec -v   (from tests/, after cargo build)
"""
import http.server
import json
import os
os.environ.setdefault("GUI_GATE_DIALOG", "none")
import shutil
import sys
import tempfile
import threading
import time
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from test_gate import Client, txt  # noqa: E402
from test_actions_e2e import ALLOW, DENY, DONE, GG, Modern, Person, gate  # noqa: E402
from test_v04 import _Rev  # noqa: E402

TASK_HDR = "gui-pi-task/1 version={v}\n--- user 1 Mon 2026-10-26 8:00 AM CST (UTC+08:00) initial request\n{t}\n"


class Env:
    """A temp dir with the driver action log and the 'person outside the agent' control file."""
    def __init__(self):
        self.d = tempfile.mkdtemp(prefix="gg-v3-")
        self.log = os.path.join(self.d, "actions.jsonl")
        self.ctl = os.path.join(self.d, "ctl.json")
        self.env = {"FAKE_ACTION_LOG": self.log, "FAKE_CTL": self.ctl}

    def acts(self, tool=None):
        if not os.path.exists(self.log):
            return []
        with open(self.log) as fh:
            out = [json.loads(l) for l in fh if l.strip()]
        return [a for a in out if tool is None or a["tool"] == tool]

    def person_does(self, **c):
        with open(self.ctl, "w") as fh:
            json.dump(c, fh)

    def close(self):
        shutil.rmtree(self.d, ignore_errors=True)


class _SlowRole:
    """An OpenAI-compatible role endpoint that answers slowly (or with a fixed JSON) and counts requests."""
    def __init__(self, delay=0.0, reply=None):
        seen = self.seen = []

        class H(http.server.BaseHTTPRequestHandler):
            def do_POST(self):
                self.rfile.read(int(self.headers["Content-Length"]))
                seen.append(time.time())
                time.sleep(delay)
                out = json.dumps({"choices": [{"message": {"content": json.dumps(reply or {"probe": "windows"})}}]}).encode()
                try:
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.end_headers()
                    self.wfile.write(out)
                except OSError:
                    pass

            def log_message(self, *a):
                pass
        self.srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
        self.srv.daemon_threads = True
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()
        self.url = f"http://127.0.0.1:{self.srv.server_port}/v1/chat/completions"

    def close(self):
        self.srv.shutdown()


# ---------------------------------------------------------------------------------------------------- 1


class EverySendIsChecked(unittest.TestCase):
    def test_task_change_between_background_failure_and_foreground_retry(self):
        """Background input is unavailable (nothing delivered); while that happens the user's task changes. The
        foreground retry is a new send: it must pass the final check again, which voids the approval."""
        e = Env()
        tf = os.path.join(e.d, "task.txt")
        with open(tf, "w") as fh:
            fh.write(TASK_HDR.format(v=1, t="tidy up the window"))
        hook = os.path.join(e.d, "bgfail.json")
        with open(hook, "w") as fh:
            json.dump({"write": [tf, TASK_HDR.format(v=2, t="tidy up the window") +
                                 "--- user 2 Mon 2026-10-26 8:01 AM CST (UTC+08:00) later message\ndo not send anything\n", 20]}, fh)
        p = Person(approve=ALLOW)
        c = Modern("--task-file", tf, person=p, env=dict(e.env, FAKE_NO_BG="1", FAKE_ON_BGFAIL=hook))
        try:
            c.read()
            r = c.call("hotkey", pid=4242, window_id=777, keys=["ctrl", "enter"])  # send: medium risk -> person
            self.assertEqual(len(p.asked), 1, txt(r))
            hk = e.acts("hotkey")
            self.assertEqual(len(hk), 1, f"the undelivered background attempt only; got {hk}")
            self.assertNotEqual(hk[0]["args"].get("delivery_mode"), "foreground")
            self.assertTrue(r.get("isError"), txt(r))
        finally:
            c.close(); e.close()

    def test_unchanged_foreground_retry_runs_and_is_checked(self):
        e = Env()
        p = Person(approve=ALLOW)
        c = Modern(person=p, env=dict(e.env, FAKE_NO_BG="1"))
        try:
            c.read()
            r = c.call("hotkey", pid=4242, window_id=777, keys=["ctrl", "enter"])
            self.assertFalse(r.get("isError"), txt(r))
            modes = [a["args"].get("delivery_mode") for a in e.acts("hotkey")]
            self.assertEqual(modes, [None, "foreground"], modes)
            checks = [ev for ev in c.events() if ev.get("ev") == "send_check" and ev.get("tool") == "hotkey"]
            self.assertEqual(len(checks), 2, "each of the two sends passed the final check")
            self.assertEqual(checks[1].get("retry"), "not_delivered")
        finally:
            c.close(); e.close()


# ---------------------------------------------------------------------------------------------------- 2


class UnknownLedger(unittest.TestCase):
    SEND = [["button", "Send"], ["button", "Increment"], ["text", "To: bob@example.com"]]

    def setup(self, *extra, person=None, rows=None, fail_reads_after_restart=False):
        self.e = Env()
        self.rev = _Rev("allow")
        boot = os.path.join(self.e.d, "boot.json")
        with open(boot, "w") as fh:
            json.dump({"elements": rows or self.SEND}, fh)  # the restarted driver shows the same window
        self.c = Modern("--task", "send the reply to Bob", *extra, person=person, reviewer_url=self.rev.url,
                        env=dict(self.e.env, FAKE_BOOT=boot))
        self.c.call("fake_mutate", crash_on="click")
        s = self.c.read()
        if fail_reads_after_restart:
            with open(boot, "w") as fh:
                # the restarted driver's first three reads fail (the gate's own look right after the crash and its
                # looks on the re-sends); the gate still has its read from before the crash
                json.dump({"elements": rows or self.SEND, "read_fail": 3}, fh)
        r = self.c.call("click", element_token=f"{s}:0")  # (Send: the reviewer allows) the driver dies while clicking
        self.assertEqual((r.get("structuredContent") or {}).get("status"), "unknown", txt(r))
        return s

    def tearDown(self):
        self.c.close(); self.rev.close(); self.e.close()

    def sends(self):
        return len(self.e.acts("click"))

    def test_failed_read_does_not_clear_the_record(self):
        # the action happened, its answer was lost, and every read since fails: it must never be sent again
        for rows in (self.SEND, [["button", "Increment"], ["button", "Plum"], ["text", "count=0"]]):
            self.setup("--human", "deny", rows=rows, fail_reads_after_restart=True)
            for _ in range(3):
                r = self.c.call("click", pid=4242, window_id=777, element_index=0)
                self.assertTrue(r.get("isError"), txt(r))
            self.assertEqual(self.sends(), 1, f"{rows[0]}: a failed re-read must never unlock the unknown send")
            self.tearDown()
        self.setup("--human", "deny")  # for tearDown

    def test_unmodelled_action_failed_window_list_does_not_clear(self):
        # the review's case: on the path without a GUI target model the record was removed BEFORE the window list was
        # read, so a failed list still unlocked the next re-send
        self.e = Env()
        self.rev = _Rev("allow")
        self.c = Modern("--human", "deny", env=self.e.env)
        self.c.call("fake_mutate", garbage_on="set_clipboard")
        r = self.c.call("set_clipboard", text="pay 100")
        self.assertEqual(r["structuredContent"]["status"], "unknown", txt(r))
        self.c.call("fake_mutate", list_fail=5)
        for _ in range(3):
            r = self.c.call("set_clipboard", text="pay 100")
            self.assertEqual(r["structuredContent"]["status"], "blocked", txt(r))
        self.assertEqual(len(self.e.acts("set_clipboard")), 1, "the window list never came back: it must not be sent again")

    def test_non_idempotent_needs_a_person_even_after_a_read(self):
        self.setup("--human", "deny")
        for _ in range(3):
            self.c.read()
            r = self.c.call("click", pid=4242, window_id=777, element_index=0)
            self.assertTrue(r.get("isError"), txt(r))
        self.assertEqual(self.sends(), 1, "Send had an unknown outcome: reading the window is not proof it did not happen")

    def test_person_confirms_then_it_is_sent(self):
        p = Person(approve=ALLOW)
        self.setup(person=p)
        self.c.read()
        r = self.c.call("click", pid=4242, window_id=777, element_index=0)
        self.assertFalse(r.get("isError"), txt(r))
        self.assertEqual(self.sends(), 2)
        self.assertTrue(any("unknown" in (q["message"] + json.dumps(q["_meta"][GG])).lower() for q in p.asked), p.asked)

    def test_idempotent_step_unlocks_after_a_successful_read(self):
        self.setup("--human", "deny", rows=[["button", "Increment"], ["button", "Plum"], ["text", "count=0"]])
        # the gate read the restarted driver's window right after the unknown outcome (shown with the result): a
        # deliberate re-send of a harmless step is decided afresh
        r = self.c.call("click", pid=4242, window_id=777, element_index=0)
        self.assertFalse(r.get("isError"), txt(r))
        self.assertEqual(self.sends(), 2)


# ---------------------------------------------------------------------------------------------------- 3


class ApprovalPremise(unittest.TestCase):
    def test_recipient_changed_after_approval_voids_it(self):
        e = Env()

        def person(params):
            # while the person reads the form, the To: field changes (another tab, an autocomplete, a sync)
            e.person_does(elements=[["button", "Send"], ["text", "To: eve@example.net"], ["text", "Body: hello"]])
            return ALLOW
        p = Person(approve=person)
        c = Modern("--task", "tidy the drafts", person=p, env=e.env)
        try:
            c.call("fake_mutate", elements=[["button", "Send"], ["text", "To: bob@example.com"], ["text", "Body: hello"]])
            c.read()
            r = c.call("click", pid=4242, window_id=777, element_index=0)
            self.assertEqual(len(e.acts("click")), 0, "approved for bob@, the window now sends to eve@")
            self.assertTrue(r.get("isError"), txt(r))
            self.assertIn("premise", json.dumps(r.get("structuredContent")))
            meta = p.asked[0]["_meta"][GG]
            self.assertIn("bob@example.com", json.dumps(meta.get("premise")))
            # sent again: the person is asked again, about eve@
            p.answers["approve"] = ALLOW
            r = c.call("click", pid=4242, window_id=777, element_index=0)
            self.assertEqual(len(p.asked), 2)
            self.assertIn("eve@example.net", json.dumps(p.asked[1]["_meta"][GG].get("premise")))
            self.assertFalse(r.get("isError"), txt(r))
            self.assertEqual(len(e.acts("click")), 1)
        finally:
            c.close(); e.close()

    def test_unreadable_premise_is_said_so(self):
        p = Person(approve=DENY)
        c = Modern(person=p)
        try:
            # a send hotkey to a window the gate never read: nothing to bind the approval to
            c.call("hotkey", pid=4242, keys=["ctrl", "enter"])
            self.assertEqual(len(p.asked), 1)
            pr = p.asked[0]["_meta"][GG].get("premise") or {}
            self.assertEqual(pr.get("readable"), False, pr)
        finally:
            c.close()


# ---------------------------------------------------------------------------------------------------- 5


class ReviewFailureNeverDirect(unittest.TestCase):
    def doubtful(self, *extra, person=None):
        # a click that already failed is sent again: the value model asks the gate's reviewer (dummy: unreachable)
        e = Env()
        c = Modern("--task", "increment the counter", *extra, person=person, env=dict(e.env, GUI_GATE_LOOP_BREAKER="0"))
        c.call("fake_mutate", fail_next=1)
        s = c.read()
        c.call("click", element_token=f"{s}:2")
        s = c.read()
        r = c.call("click", element_token=f"{s}:2")
        return c, e, r

    def test_unattended_is_refused(self):
        c, e, r = self.doubtful("--human", "deny")
        try:
            ev = c.actions()[-1]
            self.assertIn("reviewer_down", ev["trail"], ev)
            self.assertTrue(r.get("isError"), txt(r))
            self.assertEqual(len(e.acts("click")), 1, "only the first (failed) click reached the driver")
        finally:
            c.close(); e.close()

    def test_attended_asks_the_person(self):
        for ans, sent in ((ALLOW, 2), (DENY, 1)):
            p = Person(approve=ans)
            c, e, r = self.doubtful(person=p)
            try:
                self.assertEqual(len(p.asked), 1, txt(r))
                self.assertEqual(len(e.acts("click")), sent, (ans, txt(r)))
            finally:
                c.close(); e.close()


# ---------------------------------------------------------------------------------------------------- 6


class PersonRecords(unittest.TestCase):
    def test_wait_is_not_active_time(self):
        def slow(params):
            time.sleep(1.2)
            return ALLOW
        c = Modern(person=Person(approve=slow))
        try:
            s = c.read()
            c.call("click", element_token=f"{s}:1")
            b = [ev for ev in c.events() if ev.get("ev") == "person_burden"][-1]
            self.assertGreaterEqual(b["wait_s"], 1.0)
            self.assertIsNone(b["active_s"], "no interaction evidence: active time is missing, not the wait")
            self.assertGreaterEqual(b["active_upper_s"], 1.0)
        finally:
            c.close()

    def test_client_measured_active_time(self):
        ans = {"action": "accept", "content": {"decision": "allow"}, "_meta": {GG: {"active_ms": 400}}}
        c = Modern(person=Person(approve=ans))
        try:
            s = c.read()
            c.call("click", element_token=f"{s}:1")
            b = [ev for ev in c.events() if ev.get("ev") == "person_burden"][-1]
            self.assertAlmostEqual(b["active_s"], 0.4, places=2)
        finally:
            c.close()

    def test_expired_without_retry_is_not_asked_again_and_ends_once(self):
        c = Modern("--human-timeout", "1", person=Person(approve=ALLOW))
        try:
            s = c.read()
            m = c.raw("tools/call", {"name": "click", "arguments": {"element_token": f"{s}:1"}})
            self.assertEqual(m["result"]["resultType"], "input_required")
            time.sleep(7)  # deadline 1 s + grace 5 s: the client never retried
            s = c.read()
            m2 = c.raw("tools/call", {"name": "click", "arguments": {"element_token": f"{s}:1"}})
            res = m2["result"]
            self.assertNotEqual(res.get("resultType"), "input_required", "the same effect was asked again")
            self.assertEqual(res["structuredContent"].get("human_outcome"), "not_asked_again", res)
            self.assertNotIn("Delete file", c.clicked())
            starts = [ev for ev in c.events() if ev.get("ev") == "person_request"]
            ends = [ev for ev in c.events() if ev.get("ev") == "person_request_end"]
            self.assertEqual(len(starts), 1, starts)
            self.assertEqual([x["terminal"] for x in ends], ["expired_no_retry"], ends)
            self.assertEqual(ends[0]["req"], starts[0]["req"])
            for k in ("task_version", "kind", "effect", "deadline_ms"):
                self.assertIn(k, starts[0])
        finally:
            c.close()

    def test_reset_void_then_answer_ends_once(self):
        c = Modern(person=Person(approve=ALLOW))
        try:
            s = c.read()
            m = c.raw("tools/call", {"name": "click", "arguments": {"element_token": f"{s}:1"}})
            st = m["result"]["requestState"]
            c.call("gate_reset")
            c.raw("tools/call", {"name": "click", "arguments": {"element_token": f"{s}:1"},
                                 "inputResponses": {"human": ALLOW}, "requestState": st})
            self.assertNotIn("Delete file", c.clicked())
            ends = [ev for ev in c.events() if ev.get("ev") == "person_request_end"]
            self.assertEqual(len(ends), 1, ends)
            self.assertEqual(ends[0]["terminal"], "void")
        finally:
            c.close()

    def test_takeover_rereads_and_never_replays(self):
        e = Env()
        home = os.path.join(e.d, "home")
        os.makedirs(home)
        doc = os.path.join(home, "notes.txt")
        with open(doc, "w") as fh:
            fh.write("before")
        old = time.time() - 3600
        os.utime(doc, (old, old))

        def person(params):
            # the person logs in, edits a file and leaves another window focused
            e.person_does(elements=[["text", "Signed in as bob"], ["button", "Plum"], ["button", "Sign out"]],
                          front=999, front_title="Mail", write=[doc, "edited by the person", 2])
            return DONE
        p = Person(takeover=person)
        c = Modern(person=p, env=dict(e.env, HOME=home))
        try:
            c.call("fake_mutate", elements=[["password text", "Password"], ["button", "Plum"], ["button", "Sign out"]])
            s = c.read()
            r = c.call("type_text", element_token=f"{s}:0", text="hunter2")
            self.assertEqual(r["structuredContent"]["status"], "done_by_person", txt(r))
            self.assertEqual(e.acts("type_text"), [], "the gate never sends the step the person took over")
            rr = r["structuredContent"].get("reread") or {}
            self.assertIn("Signed in as bob", json.dumps(rr.get("window")))
            self.assertEqual(rr.get("focused_window"), 999, rr)
            self.assertIn("notes.txt", json.dumps(rr.get("files_changed")), rr)
            # a token from before the takeover: the person changed things since, so it is not acted on
            r2 = c.call("click", element_token=f"{s}:1")
            self.assertTrue(r2.get("isError"), txt(r2))
            self.assertEqual(e.acts("click"), [], "a click planned on the pre-takeover window reached the driver")
            # the same step again: never replayed silently (here: a person again, who declines)
            p.answers["takeover"] = {"action": "accept", "content": {"done": False}}
            c.call("type_text", element_token=f"{s}:0", text="hunter2")
            self.assertEqual(e.acts("type_text"), [])
        finally:
            c.close(); e.close()


# ---------------------------------------------------------------------------------------------------- 7


class Roles(unittest.TestCase):
    def test_explicit_disable_wins_over_endpoint(self):
        srv = _SlowRole(reply={"final": {"decision": "done", "reasoning": "x"}})
        env = {"GUI_GATE_VERIFY_PLUS": "1", "GUI_GATE_ROLE_VERIFIER_URL": srv.url, "GUI_GATE_ROLE_VERIFIER_MODEL": "m",
               "GUI_GATE_ROLE_VERIFIER_KEY": "k", "GUI_GATE_ROLE_VERIFIER_ENABLE": "0", "GUI_GATE_ROLES_ENABLE": "verifier"}
        c = Client("--human", "deny", "--task", "save the file", env=env)
        try:
            c.call("set_plan", milestones=[{"state": "the file is saved", "check": None}])
            c.call("check_done")
            self.assertEqual(srv.seen, [], "GUI_GATE_ROLE_VERIFIER_ENABLE=0 must switch the verifier off")
        finally:
            c.close(); srv.close()

    def test_role_calls_have_a_total_deadline(self):
        # every model call takes 3 s and asks for one more probe; 8 steps would be ~30 s, the deadline is 4 s
        srv = _SlowRole(delay=3.0)
        env = {"GUI_GATE_VERIFY_PLUS": "1", "GUI_GATE_ROLE_VERIFIER_URL": srv.url, "GUI_GATE_ROLE_VERIFIER_MODEL": "m",
               "GUI_GATE_ROLE_VERIFIER_KEY": "k", "GUI_GATE_ROLE_DEADLINE_S": "4"}
        c = Client("--human", "deny", "--task", "save the file", env=env)
        try:
            c.call("set_plan", milestones=[{"state": "the file is saved", "check": None}])
            t0 = time.time()
            r = c.call("check_done")
            el = time.time() - t0
            self.assertLess(el, 9, f"check_done took {el:.1f} s")
            v = r["structuredContent"].get("verifier") or {}
            self.assertEqual(v.get("status"), "timeout", r["structuredContent"])
        finally:
            c.close(); srv.close()

    def test_verifier_only_with_explicit_postconditions(self):
        srv = _SlowRole(reply={"final": {"decision": "done", "reasoning": "x"}})
        env = {"GUI_GATE_VERIFY_PLUS": "1", "GUI_GATE_ROLE_VERIFIER_URL": srv.url, "GUI_GATE_ROLE_VERIFIER_MODEL": "m",
               "GUI_GATE_ROLE_VERIFIER_KEY": "k"}
        c = Client("--human", "deny", "--task", "save the file", env=env)
        try:
            r = c.call("check_done")  # no plan, no postcondition: nothing for a verifier to verify
            self.assertEqual(srv.seen, [])
            self.assertEqual((r["structuredContent"].get("verifier") or {}).get("status"), "no_postconditions")
        finally:
            c.close(); srv.close()


# ---------------------------------------------------------------------------------------------------- 9


class StructuredStatus(unittest.TestCase):
    def test_status_values_and_fields(self):
        c = Modern()
        try:
            s = c.read()
            r = c.call("click", element_token=f"{s}:0")
            sc = r["structuredContent"]
            self.assertEqual(sc["status"], "done", sc)
            self.assertIs(sc["executed"], True)
            self.assertTrue(sc.get("continuation_id"))
            self.assertIn(sc["postcondition"]["status"], ("verified", "unverifiable", "none"))
            s = c.read()
            r = c.call("run_actions", pid=4242, window_id=777,
                       steps=[{"tool": "click", "args": {"element_token": f"{s}:0"}},
                              {"tool": "click", "args": {"target": {"label": "No such thing"}}}])
            sc = r["structuredContent"]
            if sc["status"] == "partial":
                self.assertEqual(sc["completed_steps"], [0])
                self.assertEqual(sc["next_step"], 1)
        finally:
            c.close()
        c = Modern(elicitation=False)  # nobody can approve "Delete file": refused, nothing sent
        try:
            s = c.read()
            r = c.call("click", element_token=f"{s}:1")
            self.assertEqual(r["structuredContent"]["status"], "blocked")
            self.assertIs(r["structuredContent"]["executed"], False)
            self.assertIn(r["structuredContent"]["loop_state"], ("normal", "switch_strategy", "blocked_repeat", "help_or_stop"))
        finally:
            c.close()


if __name__ == "__main__":
    unittest.main()
