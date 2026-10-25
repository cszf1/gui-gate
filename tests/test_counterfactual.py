"""bench/counterfactual/: checkpoint restore, the seven treatments, analysis, and an offline dry run through the gate."""
import json
import os
os.environ.setdefault("GUI_GATE_DIALOG", "none")  # tests answer person requests through the client (MRTR) or --human, never a real desktop dialog
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "bench", "counterfactual"))
sys.path.insert(0, os.path.join(ROOT, "bench"))
import analyze  # noqa: E402
import dryrun  # noqa: E402
import harness  # noqa: E402
import osworld  # noqa: E402
from fixture_env import FixtureEnv  # noqa: E402

BIN = os.environ.get("GUI_GATE_BIN") or next(
    (p for p in (os.path.join(ROOT, "target", "release", "gui-gate"), os.path.join(ROOT, "target", "debug", "gui-gate")) if os.path.exists(p)), None)


class FixtureTwinTests(unittest.TestCase):
    def test_dump_load_is_exact_including_the_rng(self):
        e = FixtureEnv(seed=4, length=6)
        for i in range(5):
            e.advance(1.0)
            e.click(i % 8)
        d = e.dump()
        e2 = FixtureEnv.load(json.loads(json.dumps(d)))
        self.assertEqual(e.state_hash(), e2.state_hash())
        # the same future after restore: same hazards from the same RNG state
        a = [e.click(j % 8) for j in range(6)]
        b = [e2.click(j % 8) for j in range(6)]
        self.assertEqual(a, b)
        self.assertEqual(e.state_hash(), e2.state_hash())

    def test_hazards_match_the_gtk_app(self):
        e = FixtureEnv(seed=0, length=3, p_relabel=0, p_loading=1, p_popup=0)
        want = e.st["seq"][0]
        ev = e.click(e.st["labels"].index(want))
        self.assertTrue(ev["correct"])
        self.assertTrue(e.busy)
        self.assertEqual(e.click(0)["ev"], "ignored")
        self.assertEqual(e.observe()["labels"], ["Loading..."] * 8)
        e.advance(1.0)
        self.assertFalse(e.busy)
        p = FixtureEnv(seed=0, length=3, p_relabel=0, p_loading=0, p_popup=1)
        p.click(p.st["labels"].index(p.st["seq"][0]))
        self.assertTrue(p.observe(popup_visible=True)["popup"])
        self.assertFalse(p.observe(popup_visible=False)["popup"])  # a main-window tree read does not see the dialog
        self.assertTrue(p.click("ok")["useful"])


