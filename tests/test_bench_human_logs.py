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


if __name__ == "__main__":
    unittest.main()
