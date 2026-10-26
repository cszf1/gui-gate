"""bench/offline_v2.py (offline value-model v2 evaluation) and bench/online_plan.py (online experiment plan generator).

Synthetic results directories only: no model, no gate run, no network.
"""
import json
import os
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "bench"))
import offline_v2 as ov  # noqa: E402
import online_plan as op  # noqa: E402


def _act(sess, aid, ts, tool, executed=True, err=False, decision="direct", tier="model", trail=None, kind="token", role="push button",
         label="OK", p=0.9, feats=None, sig=None, effect="unverifiable"):
    f = {"bias": 1.0, "is_token": 1.0 if kind == "token" else 0.0, "is_pixel": 1.0 if kind == "pixel" else 0.0}
    if tool == "run_command":
        f = {"bias": 1.0, "no_obs": 1.0}
        kind, role = "command", "command"
    f.update(feats or {})
    return {"ev": "action", "ts": ts, "session": sess, "aid": aid, "tool": tool, "executed": executed, "decision": decision, "tier": tier,
            "trail": trail or [decision], "features": f, "p_direct": p, "sig": sig or f"{tool}|1|{role}|{label}{aid}",
            "target": {"kind": kind, "role": role, "label": label}, "is_error": err if executed else None, "stale_error": False,
            "effect": effect if executed else None, "popup": False, "gate_ms": 2.0, "risk_level": None, "handoff": None}


def make_results(d):
    """5 expected tasks: a (ok, 2 relay attempts then valid), b (ok), c (setup_failed), d (truncated row only), e (fail)."""
    os.makedirs(os.path.join(d, "gate"))
    os.makedirs(os.path.join(d, "pi"))
    os.makedirs(os.path.join(d, "setup"))
    os.makedirs(os.path.join(d, "retry"))
    tasks = ["chrome-a", "gimp-b", "os-c", "vlc-d", "chrome-e"]
    with open(os.path.join(d, "setup", "task_list_ovf.txt"), "w") as fh:
        fh.write("\n".join(f"ovf/{t}.json" for t in tasks) + "\n")
    def row(t, attempt, infra, success):
        return {"tag": f"{t}-gate8-s0", "task": t, "cond": "gate8", "seed": 0, "attempt": attempt, "infra": infra, "success": success, "score": 1.0 if success else 0.0}
    # like the real run: the final files keep only the last attempt; the infra attempt 1 row lives in retry/
    scored = [row("chrome-a", 2, None, False), row("gimp-b", 1, None, True),
              row("os-c", 1, "setup_failed", False), row("chrome-e", 1, None, False), row("os-c", 1, "setup_failed", False)][:4]
    with open(os.path.join(d, "runs_scored.jsonl"), "w") as fh:
        fh.write("\n".join(json.dumps(r) for r in scored) + "\n")
    with open(os.path.join(d, "retry", "runs.infra_first.jsonl"), "w") as fh:
        fh.write(json.dumps(row("chrome-a", 1, "relay_error", None)) + "\n")
    with open(os.path.join(d, "runs.jsonl"), "w") as fh:
        fh.write("\n".join(json.dumps(r) for r in scored) + "\n")
        fh.write('{"tag":"vlc-d-gate8-s0","task":"vlc-d","cond":"gate8","seed":0,"attempt":1,"infra":null,"pi":}\n')  # truncated
    # gate logs
    def write(name, evs):
        with open(os.path.join(d, "gate", name), "w") as fh:
            fh.write("\n".join(json.dumps(e) for e in evs) + "\n")
    t0 = 1000.0
    # chrome-a attempt 1 (infra) and attempt 2 (used)
    write("chrome-a-gate8-s0.jsonl", [{"ev": "client", "ts": t0, "session": "a1"}, _act("a1", 1, t0 + 1, "click", err=True, p=0.2)])
    a2 = [{"ev": "client", "ts": t0, "session": "a2"}]
    for i in range(6):
        a2.append(_act("a2", i + 1, t0 + 10 * i, "run_command", err=(i % 2 == 0), p=0.25, decision="direct", tier="rule", trail=["look_light"]))
    a2.append(_act("a2", 7, t0 + 70, "click", err=True, p=0.3))
    a2.append(_act("a2", 8, t0 + 80, "click", executed=False, decision="look_full", p=0.3))
    a2.append({"ev": "obs", "ts": t0 + 81, "session": "a2", "px": 750000})
    a2.append(_act("a2", 9, t0 + 95, "click", err=False, p=0.8))
    a2.append(_act("a2", 10, t0 + 96, "scroll", kind="pixel", role="", err=True, p=0.5))
    # an aborted re-run appended to the same file
    a2.append({"ev": "client", "ts": t0 + 5000, "session": "a3"})
    a2.append(_act("a3", 1, t0 + 5001, "click", err=True, p=0.9))
    write("chrome-a-gate8-s0-try2.jsonl", a2)
    b = [{"ev": "client", "ts": t0, "session": "b1"}]
    for i in range(12):
        b.append(_act("b1", i + 1, t0 + i, "click" if i % 3 else "press_key", kind="token" if i % 3 else "keyboard", err=(i == 5), p=0.9 - 0.05 * (i == 5)))
    write("gimp-b-gate8-s0.jsonl", b)
    write("os-c-gate8-s0.jsonl", [{"ev": "client", "ts": t0, "session": "c1"}, _act("c1", 1, t0 + 1, "click", err=True)])
    write("vlc-d-gate8-s0.jsonl", [{"ev": "client", "ts": t0, "session": "d1"}, _act("d1", 1, t0 + 1, "click")])
    e = [{"ev": "client", "ts": t0, "session": "e1"}]
    for i in range(12):
        e.append(_act("e1", i + 1, t0 + i, "click", err=(i % 4 == 0), p=0.6 if i % 4 == 0 else 0.85))
    write("chrome-e-gate8-s0.jsonl", e)
    # pi log of chrome-a try2: one assistant turn after the look_full refusal
    with open(os.path.join(d, "pi", "chrome-a-gate8-s0-try2.jsonl"), "w") as fh:
        fh.write(json.dumps({"type": "turn_end", "message": {"role": "assistant", "timestamp": int((t0 + 90) * 1000), "durationMs": 9000,
                                                               "usage": {"input": 3000, "output": 200, "cacheRead": 9000}}}) + "\n")
        fh.write(json.dumps({"type": "turn_end", "message": {"role": "assistant", "timestamp": int((t0 + 20) * 1000), "durationMs": 4000,
                                                               "usage": {"input": 1000, "output": 100, "cacheRead": 5000}}}) + "\n")
    return tasks


