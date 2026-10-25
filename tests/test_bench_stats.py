"""bench/stats.py, bench/fair_compare.py and the run_bench.sh value-policy conds (no desktop, no model)."""
import json
import os
import random
import subprocess
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "bench"))
import stats  # noqa: E402
import fair_compare  # noqa: E402

BASH_BENCH = unittest.skipUnless(sys.platform.startswith("linux"), "the bench harness is bash + GNU coreutils on Linux (cp -T, seq; on Windows `bash` is the WSL launcher)")


class StatsTests(unittest.TestCase):
    def test_wilson_matches_the_review_numbers(self):
        lo, hi = stats.wilson(137, 255)
        self.assertAlmostEqual(lo, 0.476, places=3)
        self.assertAlmostEqual(hi, 0.597, places=3)
        self.assertEqual(stats.wilson(0, 0), (0.0, 1.0))
        lo, hi = stats.wilson(0, 10)
        self.assertEqual(lo, 0.0)
        self.assertGreater(hi, 0.2)

    def test_mcnemar_exact(self):
        # 10 discordant pairs, all one way: p = 2 * 0.5^10
        a = [0] * 10 + [1] * 5
        b = [1] * 10 + [1] * 5
        r = stats.mcnemar_exact(a, b)
        self.assertEqual((r["n01"], r["n10"]), (10, 0))
        self.assertAlmostEqual(r["p"], 2 / 1024)
        self.assertEqual(stats.mcnemar_exact([1, 0], [1, 0])["p"], 1.0)
        # balanced discordance -> p = 1
        self.assertAlmostEqual(stats.mcnemar_exact([0, 1, 0, 1], [1, 0, 1, 0])["p"], 1.0)

    def test_permutation_exact_and_sampled(self):
        a = [0.0] * 8
        b = [1.0] * 8
        r = stats.paired_permutation(a, b)
        self.assertTrue(r["exact"])
        self.assertAlmostEqual(r["p"], 2 / 256)
        rng = random.Random(3)
        a2 = [rng.random() for _ in range(40)]
        b2 = [x + rng.gauss(0, 0.01) for x in a2]
        r2 = stats.paired_permutation(a2, b2, reps=2000)
        self.assertFalse(r2["exact"])
        self.assertGreater(r2["p"], 0.01)

    def test_paired_bootstrap_ci_covers_the_difference(self):
        a = [0, 0, 1, 1, 0, 1, 0, 0, 1, 0] * 3
        b = [1, 0, 1, 1, 1, 1, 0, 1, 1, 0] * 3
        r = stats.paired_bootstrap(a, b, reps=1000, seed=2)
        self.assertAlmostEqual(r["diff"], 0.3)
        self.assertLess(r["ci"][0], 0.3)
        self.assertGreater(r["ci"][1], 0.3)
        self.assertGreater(r["ci"][0], 0.0)
        with self.assertRaises(AssertionError):
            stats.paired_bootstrap([1], [1, 0])

    def test_budget_curve_counts_failures_and_missing_costs(self):
        runs = [{"policy": "p", "success": True, "wall_s": 10}, {"policy": "p", "success": True, "wall_s": 100},
                {"policy": "p", "success": False, "wall_s": 5}, {"policy": "p", "success": True, "wall_s": None}]
        c = stats.budget_curve(runs, [10, 100, 1000])["p"]
        self.assertEqual([x[1] for x in c], [0.25, 0.5, 0.5])
        self.assertTrue(all(x[4] == 4 for x in c))


