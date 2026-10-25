"""Cost accounting, held-out layouts + leakage lint, human pilot, context ablation, increment reliability plot
(offline: no desktop, no model, no network)."""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "bench"))
sys.path.insert(0, os.path.join(ROOT, "bench", "counterfactual"))
import analyze  # noqa: E402
import context_ablation  # noqa: E402
import cost_accounting  # noqa: E402
import dryrun  # noqa: E402
import harness  # noqa: E402
import human_pilot  # noqa: E402
import layouts  # noqa: E402
import leakage_lint  # noqa: E402
from fixture_env import FRUITS, FixtureEnv  # noqa: E402


def jl(path, rows):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        for r in rows:
            f.write(json.dumps(r) + "\n")


def pi_msg(role, content, usage=None, stop=None):
    m = {"role": role, "content": content}
    if usage:
        m["usage"] = usage
    if stop:
        m["stopReason"] = stop
    return {"type": "message_end", "message": m}


def call(name, **args):
    return {"type": "toolCall", "name": "mcp__gui__" + name, "arguments": args}


class CostAccountingTests(unittest.TestCase):
    def make_run(self, d, tag, turns, gui_exec, blocked=0, reviewer=0, roles=None, shots=0, wall=50.0, stop="agent"):
        pi = []
        for i in range(turns):
            pi.append(pi_msg("assistant", [call("click", element_token=f"s:{i}")],
                             usage={"input": 100, "cacheRead": 50, "output": 10, "cost": {"total": 0.001}}))
            pi.append(pi_msg("toolResult", [{"type": "text", "text": "ok"}] + [{"type": "image", "data": "x"}] * (1 if i < shots else 0)))
        jl(os.path.join(d, "pi", tag + ".jsonl"), pi)
        g = [{"ev": "action", "tool": "click", "executed": True, "judge": {"verdict": "allow"} if i < reviewer else None}
             for i in range(gui_exec)]
        g += [{"ev": "action", "tool": "click", "executed": False} for _ in range(blocked)]
        g += [{"ev": "action", "tool": "run_actions", "executed": True}]  # a batch wrapper is not a GUI action itself
        g += [{"ev": "command", "exit": 0}, {"ev": "obs", "auto": True, "screenshot": True}, {"ev": "img", "images": 2}]
        for role, n in (roles or {}).items():
            g.append({"ev": "role", "role": role, "calls": n})
        jl(os.path.join(d, "gate", tag + ".jsonl"), g)
        return {"tag": tag, "pi": {"wall_s": wall, "stop": stop}, "run_wall_s": wall + 20}

    def test_run_costs_separate_turns_actions_and_roles(self):
        d = tempfile.mkdtemp()
        row = self.make_run(d, "t1-gate8-s0", turns=10, gui_exec=4, blocked=2, reviewer=3, roles={"verifier": 2, "compact": 1}, shots=3)
        c = cost_accounting.run_costs(d, "t1-gate8-s0", row)
        self.assertEqual(c["pi_turns"], 10)
        self.assertEqual(c["pi_tool_calls"], 10)
        self.assertEqual(c["gui_actions"], 4)  # Pi made 10 calls, the desktop saw 4 actions
        self.assertEqual(c["gui_actions_blocked"], 2)
        self.assertEqual(c["reviewer_calls"], 3)
        self.assertEqual(c["role_calls"], {"verifier": 2, "compact": 1})
        self.assertEqual(c["role_calls_total"], 3)
        self.assertEqual(c["screenshots_model"], 3)
        self.assertEqual(c["screenshots_gate"], 3)
        self.assertEqual(c["tok_in"], 1500)
        self.assertEqual(c["tok_out"], 100)
        self.assertAlmostEqual(c["cost_usd"], 0.01)
        self.assertIsNone(c["role_tok_in"])  # the gate did not log role tokens: unknown, not 0
        self.assertEqual(c["run_wall_s"], 70.0)
        self.assertEqual(c["commands"], 1)

    def test_summary_times_over_all_runs_incl_failures_and_missing(self):
        d = tempfile.mkdtemp()
        rows = []
        for t in ("a", "b", "c"):
            rows.append(dict(self.make_run(d, f"{t}-A-s0", 3, 3, wall=100.0), task=t, cond="A", seed=0))
        rows.append(dict(self.make_run(d, "a-B-s0", 3, 3, wall=40.0), task="a", cond="B", seed=0))
        rows.append(dict(self.make_run(d, "b-B-s0", 3, 3, wall=480.0, stop="timeout"), task="b", cond="B", seed=0))
        # c-B missing -> imputed at the cap
        jl(os.path.join(d, "runs.jsonl"), rows)
        jl(os.path.join(d, "runs_scored.jsonl"), [{"task": r["task"], "cond": r["cond"], "seed": 0, "success": r["task"] == "a"} for r in rows])
        res = cost_accounting.main([d, "--baseline", "A", "--cap", "480", "--md", os.path.join(d, "c.md")])
        b = res["policies"]["B"]
        self.assertEqual(b["planned_runs"], 3)
        self.assertEqual(b["imputed_wall"], 1)
        self.assertAlmostEqual(b["mean_wall_s_all_runs"], (40 + 480 + 480) / 3)
        self.assertEqual(b["mean_wall_s_successes_only_DESCRIPTIVE"], 40.0)  # the survivor-biased view, labelled
        self.assertEqual(b["timeouts"], 1)
        pt = res["paired_time"]["B"]["wall_diff_bootstrap"]
        self.assertAlmostEqual(pt["diff"], ((40 - 100) + (480 - 100) + (480 - 100)) / 3)
        txt = open(os.path.join(d, "c.md")).read()
        self.assertIn("not environment actions", txt)
        self.assertIn("survivor-biased", txt)

    def test_score_rows_carry_the_accounting(self):
        d = tempfile.mkdtemp()
        r = self.make_run(d, "ow01-x-gate8-s0", 4, 2, reviewer=1)
        r.update(task="ow01-x", cond="gate8", seed=0, attempt=1, infra=None)
        jl(os.path.join(d, "runs.jsonl"), [r])
        os.makedirs(os.path.join(d, "eval"))
        json.dump({"ok": True, "score": 1.0, "success": True}, open(os.path.join(d, "eval", "ow01-x-gate8-s0.eval.json"), "w"))
        subprocess.run([sys.executable, os.path.join(ROOT, "bench", "score.py"), d], check=True, capture_output=True)
        row = json.loads(open(os.path.join(d, "runs_scored.jsonl")).readline())
        self.assertEqual(row["gui_actions"], 2)
        self.assertEqual(row["reviewer_calls"], 1)
        self.assertEqual(row["run_wall_s"], 70.0)
        self.assertNotIn("cost_accounting_error", row)