class OfflineV2Tests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = os.path.join(self.tmp.name, "results", "run1")
        make_results(self.d)

    def tearDown(self):
        self.tmp.cleanup()

    def test_corrupt_lines_duplicates_infra_and_missing_are_excluded_and_listed(self):
        a = ov.audit_runs(self.d)
        bad = a["corrupt_lines"]
        self.assertEqual(len(bad), 1)
        self.assertEqual(bad[0]["file"], "runs.jsonl")
        self.assertEqual(bad[0]["line"], 5)
        self.assertEqual(bad[0]["tag"], "vlc-d-gate8-s0")
        t = a["tasks"]
        self.assertEqual(t["chrome-a"]["status"], "used")
        self.assertEqual(t["chrome-a"]["attempt"], 2, "the first valid attempt: attempt 1 was a relay error")
        self.assertEqual(t["chrome-a"]["excluded_attempts"], [{"attempt": 1, "reason": "infra:relay_error"}])
        self.assertEqual(t["os-c"]["status"], "excluded")
        self.assertEqual(t["os-c"]["reason"], "infra:setup_failed")
        self.assertEqual(t["vlc-d"]["reason"], "no_result_row (runs.jsonl line truncated)")
        c = a["counts"]
        self.assertEqual((c["expected"], c["used"], c["success_used"]), (5, 3, 1))
        self.assertEqual(c["official_rate"], round(100 * 1 / 5, 2))
        self.assertEqual(c["valid_rate"], round(100 * 1 / 3, 2))
        strict = ov.audit_runs(self.d, attempt_policy="first")
        self.assertEqual(strict["tasks"]["chrome-a"]["status"], "excluded")
        self.assertEqual(strict["tasks"]["chrome-a"]["reason"], "infra:relay_error")

    def test_steps_use_the_first_session_and_split_commands_from_gui(self):
        a = ov.audit_runs(self.d)
        rows, st = ov.load_all_steps(self.d, a)
        self.assertEqual(st["later_sessions_excluded_actions"], 1)
        sess = {r["session"] for r in rows}
        self.assertNotIn("a3", sess)
        self.assertNotIn("a1", sess, "attempt 1 of chrome-a was infra")
        self.assertNotIn("c1", sess)
        self.assertNotIn("d1", sess)
        cmds = [r for r in rows if r["is_cmd"] and r["executed"]]
        gui = [r for r in rows if not r["is_cmd"] and r["executed"]]
        self.assertEqual(len(cmds), 6)
        self.assertEqual(len(gui), 3 + 12 + 12)
        # v2 history: the six commands (three failed) do not count as GUI failures
        click7 = next(r for r in rows if r["session"] == "a2" and r["aid"] == 7)
        self.assertEqual(click7["f1"].get("consec_fail", 0.0), 0.0)
        self.assertEqual(click7["f2"]["consec_fail"], 0.0)
        self.assertEqual(click7["f2"]["tool_click"], 1.0)
        scroll = next(r for r in rows if r["session"] == "a2" and r["aid"] == 10)
        self.assertEqual(scroll["f2"]["scroll_pixel"], 1.0)
        self.assertEqual(scroll["f2"]["consec_fail"], 0.0, "click 9 succeeded")
        # hand-back row labelled with the next executed GUI action
        hb = next(r for r in rows if r["session"] == "a2" and r["aid"] == 8)
        self.assertEqual((hb["option"], hb["y_next"]), ("look_full", 1.0))

    def test_calibration_is_reported_separately_for_commands_and_gui(self):
        a = ov.audit_runs(self.d)
        rows, _ = ov.load_all_steps(self.d, a)
        p = ov.builtin_calibration(rows, reps=20, seed=1)
        self.assertEqual(p["gui"]["n"], 27)
        self.assertEqual(p["command"]["n"], 6)
        self.assertEqual(p["all"]["n"], 33)
        self.assertIn("ece_mass", p["gui"])
        self.assertIn("note", p)

    def test_logistic_and_metrics(self):
        xs = [{"bias": 1.0, "x": float(i % 2)} for i in range(200)]
        ys = [1.0 if (i % 2 == 0 or i % 5 == 0) else 0.0 for i in range(200)]
        w = ov.fit_logistic(xs, ys, ["bias", "x"], l2=0.1)
        ps = [ov.predict(w, x) for x in xs]
        m = ov.metrics(ps, ys)
        self.assertGreater(m["auc"], 0.8)
        self.assertLess(m["ece_mass"], 0.05)
        self.assertAlmostEqual(ov.auc([0.1, 0.9, 0.8, 0.2], [0, 1, 1, 0]), 1.0)

    def test_report_never_claims_avoided_failures(self):
        out = os.path.join(self.tmp.name, "out")
        rep = ov.run_all(self.d, out, reps=20, seed=0, bin_path=None)
        self.assertTrue(os.path.exists(os.path.join(out, "report.json")))
        self.assertTrue(os.path.exists(os.path.join(out, "report.md")))
        self.assertTrue(os.path.exists(os.path.join(out, "exclusions.json")))
        claims = rep["claims"]
        self.assertEqual(claims["avoided_failure_after_intervention"]["status"], "not_estimable_from_logs")
        self.assertEqual(claims["predicted_execution_failure"]["status"], "measured")
        with open(os.path.join(out, "report.md"), encoding="utf-8") as fh:
            md = fh.read()
        self.assertIn("not estimable", md)
        # costs: the look_full hand-back cost one model turn of real tokens
        lf = rep["costs"]["options"]["look_full"]
        self.assertEqual(lf["n"], 1)
        self.assertEqual(lf["tokens_input_mean"], 3000)
        self.assertAlmostEqual(lf["extra_s_mean"], 15.0, places=3)
        # the input results are not modified
        self.assertFalse(os.path.exists(os.path.join(self.d, "report.json")))

    def test_v0103_groups_dev_test_baselines_and_label_readings(self):
        a = ov.audit_runs(self.d)
        rows, _ = ov.load_all_steps(self.d, a)
        self.assertTrue(all("group" in r for r in rows))
        self.assertEqual(ov.task_group("chrome-a-layout2"), "chrome-a", "a layout variant is the same group")
        self.assertEqual(ov.task_group("x", {"x": "g"}), "g")
        p = ov.predictors(rows, reps=10, seed=0)
        self.assertEqual(p["generalisation"]["cross_run"].split(":")[0], "not available")
        for name in ("gui_v1_refit", "gui_v2"):
            self.assertEqual(p["hyperparameters"][name]["chosen_on"], "development task groups only")
        self.assertIn("random_same_budget", p["baselines"])
        self.assertIn("equal_time_wait", p["baselines"]["online_only"])
        # the scroll-repeat label has a switch and both readings are reported
        self.assertIn("alternate", p["label_sensitivity"])
        rows_nf, _ = ov.load_all_steps(self.d, a, scroll_repeat_fails=False)
        ex = [r for r in rows if r["executed"]]
        ex_nf = [r for r in rows_nf if r["executed"]]
        self.assertEqual([r["y"] for r in ex], [r["y_alt"] for r in ex_nf])
        self.assertEqual([r["y_alt"] for r in ex], [r["y"] for r in ex_nf])
        out = os.path.join(self.tmp.name, "out3")
        rep = ov.run_all(self.d, out, reps=10, seed=0, bin_path=None)
        with open(os.path.join(out, "report.md"), encoding="utf-8") as fh:
            md = fh.read()
        self.assertIn("post-hoc top-k screen", md)
        self.assertIn("no paired branch data", md)
        self.assertEqual(rep["claims"]["flagging_budget"]["status"], "post_hoc_top_k_screen")
        lf = rep["costs"]["options"]["look_full"]
        self.assertIn("extra_s_p90", lf)
        self.assertEqual(lf["kind"], "next_action_gap_upper_bound")

    def test_incremental_calibration(self):
        self.assertEqual(ov.incremental_calibration([])["status"], "no paired branch data")
        br = []
        for i in range(10):
            tau = -0.2 + 0.05 * i
            for rep in range(3):
                br.append({"checkpoint": f"c{i}", "option": "look_full", "tau_hat": tau, "y_option": 1.0 if (i >= 5 or rep == 0) else 0.0, "y_direct": 1.0 if i < 3 else 0.0})
        ic = ov.incremental_calibration(br, bins=2, reps=50)
        self.assertEqual(ic["status"], "ok")
        self.assertEqual(ic["checkpoints"], 10)
        self.assertLess(ic["bins"][0]["mean_tau_hat"], 0.0, "negative predicted increments are kept")
        self.assertTrue(any(b["mean_observed_diff"] < 0 for b in ic["bins"]) or ic["bins"][0]["mean_observed_diff"] <= ic["bins"][1]["mean_observed_diff"])


class OnlinePlanTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = os.path.join(self.tmp.name, "run1")
        self.tasks = make_results(self.d)

    def tearDown(self):
        self.tmp.cleanup()

    def test_plan_is_frozen_seeded_and_reproducible(self):
        tl = os.path.join(self.d, "setup", "task_list_ovf.txt")
        p1 = op.make_plan(tl, arms=["legacy", "v2"], holdout_frac=0.4, seed=7, repeats=2, name="t")
        p2 = op.make_plan(tl, arms=["legacy", "v2"], holdout_frac=0.4, seed=7, repeats=2, name="t")
        self.assertEqual(p1, p2)
        self.assertEqual(p1["plan_sha256"], op.plan_hash(p1))
        h, d = set(p1["tasks"]["holdout"]), set(p1["tasks"]["development"])
        self.assertFalse(h & d)
        self.assertEqual(len(h) + len(d), 5)
        self.assertEqual(len(h), 2)
        # every holdout task runs under every arm and repeat exactly once
        cells = [(r["task"], r["arm"], r["repeat"]) for r in p1["schedule"]]
        self.assertEqual(len(cells), len(set(cells)))
        self.assertEqual(len(cells), 2 * 2 * 2)
        self.assertEqual({r["task"] for r in p1["schedule"]}, h)
        # arms differ only in GUI_GATE_VALUE_MODEL
        envs = {a["name"]: a["env"] for a in p1["arms"]}
        self.assertEqual(envs["legacy"]["GUI_GATE_VALUE_MODEL"], "legacy")
        self.assertEqual(envs["v2"]["GUI_GATE_VALUE_MODEL"], "v2")
        self.assertEqual({k: v for k, v in envs["legacy"].items() if k != "GUI_GATE_VALUE_MODEL"},
                         {k: v for k, v in envs["v2"].items() if k != "GUI_GATE_VALUE_MODEL"})
        p3 = op.make_plan(tl, arms=["legacy", "v2"], holdout_frac=0.4, seed=8, repeats=2, name="t")
        self.assertNotEqual([r["task"] for r in p1["schedule"]], [r["task"] for r in p3["schedule"]])
        self.assertTrue(p1["dry"])
        self.assertIn("never starts", p1["note"])

    def test_cli_writes_the_plan_and_starts_nothing(self):
        out = os.path.join(self.tmp.name, "plan.json")
        tl = os.path.join(self.d, "setup", "task_list_ovf.txt")
        rc = op.main([tl, "-o", out, "--seed", "3", "--holdout", "0.4"])
        self.assertEqual(rc, 0)
        with open(out) as fh:
            p = json.load(fh)
        self.assertEqual(p["seed"], 3)
        self.assertEqual(p["plan_sha256"], op.plan_hash(p))
        with open(os.path.join(ROOT, "bench", "online_plan.py"), encoding="utf-8") as fh:
            src = fh.read()
        for forbidden in ("subprocess", "os.system", "popen", "run_bench"):
            self.assertNotIn(forbidden + "(", src.lower().replace(" ", ""))
        self.assertNotIn("import subprocess", src)

    def _task_list(self):
        p = os.path.join(self.tmp.name, "tl.txt")
        doms = ["chrome", "libreoffice_impress", "libreoffice_calc", "multi_apps", "gimp"]
        with open(p, "w") as fh:
            fh.write("\n".join(f"ovf/{d}-{i:04d}-x.json" for d in doms for i in range(6)) + "\n")
        return p

    def test_pilots_refuse_frozen_without_every_hash(self):
        tl = self._task_list()
        draft = op.make_policy_pilot(tl, {"gate_commit": "acaba69"})
        self.assertEqual(draft["status"], "draft")
        self.assertIn("main_model", draft["frozen"]["missing"])
        self.assertIn("DRAFT", draft["note_draft"])
        with self.assertRaises(ValueError):
            op.make_policy_pilot(tl, {"gate_commit": "acaba69"}, require_frozen=True)
        full = {k: "a" * 64 for k in op.FREEZE_KEYS}
        full["gate_commit"] = "acaba69"
        pol = op.make_policy_pilot(tl, full)
        self.assertEqual(pol["status"], "frozen")
        self.assertEqual(pol["runs"], 108)
        self.assertEqual(sorted(pol["selection"]["tasks"]), sorted(op.POLICY_DOMAINS))
        self.assertTrue(all(len(v) == 3 for v in pol["selection"]["tasks"].values()))
        self.assertEqual(pol["seed"], 20261026)
        self.assertEqual(pol, op.make_policy_pilot(tl, full), "seeded and reproducible")
        self.assertEqual(set(pol["conditions"]), {"direct_with_safety", "legacy", "frozen_new"})
        self.assertIn("McNemar", pol["analysis"]["not_used"])
        self.assertIn("never infra", pol["infra"]["not_infra"])
        self.assertEqual(pol["plan_sha256"], op.plan_hash(pol))
        inc = op.make_increment_pilot(full)
        self.assertEqual(inc["branches"], 216)
        self.assertEqual(len(inc["checkpoints"]), 24)
        self.assertEqual(inc["analysis"]["unit"], "checkpoint (cluster)")
        self.assertEqual(set(inc["arms"]), {"direct", "look_full", "matched_wait"})
        out = os.path.join(self.tmp.name, "pil")
        self.assertEqual(op.main(["pilots", tl, "-o", out]), 0)
        with open(os.path.join(out, "policy_pilot.json")) as fh:
            self.assertEqual(json.load(fh)["status"], "draft")
        self.assertEqual(op.main(["pilots", tl, "-o", out, "--require-frozen"]), 2)


if __name__ == "__main__":
    unittest.main()
