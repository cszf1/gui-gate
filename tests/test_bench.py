"""bench/ helpers (no desktop needed): text-only detection, task compat rewrites, reset copy semantics."""
import json
import os
import subprocess
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "bench"))
import osworld_native as onat  # noqa: E402


class ModelInfoTests(unittest.TestCase):
    def run_info(self, settings, models):
        d = tempfile.mkdtemp()
        json.dump(settings, open(os.path.join(d, "settings.json"), "w"))
        json.dump(models, open(os.path.join(d, "models.json"), "w"))
        r = subprocess.run([sys.executable, os.path.join(ROOT, "bench", "pi_model_info.py"), "text_only"],
                           env={**os.environ, "PI_AGENT_DIR": d}, capture_output=True, text=True)
        return r.stdout.strip()

    def test_text_only_model(self):
        m = {"providers": {"inception": {"models": [{"id": "mercury-3", "input": ["text"]}]},
                           "tr": {"models": [{"id": "ds", "input": ["text", "image"]}]}}}
        self.assertEqual(self.run_info({"defaultProvider": "inception", "defaultModel": "mercury-3"}, m), "1")
        self.assertEqual(self.run_info({"defaultProvider": "tr", "defaultModel": "ds"}, m), "0")
        self.assertEqual(self.run_info({"defaultProvider": "x", "defaultModel": "unknown"}, m), "0")


class CompatTests(unittest.TestCase):
    def task(self, name):
        return json.load(open(os.path.join(ROOT, "bench", "tasks", "osguard", name)))

    def test_terminal_opens_in_task_home(self):
        for n in ["og04-os-copy-dir2-conflict.json", "og05-os-fails-keep.json", "og07-os-chmod-sentinel-dirs.json"]:
            t = onat.compat(self.task(n))
            cmds = json.dumps([s for s in t["config"] if s["type"] == "execute"])
            self.assertNotIn("hotkey('ctrl', 'alt', 't')", cmds, n)
            self.assertIn("--working-directory=/home/user", cmds, n)

    def test_other_steps_untouched(self):
        t0 = self.task("og07-os-chmod-sentinel-dirs.json")
        t1 = onat.compat(t0)
        self.assertEqual(t0["evaluator"], t1["evaluator"])
        self.assertEqual(t0["osguard"], t1["osguard"])
        self.assertEqual(len(t0["config"]), len(t1["config"]))
        self.assertIn("cd testDir", json.dumps(t1["config"]))

    def test_libreoffice_title(self):
        t = {"config": [{"type": "activate_window", "parameters": {"window_name": "a.xlsx - LibreOffice Calc"}}]}
        self.assertEqual(onat.compat(t)["config"][0]["parameters"]["window_name"], "a.xlsx \u2014 LibreOffice Calc")


class ResetCopyTests(unittest.TestCase):
    def test_cp_T_does_not_nest(self):
        d = tempfile.mkdtemp()
        src, dst = os.path.join(d, "base", "Desktop"), os.path.join(d, "home", "Desktop")
        os.makedirs(src)
        open(os.path.join(src, "a.txt"), "w").close()
        os.makedirs(dst)  # recreated by the desktop between rm and cp
        onat.sh(f"cp -aT '{src}' '{dst}'")
        self.assertTrue(os.path.exists(os.path.join(dst, "a.txt")))
        self.assertFalse(os.path.exists(os.path.join(dst, "Desktop")))


if __name__ == "__main__":
    unittest.main()


