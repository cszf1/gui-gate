"""bench/human_pilot.py from-logs: the desktop client's logs/human.jsonl (gui-pi-human/1) and the gate's
person_burden events become pilot sessions the analysis reads."""
import json
import os
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "bench"))
import human_pilot  # noqa: E402


def w(path, rows):
    with open(path, "w", encoding="utf-8") as fh:
        for r in rows:
            fh.write((r if isinstance(r, str) else json.dumps(r)) + "\n")


def hq(task, ty, outcome, secs, repeated=False, steps=0, refused=False):
    # the exact line desktop/core/src/human.rs writes per question
    return {"ev": "human_q", "schema": "gui-pi-human/1", "task": task, "type": ty, "outcome": outcome,
            "refused": refused, "secs": secs, "repeated": repeated, "takeover_steps": steps}


def hs(task, **kw):
    base = {"ev": "human_summary", "schema": "gui-pi-human/1", "task": task, "end": "done", "asks": 0,
            "by_type": {"approve": 0, "clarify": 0, "takeover": 0, "help": 0}, "repeated": 0, "repeated_ratio": 0.0,
            "person_active_s": 0.0, "person_wait_s": 0.0, "agent_blocked_s": 0.0, "takeover_steps": 0,
            "cancelled": 0, "timed_out": 0, "declined": 0, "unavailable": 0, "task_s": 0.0}
    base.update(kw)
    return base


def pb(kind, outcome, wait_s, repeat=False, totals=None):
    # the gate's person_burden event (src/server.rs note_burden)
    label = "no_answer" if outcome in ("timeout", "cancelled", "declined", "window_closed", "no_response", "channel_unavailable") \
        else ("answered" if outcome in ("accepted", "denied") else "other")
    return {"ev": "person_burden", "kind": kind, "outcome": outcome, "label": label, "repeat": repeat, "wait_s": wait_s,
            "active_s": wait_s if label == "answered" else 0.0, "task_version": 1, "totals": totals or {}}


