"""Every gate return action, end to end, over a 2026-07-28 client (per-request `_meta`, MRTR person requests).

The real gate binary, tests/fake_driver.py as the driver, a local fake reviewer endpoint where a test needs one.
A person is simulated by the client: it answers each `input_required` elicitation from a script, then retries the
same tools/call with `inputResponses` + `requestState` (MCP 2026-07-28 Multi Round-Trip Requests).

Run:  python3 -m unittest tests.test_actions_e2e -v   (after cargo build)
"""
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from test_gate import BIN, FAKE, FAKE_REVIEWER, ROOT, txt  # noqa: E402
from test_v04 import _Rev  # noqa: E402

PV, CC, CI = "io.modelcontextprotocol/protocolVersion", "io.modelcontextprotocol/clientCapabilities", "io.modelcontextprotocol/clientInfo"
GG = "io.github.cszf1/gui-gate"


class Person:
    """Scripted person: answers[kind] is an ElicitResult (dict) or a callable(params) -> ElicitResult, or "silent"."""
    def __init__(self, **answers):
        self.answers = answers
        self.asked = []

    def __call__(self, params):
        kind = params["_meta"][GG]["kind"]
        self.asked.append(params)
        a = self.answers.get(kind)
        if callable(a):
            a = a(params)
        return a


ALLOW = {"action": "accept", "content": {"decision": "allow"}}
DENY = {"action": "accept", "content": {"decision": "deny"}}
DECLINE = {"action": "decline"}
DONE = {"action": "accept", "content": {"done": True}}


class Modern:
    """A 2026-07-28 MCP client: no initialize, `_meta` on every request, MRTR retries."""
    def __init__(self, *extra, env=None, person=None, elicitation=True, reviewer_url=None):
        if not BIN:
            raise unittest.SkipTest("build the gate first: cargo build")
        self.log = tempfile.NamedTemporaryFile(suffix=".jsonl", delete=False).name
        e = dict(os.environ, PYTHONPATH=ROOT, GUI_GATE_NO_UPDATE_CHECK="1")
        e.update({k: v for k, v in FAKE_REVIEWER.items() if not os.environ.get(k)})
        if reviewer_url:
            e["GUI_GATE_REVIEWER_URL"] = reviewer_url
        e.update(env or {})
        self.p = subprocess.Popen([BIN, "--child", FAKE, "--log", self.log, *extra], stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, text=True, env=e, cwd=ROOT)
        self.i = 0
        self.person = person or Person()
        self.caps = {"elicitation": {"form": {}}} if elicitation else {}
        self.rounds = []  # every input_required result seen

    def raw(self, method, params):
        self.i += 1
        params = dict(params)
        params["_meta"] = {PV: "2026-07-28", CC: self.caps, CI: {"name": "e2e", "version": "0"}}
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.i, "method": method, "params": params}) + "\n")
        self.p.stdin.flush()
        while True:
            m = json.loads(self.p.stdout.readline())
            if m.get("id") == self.i:
                return m

    def call(self, name, **args):
        params = {"name": name, "arguments": args}
        for _ in range(4):
            m = self.raw("tools/call", params)
            if "error" in m:
                return m
            r = m["result"]
            if r.get("resultType") != "input_required":
                return r
            self.rounds.append(r)
            resp = {}
            for k, req in r.get("inputRequests", {}).items():
                assert req["method"] == "elicitation/create", req
                ans = self.person(req["params"])
                if ans == "silent":
                    time.sleep(float(req["params"]["_meta"][GG]["timeout_s"]) + 6)  # past deadline + grace
                    ans = ALLOW  # an approval that comes too late must not count
                resp[k] = ans
            params = {"name": name, "arguments": args, "inputResponses": resp, "requestState": r["requestState"]}
        raise AssertionError("too many MRTR rounds")

    def read(self, **a):
        return self.call("get_window_state", pid=4242, window_id=777, **a)["structuredContent"]["snapshot_id"]

    def events(self):
        with open(self.log) as fh:
            return [json.loads(l) for l in fh]

    def actions(self):
        return [e for e in self.events() if e.get("ev") == "action"]

    def clicked(self):
        return json.loads(txt(self.call("fake_state")))["clicked"]

    def close(self):
        self.p.stdin.close()
        self.p.wait(timeout=10)


