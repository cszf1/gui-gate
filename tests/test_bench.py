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
