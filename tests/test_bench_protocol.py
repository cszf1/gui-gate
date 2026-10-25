"""Frozen manifest, interleaved schedule, run_bench.sh guards, non-inferiority, protocol doc and results wording
(offline: no desktop, no model, no network)."""
import json
import os
import random
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "bench"))
import fair_compare  # noqa: E402
import manifest as mf  # noqa: E402
import schedule  # noqa: E402
import stats  # noqa: E402

RUN_BENCH = os.path.join(ROOT, "bench", "run_bench.sh")


def write_tasks(d, names):
    os.makedirs(os.path.join(d, "osworld"), exist_ok=True)
    entries = []
    for n in names:
        e = f"osworld/{n}.json"
        json.dump({"id": n, "snapshot": n.split("-")[1] if "-" in n else n, "instruction": "x",
                   "evaluator": {"func": "f", "expected": {"type": "rule", "rules": {"expected": [f"gold-{n}"]}}}},
                  open(os.path.join(d, e), "w"))
        entries.append(e)
    return entries


NAMES = ["ow01-libreoffice_calc-aaaaaaaa", "ow02-libreoffice_calc-bbbbbbbb", "ow03-chrome-cccccccc", "ow04-chrome-dddddddd",
         "ow05-gimp-eeeeeeee", "ow06-vlc-ffffffff", "ow07-libreoffice_writer-11111111", "ow08-multi_apps-22222222"]


class FamilyTests(unittest.TestCase):
    def test_family_names(self):
        self.assertEqual(mf.family_of("osworld/ow01-libreoffice_calc-7efeb4b1.json"), "libreoffice_calc")
        self.assertEqual(mf.family_of("osguard/og02-writer-pdf-exists.json"), "osguard-writer")
        self.assertEqual(mf.family_of("ovf/chrome-030eeff7-b492-4218-b312-701ec99ee0cc.json"), "chrome")
        self.assertEqual(mf.family_of("ovf/multi_apps-00fa164e-2612-4439-992e-157d019a8436.json"), "multi_apps")