class FromLogsTests(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp()
        self.human = os.path.join(self.d, "human.jsonl")
        w(self.human, [
            hq("task-1", "approve", "allowed", 6.0),
            hq("task-1", "clarify", "answered", 4.5),
            hq("task-1", "clarify", "answered", 3.0, repeated=True),
            hq("task-1", "takeover", "done by the person", 30.0, steps=3),
            hs("task-1", asks=4, repeated=1, person_active_s=43.5, person_wait_s=120.0, agent_blocked_s=43.5,
               takeover_steps=3, task_s=300.0),
            hq("task-2", "approve", "timed out", 120.0, refused=True),
            hq("task-2", "help", "try another way", 8.0),
            hs("task-2", asks=2, person_active_s=8.0, person_wait_s=60.0, agent_blocked_s=128.0, timed_out=1, task_s=400.0),
            hq("task-3", "approve", "window closed", 2.0, refused=True),  # crashed: no summary
            '{"ev": "human_q", "schema": "gui-pi-hu',  # torn last line
        ])
        self.gate3 = os.path.join(self.d, "gate3.jsonl")
        w(self.gate3, [{"ev": "session", "policy": "gate"},
                       pb("approve", "window_closed", 2.0, totals={"asks": 1, "not_asked_again": 0})])
        self.gate4 = os.path.join(self.d, "gate4.jsonl")
        w(self.gate4, [pb("approve", "accepted", 5.0, totals={"asks": 1}),
                       pb("ask_help", "accepted", 20.0, totals={"asks": 2, "person_active_s": 25.0, "person_wait_s": 25.0,
                                                                "takeover_steps": 1, "not_asked_again": 1}),
                       pb("answer", "timeout", 120.0, repeat=True,
                          totals={"asks": 3, "person_active_s": 25.0, "person_wait_s": 145.0, "takeover_steps": 1, "not_asked_again": 1})])
        self.meta = [
            {"log_task": "task-1", "participant": "p01", "task": "ow01", "condition": "value_ask", "order": 0, "success": True, "interruption_rating": 2},
            {"log_task": "task-2", "participant": "p01", "task": "ow02", "condition": "threshold_ask", "order": 1, "success": False},
            {"log_task": "task-3", "participant": "p02", "task": "ow01", "condition": "threshold_ask", "order": 0, "success": True, "gate_log": self.gate3},
            {"log_task": "task-4", "participant": "p02", "task": "ow02", "condition": "value_ask", "order": 1, "success": True, "total_s": 250.0},
        ]

    def sessions(self):
        return {s["log_task"]: s for s in human_pilot.from_logs(self.meta, [self.human], {"task-4": self.gate4})}

    def test_desktop_log_maps_to_the_session_shape(self):
        s = self.sessions()["task-1"]
        self.assertEqual(s["source"], "gui-pi-human/1")
        self.assertEqual([a["kind"] for a in s["asks"]], ["approve", "answer", "answer", "takeover"])
        self.assertEqual([a["outcome"] for a in s["asks"]], ["approved", "answered", "answered", "taken_over"])
        self.assertEqual((s["active_s"], s["waiting_s"], s["takeover_steps"], s["total_s"]), (43.5, 120.0, 3, 300.0))
        self.assertFalse(s["cancelled"] or s["timed_out"])
        m = human_pilot.session_metrics(s)
        self.assertEqual((m["asks"], m["repeated"]), (4, 1))  # the client's repeated flag, not question text
        self.assertNotIn("warning", s)

    def test_timeouts_and_hints_are_not_refusals_to_help(self):
        s = self.sessions()["task-2"]
        self.assertEqual([a["outcome"] for a in s["asks"]], ["timeout", "hint"])
        self.assertTrue(s["timed_out"])
        self.assertEqual(human_pilot.session_metrics(s)["timeout_reasons"], ["unrecorded"])

    def test_crashed_task_without_summary_and_gate_cross_check(self):
        s = self.sessions()["task-3"]
        self.assertIsNone(s["waiting_s"])
        self.assertIn("warning", s)
        self.assertTrue(s["cancelled"])
        self.assertEqual(s["gate_check"], {"asks": 1, "not_asked_again": 0, "agrees": True})

    def test_gate_only_session_from_person_burden_events(self):
        s = self.sessions()["task-4"]
        self.assertEqual(s["source"], "gate person_burden")
        self.assertEqual([a["kind"] for a in s["asks"]], ["approve", "help", "answer"])
        self.assertEqual([a["outcome"] for a in s["asks"]], ["approved", "helped", "timeout"])
        self.assertIsNone(s["waiting_s"])  # the gate cannot see watching time between questions
        self.assertEqual((s["active_s"], s["agent_blocked_s"], s["takeover_steps"], s["not_asked_again"]), (25.0, 145.0, 1, 1))
        self.assertEqual(s["total_s"], 250.0)
        self.assertEqual(human_pilot.session_metrics(s)["repeated"], 1)

    def test_cli_output_feeds_the_analysis(self):
        mp = os.path.join(self.d, "meta.jsonl")
        w(mp, self.meta)
        out = os.path.join(self.d, "sessions.jsonl")
        human_pilot.main(["from-logs", "--meta", mp, "--human", self.human, "--gate-log", f"task-4={self.gate4}", "--out", out])
        rows = [json.loads(x) for x in open(out)]
        self.assertEqual(len(rows), 4)
        res = human_pilot.main([out, "--md", os.path.join(self.d, "h.md")])
        pc = res["conditions"]
        # waiting time averages only the sessions that know it (one per condition here)
        self.assertEqual(pc["value_ask"]["waiting_s_sessions"], 1)
        self.assertEqual(pc["value_ask"]["waiting_s"], 120.0)
        self.assertEqual(pc["threshold_ask"]["waiting_s"], 60.0)
        self.assertEqual(res["paired"]["unit"], "participant")  # different tasks per person: participant means


def hq2(task, ty, outcome, wait_s, active_s=None, evidence="none", repeated=False, steps=0, refused=False):
    # the gui-pi-human/2 line desktop 0.3.2 writes (desktop/core/src/human.rs): secs kept = wait_s for old readers
    return {"ev": "human_q", "schema": "gui-pi-human/2", "task": task, "type": ty, "outcome": outcome, "refused": refused,
            "wait_s": wait_s, "secs": wait_s, "active_s": active_s, "evidence": evidence, "repeated": repeated,
            "takeover_steps": steps}


class HumanV2Tests(unittest.TestCase):
    """gui-pi-human/2: wait_s (popup open) is never operating time; active_s only with input evidence, null = missing."""

    def setUp(self):
        self.d = tempfile.mkdtemp()
        self.human = os.path.join(self.d, "human.jsonl")
        w(self.human, [
            hq("old-1", "approve", "allowed", 6.0),  # a /1 line in the same file still reads
            hs("old-1", asks=1, person_active_s=6.0, person_wait_s=50.0, agent_blocked_s=6.0, task_s=90.0),
            hq2("t-1", "approve", "allowed", 9.0, active_s=1.2, evidence="input"),
            hq2("t-1", "clarify", "answered", 5.0, evidence="answer_only"),
            hq2("t-1", "takeover", "done by the person", 40.0, evidence="outside_popup", steps=2),
            {"ev": "human_summary", "schema": "gui-pi-human/2", "task": "t-1", "end": "done", "asks": 3,
             "by_type": {"approve": 1, "clarify": 1, "takeover": 1, "help": 0}, "repeated": 0, "repeated_ratio": 0.0,
             "person_active_s": 1.2, "person_active_missing": 2, "person_active_upper_s": 45.0, "question_wait_s": 54.0,
             "no_question_s": 146.0, "agent_blocked_s": 54.0, "takeover_steps": 2, "cancelled": 0, "timed_out": 0,
             "declined": 0, "unavailable": 0, "task_s": 200.0},
            hq2("t-2", "approve", "allowed", 7.0, active_s=2.0, evidence="input"),  # crashed: no summary
            hq2("t-2", "help", "timed out", 120.0, refused=True),
        ])
        self.meta = [
            {"log_task": "old-1", "participant": "p01", "task": "a", "condition": "value_ask", "order": 0, "success": True},
            {"log_task": "t-1", "participant": "p01", "task": "b", "condition": "value_ask", "order": 1, "success": True},
            {"log_task": "t-2", "participant": "p02", "task": "b", "condition": "threshold_ask", "order": 0, "success": False},
        ]

    def sessions(self):
        return {s["log_task"]: s for s in human_pilot.from_logs(self.meta, [self.human])}

    def test_v2_summary_keeps_wait_and_evidenced_active_apart(self):
        s = self.sessions()["t-1"]
        self.assertEqual(s["source"], "gui-pi-human/2")
        self.assertEqual(s["active_s"], 1.2)  # never the 54 s the popups were open
        self.assertEqual((s["active_missing"], s["active_upper_s"], s["question_wait_s"]), (2, 45.0, 54.0))
        self.assertEqual(s["waiting_s"], 146.0)  # no_question_s (/1 called it person_wait_s)
        self.assertEqual([a["wait_s"] for a in s["asks"]], [9.0, 5.0, 40.0])
        self.assertEqual([a["active_s"] for a in s["asks"]], [1.2, None, None])
        self.assertEqual([a["secs"] for a in s["asks"]], [9.0, 5.0, 40.0])  # secs = wait_s
        self.assertEqual([a["outcome"] for a in s["asks"]], ["approved", "answered", "taken_over"])
        self.assertNotIn("warning", s)

    def test_v1_lines_still_read_with_secs_as_wait(self):
        s = self.sessions()["old-1"]
        self.assertEqual(s["source"], "gui-pi-human/1")
        self.assertEqual((s["active_s"], s["waiting_s"]), (6.0, 50.0))
        self.assertEqual(s["asks"][0]["wait_s"], 6.0)
        self.assertIsNone(s["asks"][0]["active_s"])
        self.assertNotIn("active_missing", s)

    def test_v2_crashed_task_uses_only_evidenced_active_time(self):
        s = self.sessions()["t-2"]
        self.assertIsNone(s["waiting_s"])
        self.assertEqual((s["active_s"], s["active_missing"], s["active_upper_s"], s["question_wait_s"]), (2.0, 1, 120.0, 127.0))
        self.assertTrue(s["timed_out"])

    def test_gate_0103_totals_bound_only_the_missing_questions(self):
        g = os.path.join(self.d, "gate.jsonl")
        w(g, [{"ev": "person_burden", "kind": "approve", "outcome": "accepted", "label": "answered", "repeat": False,
               "wait_s": 8.0, "active_s": None, "totals": {"asks": 1, "person_active_s": 0.0, "active_measured": 0,
                                                            "active_missing": 1, "person_active_upper_s": 8.0, "person_wait_s": 8.0}}])
        meta = [{"log_task": "g-1", "participant": "p03", "task": "c", "condition": "value_ask", "order": 0, "success": True}]
        s = human_pilot.from_logs(meta, [], {"g-1": g})[0]
        self.assertEqual(s["source"], "gate person_burden")
        self.assertIsNone(s["asks"][0]["active_s"])
        self.assertEqual((s["active_s"], s["active_missing"], s["active_upper_s"]), (0.0, 1, 8.0))

    def test_analysis_reports_missing_active_time(self):
        p = os.path.join(self.d, "s.jsonl")
        w(p, human_pilot.from_logs(self.meta, [self.human]))
        res = human_pilot.main([p, "--md", os.path.join(self.d, "h.md")])
        v = res["conditions"]["value_ask"]
        self.assertEqual(v["active_missing"], 2)
        self.assertEqual(v["active_s"], (6.0 + 1.2) / 2)


if __name__ == "__main__":
    unittest.main()