class LayoutTests(unittest.TestCase):
    def test_unseen_layouts_share_no_dev_caption_and_are_frozen(self):
        lays = layouts.unseen_layout_set()
        self.assertEqual(len(lays), 12)
        for lay in lays:
            self.assertEqual(lay["split"], "unseen_layout_test")
            self.assertFalse(set(lay["vocab"]) & set(FRUITS))
            self.assertEqual(sorted(lay["order"]), list(range(8)))
        doc = layouts.frozen(lays)
        self.assertTrue(layouts.check(doc))
        doc["layouts"][0]["vocab"][0] = "Apple"
        self.assertFalse(layouts.check(doc))
        self.assertEqual(layouts.unseen_layout_set(), lays)  # deterministic
        self.assertEqual(layouts.get(lays[3]["id"]), lays[3])

    def test_default_layout_state_is_unchanged_and_variants_restore_exactly(self):
        a = FixtureEnv(seed=5, length=6).dump()
        self.assertNotIn("layout", a)
        self.assertEqual(a["labels"], FRUITS)
        lay = layouts.unseen_layout_set()[0]
        e = FixtureEnv(seed=5, length=6, **dict(harness.FAMILIES[lay["hazards"]], layout=lay))
        self.assertEqual(e.st["layout"], lay["id"])
        self.assertTrue(set(e.st["seq"]) <= set(lay["vocab"]))
        e.click(0)
        e2 = FixtureEnv.load(e.dump())
        self.assertEqual(e2.state_hash(), e.state_hash())

    def test_harness_runs_unseen_layout_families(self):
        fam = layouts.families(layouts.unseen_layout_set(2))
        cps = [harness.make_checkpoint(harness.sim_backend, s, f, 6, 2, 160.0) for f in fam for s in range(2)]
        recs = harness.run_branches(cps, harness.sim_backend, repeats=1, seed=0)
        self.assertTrue(recs)
        self.assertTrue(all(r["restored_ok"] for r in recs))
        self.assertEqual({r["family"] for r in recs}, set(fam))
        sim = harness.SimBackend.fresh(0, 5, "layout:" + layouts.variant(1, "cities", "mixed", n_buttons=5)["id"])
        self.assertEqual(len(sim.env.st["labels"]), 5)  # sim-only variable button count