def gate(r):
    return (r.get("structuredContent") or {}).get("gate") or {}


class ValueGateActions(unittest.TestCase):
    """direct, look_light, look_full, check, wait, heal: the gate settles these without a person."""
    def setUp(self):
        self.c = Modern()

    def tearDown(self):
        self.c.close()

    def test_direct(self):
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:0")
        self.assertFalse(r.get("isError"), txt(r))
        self.assertEqual(r["resultType"], "complete")
        ev = self.c.actions()[-1]
        self.assertEqual((ev["decision"], ev["executed"]), ("direct", True))

    def test_look_light(self):
        s1 = self.c.read()
        self.c.read()
        r = self.c.call("click", element_token=f"{s1}:3")  # stale + "OK" twice: no unique re-bind
        self.assertTrue(r.get("isError"))
        self.assertEqual(gate(r)["decision"], "look_light")
        self.assertIn("fresh tree is attached", txt(r))
        self.assertNotIn("OK", self.c.clicked())

    def test_look_full(self):
        r = self.c.call("click", pid=4242, window_id=777, x=120, y=80)  # pixels, but no screenshot seen yet
        self.assertTrue(r.get("isError"))
        self.assertEqual(gate(r)["decision"], "look_full")
        self.assertTrue(any(c.get("type") == "image" for c in r["content"]))
        self.assertEqual(self.c.clicked(), [])

    def test_check_probes_in_the_grey_zone(self):
        # the same click failed once: before sending the step back to the model the gate probes the tree (check);
        # the target is still there, so the step goes on without a model turn
        self.c.close()
        self.c = Modern("--no-auto-observe")
        self.c.call("fake_mutate", fail_next=1)
        s = self.c.read()
        self.c.call("click", element_token=f"{s}:0")
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:0")
        ev = self.c.actions()[-1]
        self.assertIn("check", ev["trail"], ev)
        self.assertTrue(ev["check"])
        self.assertTrue(ev["executed"], txt(r))

    def test_wait_when_the_window_is_loading(self):
        self.c.call("fake_mutate", elements=[["button", "Increment"], ["text", "Loading, please wait"]])
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:0")
        ev = self.c.actions()[-1]
        self.assertIn("wait", ev["trail"], ev)
        self.assertIn(ev["decision"], ("direct", "look_light"))

    def test_heal(self):
        s1 = self.c.read()
        self.c.read()
        r = self.c.call("click", element_token=f"{s1}:0")
        self.assertFalse(r.get("isError"), txt(r))
        self.assertIn("re-bound", txt(r))
        self.assertIn("healed", self.c.actions()[-1]["trail"])


