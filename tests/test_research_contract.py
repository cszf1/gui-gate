"""v0.10.3 decision data contract (src/research.rs) end to end against tests/fake_driver.py: every action event keeps
an immutable x_pre next to x_post, a decision id, the processing chain with timings, the three label layers kept
apart, and a continuation link from a hand-back to the next action. gate_next_step uses its own freshness rule.
No model, no network."""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from test_gate import Client, txt  # noqa: E402
from test_remaining import client_call  # noqa: E402


def actions(c):
    return [e for e in c.events() if e.get("ev") == "action"]


class DecisionContract(unittest.TestCase):
    def test_direct_step_has_pre_post_ids_and_label_layers(self):
        c = Client()
        try:
            s = c.read()
            r = c.call("click", element_token=f"{s}:0")
            self.assertFalse(r.get("isError"), txt(r))
            ev = actions(c)[-1]
            self.assertEqual(ev["contract"], "decision_v1")
            self.assertTrue(ev["decision_id"].startswith("d-"))
            self.assertEqual(ev["x_post"], ev["features"], "x_post is the legacy features key")
            self.assertEqual(ev["x_pre"], ev["features"], "nothing ran before the step: pre == post")
            self.assertEqual(ev["proposed"], "direct")
            self.assertFalse(ev["waited"])
            self.assertEqual(ev["labels"]["exec"], "delivered")
            self.assertIn(ev["labels"]["local"], ("met", "not_met", "unknown"))
            self.assertIsNone(ev["labels"]["task"], "the gate never writes a task label onto a step")
            self.assertTrue(ev["candidate"]["effect_id"].startswith("e"))
            self.assertEqual(ev["escalation"]["stage"], "continue")
            self.assertIn("exec_t0_ms", ev)
        finally:
            c.close()

    def test_heal_keeps_the_pre_decision_state(self):
        c = Client()
        try:
            s1 = c.read()
            c.read()  # s1 tokens are stale now
            r = c.call("click", element_token=f"{s1}:0")
            self.assertIn("re-bound", txt(r))
            ev = actions(c)[-1]
            self.assertEqual(ev["x_pre"]["token_stale"], 1.0, "x_pre is the state BEFORE the heal")
            self.assertEqual(ev["x_post"]["token_stale"], 0.0)
            heal = [x for x in ev["chain"] if x["option"] == "heal"]
            self.assertEqual(len(heal), 1)
            self.assertTrue(heal[0]["rebound"])
            self.assertLessEqual(heal[0]["t0_ms"], heal[0]["t1_ms"])
        finally:
            c.close()

    def test_handback_is_linked_to_its_continuation(self):
        c = Client("--no-auto-observe")  # keep the read's tokens current so the repeat is the identical action
        try:
            s = c.read()
            c.call("fake_mutate", fail_next=1)
            r1 = c.call("click", element_token=f"{s}:0")
            self.assertTrue(r1.get("isError"))
            r2 = c.call("click", element_token=f"{s}:0")  # the identical action just failed: handed back (look_full)
            self.assertIn("[gate]", txt(r2))
            hb = actions(c)[-1]
            self.assertFalse(hb.get("executed"))
            s2 = c.read()
            c.call("click", element_token=f"{s2}:0")
            cont = actions(c)[-1]["continues"]
            self.assertEqual(cont["decision_id"], hb["decision_id"])
            self.assertTrue(cont["same_candidate"])
            self.assertTrue(cont["attributable"])
            self.assertFalse(cont["model_switched"])
            # an executed step closes the hand-back: the next action continues nothing
            c.call("click", element_token=f"{s2}:3")
            self.assertIsNone(actions(c)[-1]["continues"])
        finally:
            c.close()

    def test_a_switched_action_is_linked_but_not_attributable(self):
        c = Client()
        try:
            r = c.call("click", element_token="s000000ff:0")  # no observation of that window: handed back
            self.assertIn("[gate]", txt(r))
            hb = actions(c)[-1]
            s = c.read()
            c.call("press_key", pid=4242, window_id=777, key="Tab")
            cont = actions(c)[-1]["continues"]
            self.assertEqual(cont["decision_id"], hb["decision_id"])
            self.assertTrue(cont["model_switched"])
            self.assertFalse(cont["attributable"])
            del s
        finally:
            c.close()

    def test_next_step_uses_freshness_not_a_fictitious_click(self):
        c = Client()
        try:
            s = c.read()
            c.call("click", element_token=f"{s}:0")  # auto-observe attaches a fresh tree to this result
            sc = client_call(c, "gate_next_step", window_id="777")["structuredContent"]
            self.assertEqual(sc["rule"], "freshness")
            self.assertEqual(sc["observe"], "none", sc)
            self.assertIn("escalation", sc)
            ev = [e for e in c.events() if e.get("ev") == "next_step"][-1]
            self.assertEqual(ev["model"], "next_step_rule_v1")
            self.assertNotIn("p_direct", ev)
        finally:
            c.close()


if __name__ == "__main__":
    unittest.main()