class LeakageLintTests(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp()
        self.tasks = os.path.join(self.d, "tasks")
        os.makedirs(os.path.join(self.tasks, "osworld"))
        self.answers = os.path.join(self.d, "cache")
        os.makedirs(self.answers)
        gold = os.path.join(self.answers, "gold.xlsx")
        open(gold, "w").write("gold")
        json.dump({"id": "1234abcd-test", "evaluator": {"func": "compare_table",
                   "expected": {"type": "local_file", "path": gold, "rules": {"expected": ["Quarterly Revenue 2024"]}}}},
                  open(os.path.join(self.tasks, "osworld", "ow99-chrome-1234abcd.json"), "w"))
        self.entries = [os.path.join(self.tasks, "osworld", "ow99-chrome-1234abcd.json")]
        self.agent = os.path.join(self.d, "agentcwd")
        os.makedirs(self.agent)

    def tearDown(self):
        shutil.rmtree(self.d, ignore_errors=True)

    def test_answer_info(self):
        tj = json.load(open(self.entries[0]))
        paths, lits = leakage_lint.answer_info(tj)
        self.assertIn(os.path.join(self.answers, "gold.xlsx"), paths)
        self.assertIn("Quarterly Revenue 2024", lits)

    def test_clean_run_dir_passes(self):
        art = os.path.join(self.d, "weights.json")
        json.dump({"bias": 1.0, "families": ["libreoffice_calc"]}, open(art, "w"))
        self.assertEqual(leakage_lint.lint(self.entries, [self.agent], [self.answers], [art], base=self.tasks), [])

    def test_answers_reachable_from_the_run_dir_fail(self):
        os.symlink(self.answers, os.path.join(self.agent, "cache"))
        p = leakage_lint.lint(self.entries, [self.agent], [self.answers], base=self.tasks)
        self.assertTrue(any(x["check"] == "readable" and "symlink" in x["how"] for x in p))
        # an agent dir that contains the answers directly
        p2 = leakage_lint.lint(self.entries, [self.d], base=self.tasks)
        self.assertTrue(any(x["check"] == "readable" and x["how"] == "inside" for x in p2))

    def test_tuned_artifacts_with_test_answers_fail(self):
        mem = os.path.join(self.d, "memory")
        os.makedirs(mem)
        open(os.path.join(mem, "notes.md"), "w").write("for ow99-chrome-1234abcd type Quarterly Revenue 2024\n")
        unseen = os.path.join(self.d, "unseen.json")
        json.dump(layouts.frozen(layouts.unseen_layout_set(3)), open(unseen, "w"))
        cal = os.path.join(self.d, "calibrator.json")
        json.dump({"note": "fitted on " + layouts.unseen_layout_set(3)[0]["id"]}, open(cal, "w"))
        p = leakage_lint.lint(self.entries, [self.agent], artifacts=[mem, cal], unseen=unseen, base=self.tasks)
        kinds = {x["kind"] for x in p if x["check"] == "content"}
        self.assertEqual(kinds, {"test_task_id", "gold_literal", "unseen_layout"})

    def test_cli_exit_codes(self):
        lst = os.path.join(self.d, "l.txt")
        open(lst, "w").write(self.entries[0] + "\n")
        r = subprocess.run([sys.executable, os.path.join(ROOT, "bench", "leakage_lint.py"), "--tasks", lst, "--agent-dir", self.agent],
                           capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        r = subprocess.run([sys.executable, os.path.join(ROOT, "bench", "leakage_lint.py"), "--tasks", lst, "--agent-dir", self.d],
                           capture_output=True, text=True)
        self.assertEqual(r.returncode, 1)
        self.assertFalse(json.loads(r.stdout)["ok"])

    def test_repo_weights_do_not_name_the_unseen_layouts(self):
        unseen = os.path.join(self.d, "unseen.json")
        json.dump(layouts.frozen(layouts.unseen_layout_set()), open(unseen, "w"))
        p = leakage_lint.lint([], [], artifacts=[os.path.join(ROOT, "weights"), os.path.join(ROOT, "bench", "app_hints.md")],
                              unseen=unseen, base=self.tasks)
        self.assertEqual([x for x in p if x["kind"] == "unseen_layout"], [])


class HumanPilotTests(unittest.TestCase):
    def sessions(self):
        plan = human_pilot.assign(8, [f"t{i}" for i in range(6)], seed=1)
        out = []
        for s in plan:
            value = s["condition"] == "value_ask"
            asks = [{"kind": "approve", "question": "Send the email to Bob?", "outcome": "approved", "approval_correct": True}]
            if not value:  # threshold asking: more asks, one repeated, one wrong approval, one timeout
                asks += [{"kind": "answer", "question": "Which file?", "outcome": "answered"},
                         {"kind": "answer", "question": "Which  file ?", "outcome": "answered"},
                         {"kind": "approve", "question": "Delete old.txt?", "outcome": "approved", "approval_correct": False},
                         {"kind": "help", "question": "Stuck on the dialog", "outcome": "timeout", "timeout_reason": "no answer in 30 s"}]
            out.append(dict(s, asks=asks, active_s=20.0 if value else 45.0, waiting_s=30.0 if value else 70.0,
                            takeover_steps=0 if value else 2, cancelled=False, timed_out=not value, success=True,
                            total_s=200.0 if value else 260.0, interruption_rating=2 if value else 5))
        return out

    def test_assignment_is_counterbalanced(self):
        plan = human_pilot.assign(8, [f"t{i}" for i in range(6)], seed=1)
        self.assertEqual(len(plan), 48)
        per_task = {}
        for s in plan:
            per_task.setdefault(s["task"], {}).setdefault(s["condition"], 0)
            per_task[s["task"]][s["condition"]] += 1
        self.assertTrue(all(v == {"threshold_ask": 4, "value_ask": 4} for v in per_task.values()))
        self.assertTrue(human_pilot.order_balance(plan)["balanced"])
        firsts = human_pilot.order_balance(plan)["participants_first"]
        self.assertEqual(firsts, {"threshold_ask": 4, "value_ask": 4})

    def test_metrics_and_paired_comparison(self):
        ss = self.sessions()
        pc = human_pilot.per_condition(ss)
        t, v = pc["threshold_ask"], pc["value_ask"]
        self.assertEqual(t["asks_per_task"], 5)
        self.assertEqual(v["asks_per_task"], 1)
        self.assertAlmostEqual(t["repeated_question_ratio"], 1 / 5)  # normalised text: "Which  file ?" repeats
        self.assertAlmostEqual(t["wrong_approval_rate"], 0.5)
        self.assertEqual(v["wrong_approval_rate"], 0.0)
        self.assertEqual(t["timeout_rate"], 1.0)
        self.assertEqual(t["timeout_reasons"], {"no answer in 30 s": 24})
        self.assertEqual(v["interruption_rating"], 2.0)
        cmp_ = human_pilot.paired_compare(ss)
        self.assertEqual(cmp_["unit"], "participant")  # each person did each task once: within-person pairing
        self.assertEqual(cmp_["pairs"], 8)
        self.assertTrue(cmp_["order"]["balanced"])
        self.assertAlmostEqual(cmp_["metrics"]["asks"]["diff"], -4.0)
        self.assertAlmostEqual(cmp_["metrics"]["total_s"]["diff"], -60.0)
        self.assertEqual(cmp_["metrics"]["total_s"]["better_if"], "lower")

    def test_task_pairs_and_unbalanced_flag_and_cli(self):
        d = tempfile.mkdtemp()
        ss = []
        for p in range(4):
            for t in range(3):
                for o, c in enumerate(("threshold_ask", "value_ask")):  # everyone threshold first: unbalanced
                    ss.append({"participant": f"p{p}", "task": f"t{t}", "condition": c, "order": o + 2 * t, "asks": [],
                               "success": (c == "value_ask") or t != 0, "total_s": 100})
        f = os.path.join(d, "s.jsonl")
        jl(f, ss)
        res = human_pilot.main([f, "--margin", "0.1", "--md", os.path.join(d, "h.md")])
        self.assertEqual(res["paired"]["unit"], "participant_task")
        self.assertIn("success_mcnemar", res["paired"])
        self.assertIn("success_noninferiority_verdict", res["paired"])
        self.assertFalse(res["paired"]["order"]["balanced"])
        self.assertIn("ORDER UNBALANCED", open(os.path.join(d, "h.md")).read())


class ContextAblationTests(unittest.TestCase):
    def test_parse_and_compare(self):
        d = tempfile.mkdtemp()
        facts = {"t1": [{"key": "total", "value": "4711"}, {"key": "name", "value": "Zebra Ltd"}]}
        runs, scored = [], []
        for cond, lose in (("ctx_full", False), ("ctx_current", True)):
            for s in (0, 1):
                tag = f"t1-{cond}-s{s}"
                pi = [pi_msg("user", [{"type": "text", "text": "Copy the customer Zebra Ltd total into B2"}]),
                      pi_msg("assistant", [call("get_window_state")], usage={"input": 1000, "output": 10}),
                      pi_msg("toolResult", [{"type": "text", "text": "cell A1 = 4711"}]),
                      pi_msg("assistant", [call("click", element_token="s1:3")], usage={"input": 2000 if not lose else 800, "output": 10})]
                if lose:
                    pi += [pi_msg("toolResult", [{"type": "text", "text": "error: element token s1:3 is stale"}]),
                           pi_msg("assistant", [call("click", element_token="s1:3")], usage={"input": 900, "output": 10}),
                           {"type": "auto_compaction_start"},
                           pi_msg("assistant", [call("type_text", text="4710")], usage={"input": 700, "output": 10}),
                           pi_msg("assistant", [{"type": "text", "text": "DONE"}], usage={"input": 700, "output": 5})]
                else:
                    pi += [pi_msg("toolResult", [{"type": "text", "text": "ok"}]),
                           pi_msg("assistant", [call("type_text", text="4711")], usage={"input": 2100, "output": 10}),
                           pi_msg("assistant", [{"type": "text", "text": "DONE, Zebra Ltd total copied"}], usage={"input": 2200, "output": 5})]
                jl(os.path.join(d, "pi", tag + ".jsonl"), pi)
                g = [{"ev": "action", "tool": "click", "executed": True, "stale_error": lose}]
                if lose:
                    g += [{"ev": "retro_repeat"}, {"ev": "role", "role": "compact", "calls": 1}]
                jl(os.path.join(d, "gate", tag + ".jsonl"), g)
                runs.append({"tag": tag, "task": "t1", "cond": cond, "seed": s, "context": cond[4:], "pi": {"wall_s": 90 if lose else 120}})
                scored.append({"task": "t1", "cond": cond, "seed": s, "success": not lose})
        jl(os.path.join(d, "runs.jsonl"), runs)
        jl(os.path.join(d, "runs_scored.jsonl"), scored)
        fp = os.path.join(d, "facts.json")
        json.dump(facts, open(fp, "w"))
        res = context_ablation.main([d, "--facts", fp, "--md", os.path.join(d, "c.md")])
        full, cur = res["conditions"]["ctx_full"], res["conditions"]["ctx_current"]
        self.assertEqual(full["mean_stale_token_errors"], 0)
        self.assertEqual(cur["mean_stale_token_errors"], 1)
        self.assertEqual(cur["mean_pi_stale_mentions"], 1)
        self.assertEqual(cur["mean_repeated_actions"], 2)  # gate retro_repeat + identical consecutive click
        self.assertEqual(full["facts_lost"], 0)
        self.assertEqual(cur["facts_lost"], 4)  # 2 facts x 2 runs seen but never used
        self.assertEqual(cur["mean_compactions"], 2)
        self.assertEqual(full["mean_success"], 1.0)
        self.assertLess(cur["mean_tok_in"], full["mean_tok_in"])
        p = res["paired"]["ctx_current"]
        self.assertEqual(p["vs"], "ctx_full")
        self.assertAlmostEqual(p["success"]["diff"], -1.0)
        self.assertIn("Shorter context alone is not an improvement", open(os.path.join(d, "c.md")).read())

    def test_docstring_documents_expected_log_fields(self):
        doc = context_ablation.__doc__
        for f in ("stale_error", "retro_repeat", "role=compact", "toolCall", "usage.input", "runs_scored.jsonl"):
            self.assertIn(f, doc)


class ReliabilityPlotTests(unittest.TestCase):
    def test_ties_are_never_split_across_bins(self):
        deltas = {f"c{i}": (1.0 if i % 2 else 0.0) for i in range(10)}
        preds = {f"c{i}": 0.3 for i in range(10)}
        rows = analyze.reliability(deltas, preds, bins=5)
        self.assertEqual(len(rows), 1)  # one predicted value -> one bin, no fake slope
        self.assertEqual(rows[0]["n"], 10)
        preds2 = {f"c{i}": (0.1 if i < 4 else 0.5) for i in range(10)}
        rows2 = analyze.reliability(deltas, preds2, bins=5)
        self.assertEqual([r["n"] for r in rows2], [4, 6])

    def test_reliability_ci_and_plot_png_and_ascii(self):
        fam = {f"c{i}": f"f{i % 3}" for i in range(30)}
        deltas = {f"c{i}": (i % 5 - 2) / 4 for i in range(30)}
        preds = {f"c{i}": (i % 5 - 2) / 5 + (i % 7) * 0.001 for i in range(30)}
        rel = {"look_light": analyze.reliability_ci(deltas, preds, fam, bins=5), "extra_reason": analyze.reliability_ci(deltas, preds, fam, bins=3)}
        for r in rel["look_light"]:
            self.assertLessEqual(r["ci95"][0], r["mean_observed"] + 1e-9)
            self.assertGreaterEqual(r["ci95"][1], r["mean_observed"] - 1e-9)
        d = tempfile.mkdtemp()
        out = analyze.plot_reliability(rel, os.path.join(d, "rel.png"), force_ascii=True)
        self.assertIsNone(out["png"])
        self.assertTrue(os.path.exists(os.path.join(d, "rel.csv")))
        self.assertIn("look_light", open(os.path.join(d, "rel.txt")).read())
        self.assertIn("*", out["ascii"])
        try:
            import matplotlib  # noqa: F401
        except ImportError:
            self.skipTest("matplotlib not installed: the ASCII fallback above is the tested path")
        out2 = analyze.plot_reliability(rel, os.path.join(d, "rel2.png"))
        self.assertEqual(open(out2["png"], "rb").read(8), b"\x89PNG\r\n\x1a\n")

    def test_dryrun_writes_preds_split_and_plots(self):
        d = tempfile.mkdtemp()
        cps, recs, res = dryrun.run(d, seeds=2, prefixes=(2, 4), repeats=1, unseen_layouts=2)
        split = json.load(open(os.path.join(d, "split.json")))
        self.assertEqual(len(split["unseen_layout_test"]), 2)
        self.assertFalse(set(split["unseen_layout_test"]) & set(split["train_families"]))
        preds = [json.loads(x) for x in open(os.path.join(d, "preds.jsonl"))]
        self.assertTrue(any(p["family"].startswith("layout:") for p in preds))
        self.assertTrue(os.path.exists(os.path.join(d, "reliability.csv")))
        self.assertTrue(os.path.exists(os.path.join(d, "reliability_unseen.csv")))
        self.assertTrue(res["plots"]["test"]["png"] or res["plots"]["test"]["ascii"])
        self.assertIn("incremental_reliability", res["test_families_only"])
        # the predictor never sees unseen-layout or test-family outcomes: refitting without them gives the same preds
        train_only = [r for r in recs if r["family"] in split["train_families"]]
        again = dryrun.fit_predictor(cps, train_only, split["train_families"])
        self.assertEqual({(p["checkpoint"], p["treatment"]): p["delta"] for p in again},
                         {(p["checkpoint"], p["treatment"]): p["delta"] for p in preds})

    def test_cli_plot_flag(self):
        d = tempfile.mkdtemp()
        dryrun.run(d, seeds=2, prefixes=(2,), repeats=1)
        env = dict(os.environ, GG_NO_MATPLOTLIB="1")
        r = subprocess.run([sys.executable, os.path.join(ROOT, "bench", "counterfactual", "analyze.py"), os.path.join(d, "branches.jsonl"),
                            "--pred", os.path.join(d, "preds.jsonl"), "--plot", os.path.join(d, "cli.png")], capture_output=True, text=True, env=env)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertTrue(os.path.exists(os.path.join(d, "cli.txt")))
        self.assertIn("Incremental reliability", r.stdout)


if __name__ == "__main__":
    unittest.main()