class ReviewerActions(unittest.TestCase):
    """review and risk_review: the gate's own reviewer decides; the acting model is never asked."""
    def doubtful(self, rev):
        # a click that already failed once is sent again: the gate's own reviewer looks at it
        c = Modern("--task", "increment the counter", reviewer_url=rev.url)
        c.call("fake_mutate", fail_next=1)
        s = c.read()
        c.call("click", element_token=f"{s}:2")
        s = c.read()
        return c, c.call("click", element_token=f"{s}:2")

    def test_review_allow_runs(self):
        rev = _Rev("allow")
        c, r = self.doubtful(rev)
        try:
            ev = c.actions()[-1]
            self.assertIn("reviewer_allow", ev["trail"], ev)
            self.assertFalse(r.get("isError"), txt(r))
            self.assertEqual(ev["tier"], "review")
            self.assertEqual(len(rev.seen), 1)
        finally:
            c.close(); rev.close()

    def test_review_deny_sends_back_with_fresh_tree(self):
        rev = _Rev("deny")
        c, r = self.doubtful(rev)
        try:
            self.assertEqual(gate(r)["decision"], "look_light", txt(r))
            self.assertIn("reviewer doubts", txt(r))
        finally:
            c.close(); rev.close()

    def test_risk_review_allow_and_deny(self):
        for decision, executed in (("allow", True), ("deny", False)):
            rev = _Rev(decision)
            c = Modern("--task", "draft a reply to Bob", reviewer_url=rev.url)
            try:
                c.call("fake_mutate", elements=[["button", "Send"], ["button", "Increment"], ["text", "x"]])
                s = c.read()
                r = c.call("click", element_token=f"{s}:0", gate_ack=True)  # self-ack is ignored
                self.assertEqual(not r.get("isError"), executed, txt(r))
                ev = c.actions()[-1]
                self.assertIn(f"reviewer_{decision}", ev["trail"], ev)
                if not executed:
                    self.assertEqual(gate(r)["decision"], "deny")
                    self.assertIn("independent reviewer", txt(r))
                self.assertEqual(c.person.asked, [])  # nobody was bothered
            finally:
                c.close(); rev.close()

    def test_no_reviewer_answer_goes_to_the_person(self):
        # medium risk, the (dummy) reviewer is unreachable: a person decides, never the agent
        p = Person(approve=ALLOW)
        c = Modern(person=p)
        try:
            c.call("fake_mutate", elements=[["button", "Send"], ["button", "Plum"]])
            s = c.read()
            r = c.call("click", element_token=f"{s}:0")
            self.assertFalse(r.get("isError"), txt(r))
            self.assertEqual(len(p.asked), 1)
            self.assertIn("Send", p.asked[0]["_meta"][GG]["target"])
        finally:
            c.close()