class HarnessTests(unittest.TestCase):
    def cps(self, mk=harness.sim_backend, fams=("mixed", "popup", "relabel", "loading"), seeds=2):
        return [harness.make_checkpoint(mk, s, f, 8, n, 160.0) for f in fams for s in range(seeds) for n in (2, 4)]

    def test_restore_is_verified_by_hash_snapshot_and_replay(self):
        cp = self.cps(fams=("mixed",), seeds=1)[0]
        for mode in ("snapshot", "replay"):
            be, ctl, ok, h = harness.restore(cp, harness.sim_backend, mode)
            self.assertTrue(ok, mode)
            self.assertEqual(h, cp["hash"])
        bad = dict(cp, prefix=cp["prefix"] + [("advance", 0.5)])  # a replay that drifts
        self.assertFalse(harness.restore(bad, harness.sim_backend, "replay")[2])

    def test_every_treatment_branches_from_the_same_checkpoint(self):
        cps = self.cps(seeds=1)
        recs = harness.run_branches(cps, harness.sim_backend, repeats=2, seed=0)
        live = [c for c in cps if not c["done"]]
        self.assertEqual(len(recs), len(live) * 7 * 2)
        self.assertTrue(all(r["restored_ok"] for r in recs))
        by = {}
        for r in recs:
            by.setdefault(r["checkpoint"], set()).add(r["treatment"])
        self.assertTrue(all(v == set(harness.TREATMENTS) for v in by.values()))
        # orders are shuffled, not always the treatment list order
        firsts = {r["treatment"] for r in recs if r["order"] == 0}
        self.assertGreater(len(firsts), 1)
        cost = {t: [r["cost_s"] for r in recs if r["treatment"] == t] for t in harness.TREATMENTS}
        self.assertEqual(set(cost["matched_wait"]), set(cost["look_light"]))  # matched wall time, no information
        self.assertTrue(all(r["person_s"] > 0 for r in recs if r["treatment"] == "ask"))
        self.assertTrue(all(r["tokens"] == 0 for r in recs if r["treatment"] in ("direct", "matched_wait")))
        for k in ("step_success", "task_success", "executed", "state_hash", "continuation_s"):
            self.assertIn(k, recs[0])

    def test_analysis_keeps_negative_zero_and_positive_increments(self):
        recs = harness.run_branches(self.cps(seeds=3), harness.sim_backend, repeats=2, seed=0)
        res = analyze.analyze(recs)
        t = res["treatments"]
        self.assertEqual(set(t), set(harness.TREATMENTS) - {"direct"})
        self.assertGreater(t["extra_reason"]["negative"], 0)  # thinking again without information can hurt
        self.assertLess(t["extra_reason"]["delta"], 0)
        self.assertGreater(t["look_full"]["delta"], 0)
        self.assertGreater(t["matched_wait"]["zero"], 0)
        lo, hi = t["look_full"]["ci95_family_bootstrap"]
        self.assertLessEqual(lo, t["look_full"]["delta"])
        self.assertGreaterEqual(hi, t["look_full"]["delta"])
        self.assertIn("look_light_minus_matched_wait", res)
        # unfaithful restores are excluded and counted
        recs[0]["restored_ok"] = False
        self.assertEqual(analyze.analyze(recs)["unfaithful_restores"], 1)
        # reliability against predictions
        preds = {(r["checkpoint"], "look_full"): 0.5 for r in recs}
        rel = analyze.analyze(recs, preds)["incremental_reliability"]["look_full"]
        self.assertTrue(rel and abs(rel[0]["mean_pred"] - 0.5) < 1e-9)

    def test_dryrun_writes_a_family_split(self):
        d = tempfile.mkdtemp()
        cps, recs, res = dryrun.run(d, seeds=1, prefixes=(2,), repeats=1)
        split = json.load(open(os.path.join(d, "split.json")))
        self.assertFalse(set(split["test_families"]) & set(split["train_families"]))
        self.assertTrue(os.path.exists(os.path.join(d, "analysis.md")))
        self.assertIn("NOT model data", open(os.path.join(d, "analysis.md")).read())

    def test_osworld_prefix_from_a_pi_stream(self):
        f = tempfile.NamedTemporaryFile("w", suffix=".jsonl", delete=False)
        for i in range(3):
            f.write(json.dumps({"type": "message_end", "message": {"role": "assistant", "content": [
                {"type": "toolCall", "name": "mcp__gui__click", "arguments": {"element_token": f"s1:{i}"}}]}}) + "\n")
        f.write("not json\n")
        f.close()
        self.assertEqual(osworld.prefix_from_pi_stream(f.name, 2), [("click", {"element_token": "s1:0"}), ("click", {"element_token": "s1:1"})])
        fp1, _ = osworld.fingerprint(["/x"], run=lambda c: "same")
        fp2, _ = osworld.fingerprint(["/x"], run=lambda c: "other")
        self.assertNotEqual(fp1, fp2)


@unittest.skipUnless(BIN, "build the gate first: cargo build --release")
class GateDryRunTests(unittest.TestCase):
    def test_branches_through_the_real_gate_offline(self):
        mk = harness.gate_backend(BIN)
        cps = [harness.make_checkpoint(mk, 1, "popup", 6, 2, 120.0)]
        for mode in ("snapshot", "replay"):
            recs = harness.run_branches(cps, mk, treatments=["direct", "look_full", "matched_wait"], repeats=1, restore_mode=mode)
            self.assertEqual(len(recs), 3)
            self.assertTrue(all(r["restored_ok"] for r in recs), [r["state_hash"] for r in recs])
            self.assertTrue(all(r["backend"] == "gate" for r in recs))
            self.assertEqual(recs[0]["gate_history_restored"], mode == "replay")
        # the gate adds no value intervention of its own (GUI_GATE_VALUE_POLICY=direct): whatever it held back came
        # from its hard rules (engineering base, the same for every treatment)
        be = harness.GateBackend(BIN, seed=1, length=6, family="popup")
        be.observe(True)
        for i in range(4):
            be.click(i)
        be.close()
        acts = [json.loads(x) for x in open(be.log) if '"ev": "action"' in x or '"ev":"action"' in x]
        self.assertTrue(acts)
        for e in acts:
            if e.get("value"):
                self.assertEqual(e["value"]["policy"], "direct")
            if not e.get("executed"):
                self.assertIn(e["tier"], ("rule", "policy"))


if __name__ == "__main__":
    unittest.main()