class FairCompareTests(unittest.TestCase):
    def write(self, rows):
        d = tempfile.mkdtemp()
        with open(os.path.join(d, "runs_scored.jsonl"), "w") as f:
            for r in rows:
                f.write(json.dumps(r) + "\n")
        return d

    def test_planned_denominator_paired_tests_and_budget(self):
        rows = []
        for t in range(12):
            for s in (0, 1):
                rows.append({"task": f"t{t}", "cond": "vp_direct", "seed": s, "success": t < 4, "wall_s": 100 + t})
                # the learned policy wins on 6 more tasks but one run hits an infra error (retried later, still infra)
                ok = t < 10
                rows.append({"task": f"t{t}", "cond": "vp_learned", "seed": s, "success": ok, "wall_s": 150 + t,
                             "infra": "relay_error" if (t == 9 and s == 1) else None, "attempt": 1})
        rows.append({"task": "t9", "cond": "vp_learned", "seed": 1, "success": False, "infra": "relay_error", "attempt": 2})
        d = self.write(rows)
        out = os.path.join(d, "cmp.md")
        res = fair_compare.main([d, "--baseline", "vp_direct", "--md", out, "--json", os.path.join(d, "cmp.json")])
        p = res["policies"]
        self.assertEqual(p["vp_direct"]["planned_runs"], 24)
        self.assertEqual(p["vp_direct"]["success"], 8)
        # the infra run counts as a failure in the planned denominator, is dropped from the valid one
        self.assertEqual(p["vp_learned"]["success"], 19)
        self.assertEqual(p["vp_learned"]["valid_runs"], 23)
        self.assertEqual(p["vp_learned"]["infra_or_missing"], 1)
        pr = res["paired"]["vp_learned"]
        self.assertAlmostEqual(pr["diff_bootstrap"]["diff"], (6 - 0.5) / 12)
        self.assertGreater(pr["diff_bootstrap"]["ci"][0], 0)
        self.assertEqual(pr["mcnemar_task_seed"]["n01"], 11)
        self.assertEqual(pr["mcnemar_task_seed"]["n10"], 0)
        self.assertLess(pr["mcnemar_task_seed"]["p"], 0.01)
        self.assertLess(pr["permutation"]["p"], 0.05)
        self.assertIn("vp_learned", res["budget_curve"])
        txt = open(out).read()
        self.assertIn("independent repeats", txt)
        self.assertIn("not evidence of equality", txt)

    def test_missing_planned_task_counts_as_failure(self):
        d = self.write([{"task": "a", "cond": "vp_direct", "seed": 0, "success": True, "wall_s": 1},
                        {"task": "a", "cond": "vp_full", "seed": 0, "success": True, "wall_s": 2}])
        plan = os.path.join(d, "plan.txt")
        open(plan, "w").write("osworld/a.json\nosworld/b.json\n")
        res = fair_compare.main([d, "--planned", plan])
        self.assertEqual(res["planned_tasks"], 2)
        self.assertEqual(res["policies"]["vp_full"]["planned_runs"], 2)
        self.assertEqual(res["policies"]["vp_full"]["rate"], 0.5)


@BASH_BENCH
class RunBenchValuePolicyTests(unittest.TestCase):
    def test_value_policy_conds_share_the_gate8_base(self):
        sh = open(os.path.join(ROOT, "bench", "run_bench.sh")).read()
        self.assertEqual(subprocess.run(["bash", "-n", os.path.join(ROOT, "bench", "run_bench.sh")]).returncode, 0)
        self.assertIn("gate8|vp_*)", sh)  # every vp_ cond gets exactly the gate8 arguments
        for pol in ("direct", "observe_light", "observe_full", "extra_reason", "threshold", "heuristic", "random", "learned"):
            self.assertIn(f"VPOL={pol}", sh)
        self.assertIn('"value_policy\\":\\"$VPOL\\"', sh.replace('\\\\', '\\'))

    def test_repeats_option(self):
        sh = open(os.path.join(ROOT, "bench", "run_bench.sh")).read()
        loop = sh[sh.index("MAXT_ARG=\"\""):sh.index("OUT=${1:-")]
        seeds = sh[sh.index("if [ -n \"${REPEATS_ARG:-}\" ]"):sh.index("THINK=")]
        r = subprocess.run(["bash", "-c", loop + 'SEEDS=0\n' + seeds + 'echo "$SEEDS|$1"', "x", "--repeats", "3", "/out"],
                           capture_output=True, text=True)
        self.assertEqual(r.stdout.strip(), "0 1 2|/out")


class OfficialScoreDisclaimerTests(unittest.TestCase):
    def test_v099_result_is_reported_as_custom_environment_partial(self):
        for f in ("results/README.md", "results/osworld-verified-v099-ds41-2026-10-23/README.md",
                  "results/osworld-verified-v099-ds41-2026-10-23/report.md", "README.md"):
            txt = open(os.path.join(ROOT, f), encoding="utf-8").read()
            self.assertIn("not an official OSWorld-Verified score", txt, f)
            self.assertIn("Wilson", txt, f)  # the Wilson interval travels with the number


if __name__ == "__main__":
    unittest.main()