class PersonActions(unittest.TestCase):
    """ask_human, handoff (takeover), ask_help, request_human: MRTR, timeout = refusal."""
    def risky(self, person, *extra):
        c = Modern(*extra, person=person)
        s = c.read()
        r = c.call("click", element_token=f"{s}:1")  # "Delete file": high risk -> a person
        return c, r

    def test_ask_human_allow_executes(self):
        p = Person(approve=ALLOW)
        c, r = self.risky(p)
        try:
            self.assertFalse(r.get("isError"), txt(r))
            self.assertEqual(len(c.rounds), 1)
            req = c.rounds[0]["inputRequests"]["human"]["params"]
            self.assertEqual(req["mode"], "form")
            self.assertEqual(req["requestedSchema"]["properties"]["decision"]["enum"], ["allow", "deny"])
            self.assertEqual(req["_meta"][GG]["kind"], "approve")
            self.assertEqual(req["_meta"][GG]["decision"], "ask_human")
            self.assertIn("Delete file", c.clicked())
            ev = c.actions()[-1]
            self.assertEqual((ev["decision"], ev["tier"]), ("direct", "human"))
        finally:
            c.close()

    def test_ask_human_deny_refuses_and_remembers(self):
        p = Person(approve=DENY)
        c, r = self.risky(p)
        try:
            self.assertTrue(r.get("isError"))
            self.assertIn("the person declined", txt(r))
            self.assertNotIn("Delete file", c.clicked())
            # deny: same effect already refused -> refused at once, the person is not asked again
            s = c.read()
            r2 = c.call("click", element_token=f"{s}:1")
            self.assertEqual(gate(r2)["decision"], "deny")
            self.assertIn("same effect was already refused", txt(r2))
            self.assertEqual(len(p.asked), 1)
        finally:
            c.close()

    def test_declined_form_is_a_refusal(self):
        c, r = self.risky(Person(approve=DECLINE))
        try:
            self.assertTrue(r.get("isError"))
            self.assertNotIn("Delete file", c.clicked())
        finally:
            c.close()

    def test_late_answer_is_a_timeout(self):
        c, r = self.risky(Person(approve="silent"), "--human-timeout", "1")
        try:
            self.assertTrue(r.get("isError"))
            self.assertIn("did not answer within", txt(r))
            self.assertNotIn("Delete file", c.clicked())
            self.assertTrue(any(e.get("ev") == "human_answer" and e.get("late") for e in c.events()))
        finally:
            c.close()

    def test_request_state_is_one_time_and_bound(self):
        c = Modern(person=Person(approve=ALLOW))
        try:
            s = c.read()
            m = c.raw("tools/call", {"name": "click", "arguments": {"element_token": f"{s}:1"}})
            st = m["result"]["requestState"]
            ok = {"human": ALLOW}
            # bound to its call: another target cannot use it
            bad = c.raw("tools/call", {"name": "click", "arguments": {"element_token": f"{s}:0"}, "inputResponses": ok, "requestState": st})
            self.assertEqual(bad["error"]["code"], -32602)
            # consumed by the failed attempt: the right call cannot replay it either
            again = c.raw("tools/call", {"name": "click", "arguments": {"element_token": f"{s}:1"}, "inputResponses": ok, "requestState": st})
            self.assertIn("error", again)
            forged = c.raw("tools/call", {"name": "click", "arguments": {"element_token": f"{s}:1"}, "inputResponses": ok, "requestState": "gg-forged"})
            self.assertIn("error", forged)
            self.assertNotIn("Delete file", c.clicked())
            # the agent cannot smuggle an answer inside its arguments
            r = c.call("click", element_token=f"{s}:1", inputResponses=ok, requestState=st, gate_ack=True)
            self.assertEqual(len(c.rounds), 1)  # it still went to the person (who allowed it)
        finally:
            c.close()

    def test_no_elicitation_capability_never_executes(self):
        c = Modern(elicitation=False)
        try:
            s = c.read()
            r = c.call("click", element_token=f"{s}:1")
            self.assertTrue(r.get("isError"))
            self.assertEqual(c.rounds, [])
            self.assertNotIn("Delete file", c.clicked())
        finally:
            c.close()

    def test_handoff_takeover(self):
        for ans, done in ((DONE, True), ({"action": "accept", "content": {"done": False}}, False)):
            p = Person(takeover=ans)
            c = Modern(person=p)
            try:
                c.call("fake_mutate", elements=[["password text", "Password"], ["button", "Plum"]])
                s = c.read()
                r = c.call("type_text", element_token=f"{s}:0", text="hunter2")
                self.assertEqual(p.asked[0]["_meta"][GG]["kind"], "takeover")
                self.assertIn("done", p.asked[0]["requestedSchema"]["properties"])
                self.assertTrue(r.get("isError") if not done else True)
                if done:
                    self.assertIn("A person did this step themselves", txt(r))
                else:
                    self.assertIn("declined", txt(r))
                self.assertFalse(any(e.get("executed") for e in c.actions() if e.get("tool") == "type_text"))
            finally:
                c.close()

    def stuck(self, person):
        # the same click three times without a confirmed effect -> review; the dummy reviewer is down -> a person
        c = Modern("--no-auto-observe", person=person, env={"GUI_GATE_LOOP_BREAKER": "0"})
        c.read()
        rs = [c.call("click", pid=4242, window_id=777, element_index=0) for _ in range(3)]
        ev = c.actions()[-1]
        assert ev["decision"] == "ask_help", ev
        return c, rs[-1]

    def test_ask_help_person_did_it(self):
        p = Person(ask_help={"action": "accept", "content": {"choice": "did_it"}})
        c, r = self.stuck(p)
        try:
            self.assertEqual(p.asked[0]["_meta"][GG]["kind"], "ask_help")
            self.assertEqual(p.asked[0]["requestedSchema"]["properties"]["choice"]["enum"], ["did_it", "try_other"])
            self.assertIn("A person did this step themselves", txt(r))
            self.assertEqual(c.clicked().count("Increment"), 2)  # the third was the person's
        finally:
            c.close()

    def test_ask_help_try_another_way_carries_the_hint(self):
        p = Person(ask_help={"action": "accept", "content": {"choice": "try_other", "note": "use Ctrl+S"}})
        c, r = self.stuck(p)
        try:
            self.assertTrue(r.get("isError"))
            self.assertIn("No person took this step", txt(r))
            self.assertIn("use Ctrl+S", txt(r))
        finally:
            c.close()

    def test_ask_needing_step_inside_a_batch_is_sent_alone(self):
        p = Person(approve=ALLOW)
        c = Modern(person=p)
        try:
            s = c.read()
            r = c.call("run_actions", pid=4242, window_id=777,
                       steps=[{"tool": "click", "args": {"element_token": f"{s}:0"}},
                              {"tool": "click", "args": {"target": {"label": "Delete file"}}}])
            self.assertIn("ON ITS OWN", txt(r))
            self.assertEqual(c.rounds, [])
            self.assertEqual(c.clicked(), ["Increment"])  # the first step ran once, the risky one did not
        finally:
            c.close()

    def test_request_human_three_kinds(self):
        p = Person(answer={"action": "accept", "content": {"answer": "report-final.ods"}}, approve=DENY, takeover=DONE)
        c = Modern(person=p)
        try:
            r = c.call("request_human", question="Which file name?", kind="answer")
            self.assertIn("report-final.ods", txt(r))
            r = c.call("request_human", question="May I close the unsaved document?", kind="approve")
            self.assertIn("Not approved", txt(r))
            r = c.call("request_human", question="Please log in to the mail account.", kind="takeover")
            self.assertIn("step is done", txt(r))
            self.assertEqual([q["_meta"][GG]["kind"] for q in p.asked], ["answer", "approve", "takeover"])
        finally:
            c.close()