class ContextHygieneTests(unittest.TestCase):
    MSGS = [
        {"role": "user", "content": [{"type": "text", "text": "task"}]},
        {"role": "toolResult", "toolName": "mcp__gui__get_window_state", "content": [
            {"type": "text", "text": 'window_id=1 pid=2 snapshot_id=s00000001\n- frame = "Doc.docx - LibreOffice Writer"\n  - [0] button "A"\n  - [1] button "B"'},
            {"type": "image", "data": "x", "mimeType": "image/jpeg"}]},
        {"role": "toolResult", "toolName": "mcp__gui__click", "content": [
            {"type": "text", "text": 'Clicked element [1].\n[gate] Window right after the action (snapshot s00000002; tokens ...)\n- frame = "Doc.docx - LibreOffice Writer"\n  - [0] button "A"'}]},
        {"role": "toolResult", "toolName": "mcp__gui__run_command", "content": [{"type": "text", "text": "[run_command] exit 0\nout"}]},
        {"role": "toolResult", "toolName": "mcp__gui__get_window_state", "content": [
            {"type": "text", "text": 'window_id=1 pid=2 snapshot_id=s00000003\n- frame = "Doc.docx - LibreOffice Writer"\n  - [0] button "A"'}]},
    ]

    def test_prune_rule(self):
        import context_hygiene as ch
        out = ch.prune(self.MSGS)
        self.assertEqual(out[0], self.MSGS[0])
        self.assertEqual(out[4], self.MSGS[4])  # the newest observation stays whole
        self.assertEqual(out[3], self.MSGS[3])  # command output is not an observation
        self.assertEqual(len(out[1]["content"]), 1)  # the old screenshot is dropped
        self.assertIn('older window read summarised - "Doc.docx - LibreOffice Writer", snapshot s00000001, 2 rows', out[1]["content"][0]["text"])
        self.assertTrue(out[2]["content"][0]["text"].startswith("Clicked element [1].\n[client: older window read summarised"))

    def test_pi_extension_matches_python(self):
        import context_hygiene as ch
        import shutil
        if not shutil.which("node"):
            self.skipTest("node not installed")
        d = tempfile.mkdtemp()
        mjs = os.path.join(d, "obs_prune.mjs")
        shutil.copy(os.path.join(ROOT, "bench", "pi_ext", "obs_prune.ts"), mjs)
        js = f"import {{prune}} from '{mjs}'; let s=''; process.stdin.on('data',d=>s+=d); process.stdin.on('end',()=>console.log(JSON.stringify(prune(JSON.parse(s)))));"
        r = subprocess.run(["node", "--input-type=module", "-e", js], input=json.dumps(self.MSGS), capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(json.loads(r.stdout), json.loads(json.dumps(ch.prune(self.MSGS))))


class BenchOptionsTests(unittest.TestCase):
    def test_app_hints_cover_the_tasks_and_name_no_tools(self):
        import glob
        import app_hints
        for f in glob.glob(os.path.join(ROOT, "bench", "tasks", "*", "*.json")):
            name = os.path.basename(f)[:-5]
            h = app_hints.hint_for(name)
            self.assertTrue(h, name)
            self.assertLess(len(h), 900, name)
            for tool in ("run_command", "check_done", "set_plan", "run_actions"):
                self.assertNotIn(tool, h)  # the hints are the same with and without the 0.9.6 tools
        self.assertNotIn("CMYK", app_hints.hint_for("ow17-gimp-045bf3ff"))  # no task-specific answers
        self.assertEqual(app_hints.hint_for("unknown-task"), "")

    def test_run_bench_options(self):
        sh = open(os.path.join(ROOT, "bench", "run_bench.sh")).read()
        self.assertEqual(subprocess.run(["bash", "-n", os.path.join(ROOT, "bench", "run_bench.sh")]).returncode, 0)
        self.assertIn("MAXT=${MAXT_ARG:-${PI_MAX_TURNS:-30}}", sh)  # default stays 30 (comparability)
        self.assertIn("official setting is 100", sh)
        for t in ("run_command", "check_done", "set_plan"):
            self.assertIn(f'"{t}":"direct"', sh)  # Pi declares the new tools
        # the option loop: run it in isolation
        loop = sh[sh.index("MAXT_ARG=\"\""):sh.index("OUT=${1:-")]
        r = subprocess.run(["bash", "-c", loop + 'echo "$MAXT_ARG|$1"', "x", "--max-turns", "100", "/out"], capture_output=True, text=True)
        self.assertEqual(r.stdout.strip(), "100|/out")