class ManifestTests(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp()
        self.entries = write_tasks(self.d, NAMES)

    def tearDown(self):
        shutil.rmtree(self.d, ignore_errors=True)

    def build(self, **kw):
        return mf.build(self.entries, base=self.d, **kw)

    def test_split_is_by_family_and_hash_is_stable(self):
        m = self.build(test_frac=0.5, seed=3)
        self.assertEqual(mf.validate(m, self.d), [])
        fam_side = {}
        for r in m["tasks"]:
            fam_side.setdefault(r["family"], set()).add(r["split"])
        self.assertTrue(all(len(v) == 1 for v in fam_side.values()))
        self.assertIn("test", {r["split"] for r in m["tasks"]})
        self.assertEqual(self.build(test_frac=0.5, seed=3)["hash"], m["hash"])  # created/notes not hashed
        self.assertNotEqual(self.build(test_frac=0.5, seed=4)["hash"], m["hash"])

    def test_edits_break_the_hash(self):
        m = self.build(test_families=["chrome"], exclusions={"ow04-chrome-dddddddd": "needs Google account"})
        self.assertEqual(mf.validate(m, self.d), [])
        m2 = json.loads(json.dumps(m))
        m2["tasks"][0]["split"] = "test"
        self.assertTrue(any("hash" in e for e in mf.validate(m2, self.d)))
        m3 = json.loads(json.dumps(m))
        m3["tasks"][3]["reason"] = ""
        errs = mf.validate(m3, self.d)
        self.assertTrue(any("without a reason" in e for e in errs))
        # editing a task JSON after freezing is detected
        open(os.path.join(self.d, self.entries[2]), "a").write(" ")
        self.assertTrue(any("changed since" in e for e in mf.validate(m, self.d)))

    def test_exclusions_need_known_tasks(self):
        with self.assertRaises(ValueError):
            self.build(exclusions={"nope": "x"})
        with self.assertRaises(ValueError):
            self.build(test_families=["no_such_family"])

    def test_guard(self):
        m = self.build(test_families=["chrome", "gimp"], exclusions={"ow04-chrome-dddddddd": "needs Google account"})
        test = [r["entry"] for r in mf.runnable(m, "test")]
        dev = [r["entry"] for r in mf.runnable(m, "dev")]
        self.assertEqual(sorted(mf.task_name(e) for e in test), ["ow03-chrome-cccccccc", "ow05-gimp-eeeeeeee"])
        ok, split, _ = mf.guard(dev, m, None, self.d)
        self.assertTrue(ok)
        self.assertEqual(split, "dev")
        self.assertEqual(mf.guard(test, m, None, self.d)[:2], (False, "test"))          # no hash
        self.assertFalse(mf.guard(test, m, "0" * 64, self.d)[0])                       # wrong hash
        self.assertTrue(mf.guard(test, m, m["hash"], self.d)[0])                       # exact split + hash
        self.assertFalse(mf.guard(test[:1], m, m["hash"], self.d)[0])                  # cherry-picked subset
        self.assertFalse(mf.guard(test + dev[:1], m, m["hash"], self.d)[0])            # mixed with dev
        self.assertFalse(mf.guard(test + ["osworld/ow04-chrome-dddddddd.json"], m, m["hash"], self.d)[0])  # excluded task
        self.assertTrue(mf.guard(test, None, None, self.d)[0])                         # no manifest: nothing is test
        # per-task form (counterfactual/osworld.py)
        self.assertFalse(mf.task_allowed(test[0], m, None, self.d)[0])
        self.assertTrue(mf.task_allowed(test[0], m, m["hash"], self.d)[0])
        self.assertTrue(mf.task_allowed(dev[0], m, None, self.d)[0])
        self.assertFalse(mf.task_allowed("osworld/ow04-chrome-dddddddd.json", m, m["hash"], self.d)[0])

    def test_denominators_count_infra_per_policy(self):
        m = self.build(test_families=["chrome", "gimp", "vlc"])
        plan = [r["task"] for r in mf.runnable(m, "test")]
        rows = []
        for t in plan:
            for s in (0, 1):
                rows.append({"task": t, "cond": "vp_direct", "seed": s, "success": True})
        # vp_learned: one relay error (retried, still infra), one setup failure, one missing run
        for t in plan:
            for s in (0, 1):
                if (t, s) == (plan[0], 0):
                    rows.append({"task": t, "cond": "vp_learned", "seed": s, "infra": "relay_error", "attempt": 1})
                    rows.append({"task": t, "cond": "vp_learned", "seed": s, "infra": "relay_error", "attempt": 2})
                elif (t, s) == (plan[1], 1):
                    rows.append({"task": t, "cond": "vp_learned", "seed": s, "infra": "setup_failed"})
                elif (t, s) == (plan[2], 1):
                    continue
                else:
                    rows.append({"task": t, "cond": "vp_learned", "seed": s, "success": True})
        rows.append({"task": "ow01-libreoffice_calc-aaaaaaaa", "cond": "vp_learned", "seed": 0, "success": True})  # dev task
        res = mf.denominators(m, rows, "test")
        p = res["policies"]["vp_learned"]
        self.assertEqual(len(plan), 4)  # chrome x2, gimp, vlc
        self.assertEqual(p["planned_runs"], 8)
        self.assertEqual(p["planned_success"], 5)
        self.assertEqual(p["valid_runs"], 5)
        self.assertEqual(p["infra_by_kind"], {"relay_error": 1, "setup_failed": 1, "missing": 1})
        self.assertEqual(p["infra_or_missing"], 3)
        self.assertEqual(p["outside_plan"], ["ow01-libreoffice_calc-aaaaaaaa"])
        self.assertEqual(res["policies"]["vp_direct"]["planned_rate"], 1.0)

    def test_cli_create_validate_list(self):
        lst = os.path.join(self.d, "list.txt")
        open(lst, "w").write("\n".join(self.entries) + "\n")
        out = os.path.join(self.d, "m.json")
        env = dict(os.environ)
        r = subprocess.run([sys.executable, os.path.join(ROOT, "bench", "manifest.py"), "create", "--list", lst, "--out", out,
                            "--all-dev", "--margin", "0.05"], capture_output=True, text=True, env=env, cwd=self.d)
        self.assertEqual(r.returncode, 0, r.stderr)
        m = json.load(open(out))
        self.assertEqual(m["noninferiority_margin"], 0.05)
        self.assertEqual({r["split"] for r in m["tasks"]}, {"dev"})

    def test_default_manifest_marks_the_development_tasks_dev(self):
        m = mf.load(mf.DEFAULT_MANIFEST)
        self.assertEqual(mf.validate(m), [])
        self.assertEqual({r["split"] for r in m["tasks"]}, {"dev"})
        order = mf.read_list(os.path.join(ROOT, "bench", "tasks", "order.txt"))
        self.assertEqual(sorted(r["entry"] for r in m["tasks"]), sorted(order))


class ScheduleTests(unittest.TestCase):
    def test_abba_two_conds(self):
        s = schedule.schedule(["t1", "t2", "t3", "t4"], ["A", "B"], ["0", "1"], "abba")
        blocks = [tuple(c for r, t, c in s if (r, t) == k) for k in sorted({(r, t) for r, t, _ in s}, key=lambda k: (k[0], k[1]))]
        self.assertEqual(blocks[:4], [("A", "B"), ("B", "A"), ("A", "B"), ("B", "A")])
        self.assertEqual(blocks[4:], [("B", "A"), ("A", "B"), ("B", "A"), ("A", "B")])  # repeat 1 starts flipped
        bal = schedule.balance(s)
        self.assertEqual(bal["A"], {0: 4, 1: 4})
        self.assertEqual(len(s), 16)

    def test_blocked_is_the_old_order(self):
        s = schedule.schedule(["t1", "t2"], ["direct", "gate8"], ["0"], "blocked")
        self.assertEqual(s, [("0", "t1", "direct"), ("0", "t1", "gate8"), ("0", "t2", "direct"), ("0", "t2", "gate8")])

    def test_three_conds_balanced_positions(self):
        conds = ["a", "b", "c"]
        s = schedule.schedule([f"t{i}" for i in range(6)], conds, ["0"], "abba")
        bal = schedule.balance(s)
        for c in conds:
            self.assertEqual(sorted(bal[c].values()), [2, 2, 2])  # every cond equally often 1st / 2nd / 3rd
        rows = schedule.williams_rows(4)
        follows = {}
        for r in rows:
            for x, y in zip(r, r[1:]):
                follows[(x, y)] = follows.get((x, y), 0) + 1
        self.assertEqual(set(follows.values()), {1})  # Williams: each ordered pair once


class RunBenchGuardTests(unittest.TestCase):
    def setUp(self):
        self.sh = open(RUN_BENCH).read()

    def test_options_and_order(self):
        self.assertEqual(subprocess.run(["bash", "-n", RUN_BENCH]).returncode, 0)
        loop = self.sh[self.sh.index("MAXT_ARG=\"\""):self.sh.index("OUT=${1:-")]
        r = subprocess.run(["bash", "-c", loop + 'echo "${MANIFEST_ARG:-}|${MANIFEST_HASH_ARG:-}|${INTERLEAVE_ARG:-}|$1"', "x",
                            "--manifest", "m.json", "--manifest-hash", "abc", "--interleave", "abba", "/out"], capture_output=True, text=True)
        self.assertEqual(r.stdout.strip(), "m.json|abc|abba|/out")
        g = self.sh.index("manifest.py\" guard")
        self.assertLess(g, self.sh.index("pi mcp list"))           # refused before the preflight / any run
        self.assertLess(g, self.sh.index("run_cell $task $c $s"))
        self.assertIn("exit 3", self.sh[g:g + 600])
        self.assertIn("leakage_lint.py", self.sh)
        self.assertIn('"repeat_kind":"independent"', self.sh)
        self.assertIn('"api_seed_accepted"', self.sh)
        self.assertIn('"manifest_hash"', self.sh)
        self.assertIn('"run_wall_s"', self.sh)
        self.assertIn("ctx_*|gate8|vp_*)", self.sh)  # ctx_ and vp_ conds get exactly the gate8 arguments
        self.assertIn("schedule.py", self.sh)

    def run_guard(self, lst, manifest=None, mhash=None):
        """execute the script's own guard block in isolation"""
        a = self.sh.index("# frozen manifest guard")
        b = self.sh.index("# test split: the agent's run directories")
        block = self.sh[a:b]
        out = tempfile.mkdtemp()
        pre = f'HERE={os.path.join(ROOT, "bench")}\nOUT={out}\nLIST={lst}\n'
        if manifest:
            pre += f"MANIFEST_ARG={manifest}\n"
        if mhash:
            pre += f"MANIFEST_HASH_ARG={mhash}\n"
        r = subprocess.run(["bash", "-c", "set -u\n" + pre + block + 'echo "SPLIT=$SPLIT"'], capture_output=True, text=True)
        return r

    def test_guard_block_refuses_test_split_without_hash(self):
        d = tempfile.mkdtemp()
        entries = write_tasks(d, NAMES)
        # tasks must resolve relative to bench/tasks for the hash check: use absolute entries
        abs_entries = [os.path.join(d, e) for e in entries]
        m = mf.build(abs_entries, test_families=["chrome"])
        mp = os.path.join(d, "m.json")
        json.dump(m, open(mp, "w"))
        test_list = os.path.join(d, "test.txt")
        open(test_list, "w").write("\n".join(r["entry"] for r in mf.runnable(m, "test")) + "\n")
        dev_list = os.path.join(d, "dev.txt")
        open(dev_list, "w").write("\n".join(r["entry"] for r in mf.runnable(m, "dev")) + "\n")
        r = self.run_guard(test_list, mp)
        self.assertEqual(r.returncode, 3, r.stdout + r.stderr)
        self.assertIn("refused by the manifest guard", r.stderr)
        r = self.run_guard(test_list, mp, "f" * 64)
        self.assertEqual(r.returncode, 3)
        r = self.run_guard(test_list, mp, m["hash"])
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("SPLIT=test", r.stdout)
        r = self.run_guard(dev_list, mp)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("SPLIT=dev", r.stdout)

    def test_schedule_cli_matches_module(self):
        d = tempfile.mkdtemp()
        lst = os.path.join(d, "l.txt")
        open(lst, "w").write("a.json\nb.json\n")
        out = os.path.join(d, "s.txt")
        subprocess.run([sys.executable, os.path.join(ROOT, "bench", "schedule.py"), "--tasks", lst, "--conds", "vp_direct vp_learned",
                        "--repeats", "0 1 2", "--mode", "abba", "--out", out], check=True)
        lines = [x.split() for x in open(out)]
        self.assertEqual(len(lines), 12)
        self.assertEqual(lines[0], ["0", "a.json", "vp_direct"])
        self.assertEqual(lines[2], ["0", "b.json", "vp_learned"])


class NonInferiorityTests(unittest.TestCase):
    def test_clear_noninferiority(self):
        rng = random.Random(1)
        a = [1.0 if rng.random() < 0.6 else 0.0 for _ in range(200)]
        b = list(a)  # identical outcomes, margin 0.1 -> non-inferior
        r = stats.noninferiority(a, b, 0.10, reps=2000, seed=2)
        self.assertTrue(r["noninferior"])
        self.assertGreater(r["lower"], -0.10)
        self.assertLess(r["p_one_sided"], 0.05)
        self.assertIn("non-inferior within the pre-registered margin", stats.verdict(r))

    def test_small_sample_not_shown_and_wording(self):
        a = [1, 0, 1, 1, 0, 1, 0, 1]
        b = [1, 0, 0, 1, 0, 1, 1, 0]
        r = stats.noninferiority(a, b, 0.05, reps=2000, seed=2)
        self.assertFalse(r["noninferior"])
        v = stats.verdict(r)
        self.assertIn("NOT shown", v)
        self.assertIn("never evidence of equality", v)
        self.assertNotIn("equal.", v.replace("equality", ""))
        # a clearly worse new policy is not non-inferior
        r2 = stats.noninferiority([1] * 30, [0] * 10 + [1] * 20, 0.1, reps=2000)
        self.assertFalse(r2["noninferior"])
        self.assertLess(r2["lower"], -0.1)

    def test_exploratory_margin_flag(self):
        r = stats.noninferiority([1, 1, 1], [1, 1, 1], 0.2, preregistered=False)
        self.assertIn("EXPLORATORY", stats.verdict(r))
        self.assertEqual(stats.noninferiority([], [], 0.1)["noninferior"], False)
        with self.assertRaises(AssertionError):
            stats.noninferiority([1], [1], -0.1)

    def test_fair_compare_reads_the_margin_from_the_manifest(self):
        d = tempfile.mkdtemp()
        entries = [os.path.join(d, e) for e in write_tasks(d, [f"ow{i:02d}-chrome-{i:08x}" for i in range(20)])]
        m = mf.build(entries, test_families=["chrome"], margin=0.10)
        mp = os.path.join(d, "m.json")
        json.dump(m, open(mp, "w"))
        rows = []
        for e in entries:
            t = mf.task_name(e)
            for s in (0, 1, 2):
                rows.append({"task": t, "cond": "vp_direct", "seed": s, "success": True, "wall_s": 100})
                rows.append({"task": t, "cond": "vp_learned", "seed": s, "success": True, "wall_s": 60,
                             "infra": "relay_error" if (t.endswith("00000003") and s == 2) else None})
        run = os.path.join(d, "run")
        os.makedirs(run)
        with open(os.path.join(run, "runs_scored.jsonl"), "w") as f:
            for r in rows:
                f.write(json.dumps(r) + "\n")
        md = os.path.join(d, "cmp.md")
        res = fair_compare.main([run, "--manifest", mp, "--split", "test", "--baseline", "vp_direct", "--md", md])
        pr = res["paired"]["vp_learned"]
        self.assertTrue(pr["noninferiority"]["preregistered"])
        self.assertEqual(pr["noninferiority"]["margin"], 0.10)
        self.assertTrue(pr["noninferiority"]["noninferior"])
        self.assertEqual(res["policies"]["vp_learned"]["infra_by_kind"], {"relay_error": 1})
        self.assertEqual(res["policies"]["vp_learned"]["planned_runs"], 60)
        self.assertEqual(res["manifest_hash"], m["hash"])
        txt = open(md).read()
        self.assertIn("Non-inferiority", txt)
        self.assertIn("not evidence of equality", txt)
        # a margin that differs from the manifest's is exploratory
        res2 = fair_compare.main([run, "--manifest", mp, "--margin", "0.2", "--baseline", "vp_direct"])
        self.assertFalse(res2["paired"]["vp_learned"]["noninferiority"]["preregistered"])


class ProtocolDocTests(unittest.TestCase):
    def test_protocol_doc_covers_the_review_items(self):
        txt = open(os.path.join(ROOT, "docs", "eval-protocol.md"), encoding="utf-8").read()
        for s in ("research experiment", "product experiment", "only the intervention policy", "frozen", "dev split",
                  "heuristics only", "P(direct) + fixed gains", "learned per-option gains", "incremental calibration",
                  "auto-observe", "heals", "context compression", "7-role pack", "driver error", "local subgoal",
                  "task success within budget", "100–200 checkpoints × 5 treatments × 3 repeats", "no power claim",
                  "files, focus, clipboard, windows, app state and timers", "The same screenshot is not the same state",
                  "independent repeats", "Not significant\" is not \"equal\"",
                  "custom environment based on OSWorld tasks and evaluators", "planned", "valid"):
            self.assertIn(s, txt, s)


class ResultsWordingTests(unittest.TestCase):
    PHRASE = "custom environment based on OSWorld tasks and evaluators"

    def test_every_osworld_results_readme_carries_the_wording(self):
        rdir = os.path.join(ROOT, "results")
        files = ["README.md"] + [os.path.join(d, "README.md") for d in os.listdir(rdir)
                                 if os.path.exists(os.path.join(rdir, d, "README.md"))]
        checked = 0
        for f in files:
            txt = open(os.path.join(rdir, f), encoding="utf-8").read()
            if "OSWorld" not in txt or f.startswith("counterfactual-dryrun"):
                continue
            checked += 1
            self.assertIn(self.PHRASE, txt, f)
            self.assertIn("leaderboard", txt, f)
        self.assertGreaterEqual(checked, 9)
        for f in ("results/osworld-verified-v099-ds41-2026-10-23/report.md", "bench/README.md"):
            self.assertIn(self.PHRASE, open(os.path.join(ROOT, f), encoding="utf-8").read(), f)
        import osworld_verified_report
        self.assertIn(self.PHRASE, osworld_verified_report.DISCLAIMER)  # regenerated reports keep the wording
        self.assertIn("not comparable to the", osworld_verified_report.DISCLAIMER)

    def test_v099_numbers_match_the_committed_data(self):
        d = os.path.join(ROOT, "results", "osworld-verified-v099-ds41-2026-10-23")
        best = {}
        for line in open(os.path.join(d, "runs_scored.jsonl")):
            r = json.loads(line)
            if r["task"] not in best or (r.get("attempt") or 1) >= (best[r["task"]].get("attempt") or 1):
                best[r["task"]] = r
        valid = [r for r in best.values() if not r.get("infra")]
        k = sum(1 for r in valid if r.get("success"))
        lo, hi = stats.wilson(k, len(valid))
        plo, phi = stats.wilson(k, 361)
        for f in ("README.md", "report.md"):
            txt = open(os.path.join(d, f), encoding="utf-8").read()
            self.assertIn(f"{k}/{len(valid)} = {100 * k / len(valid):.1f} %", txt, f)
            self.assertIn(f"{100 * lo:.1f} %–{100 * hi:.1f} %", txt, f)
            self.assertIn(f"{k}/361 = {100 * k / 361:.1f} %", txt, f)
            self.assertIn(f"{100 * plo:.1f} %–{100 * phi:.1f} %", txt, f)
        top = open(os.path.join(ROOT, "results", "README.md"), encoding="utf-8").read()
        self.assertNotIn("132/361", top)


if __name__ == "__main__":
    unittest.main()