class DenyAndLoopActions(unittest.TestCase):
    def test_deny_without_reviewer_answer_unattended(self):
        c = Modern("--human", "deny")
        try:
            c.call("fake_mutate", elements=[["button", "Send"], ["button", "Plum"]])
            s = c.read()
            r = c.call("click", element_token=f"{s}:0")
            self.assertEqual(gate(r)["decision"], "deny")
            self.assertEqual(c.rounds, [])
        finally:
            c.close()

    def test_deny_escalates(self):
        c = Modern("--human", "deny")
        try:
            last = None
            for _ in range(3):
                s = c.read()
                last = c.call("click", element_token=f"{s}:1")
            self.assertIn("stop here and tell the user", txt(last))
        finally:
            c.close()

    def test_loop_note_then_stop_loop(self):
        c = Modern("--human", "deny")
        try:
            c.call("fake_mutate", fail_next=40)
            rs = []
            for i in range(14):
                s = c.read()
                rs.append(c.call("click", element_token=f"{s}:{0 if i % 2 == 0 else 3}"))
            alltxt = "\n".join(txt(r) for r in rs)
            self.assertIn("Loop breaker (level 1/3)", alltxt)
            self.assertEqual(gate(rs[-1])["decision"], "stop_loop")
        finally:
            c.close()


class LegacyClientStillWorks(unittest.TestCase):
    def test_legacy_elicitation_create(self):
        from test_gate import Client
        asked = []

        def on_req(m):
            asked.append(m)
            return {"action": "accept", "content": {"allow": True}}
        c = Client(capabilities={"elicitation": {}}, on_request=on_req)
        try:
            s = c.read()
            r = c.call("click", element_token=f"{s}:1")
            self.assertFalse(r.get("isError"), txt(r))
            self.assertEqual(asked[0]["method"], "elicitation/create")
            self.assertEqual(asked[0]["params"]["_meta"][GG]["kind"], "approve")
        finally:
            c.close()


if __name__ == "__main__":
    unittest.main()
