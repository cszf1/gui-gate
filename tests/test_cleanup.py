"""Upgrade hygiene: after `gui-gate cleanup` nothing an older version left behind remains (fake HOME)."""
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from test_gate import BIN  # noqa: E402


@unittest.skipIf(os.name == "nt", "unix layout")
class CleanupTests(unittest.TestCase):
    def setUp(self):
        if not BIN:
            raise unittest.SkipTest("build the gate first")
        self.h = tempfile.mkdtemp()
        h = self.h
        self.bin = os.path.join(h, ".local", "bin")
        os.makedirs(self.bin)
        self.me = os.path.join(self.bin, "gui-gate")
        shutil.copy(BIN, self.me)
        open(os.path.join(self.bin, ".gui-gate.new"), "w").close()               # interrupted swap
        # an old release copy in another dir on PATH
        self.olddir = os.path.join(h, "oldbin")
        os.makedirs(self.olddir)
        self.old = os.path.join(self.olddir, "gui-gate")
        with open(self.old, "w") as f:
            f.write("#!/bin/sh\necho 0.6.1\n")
        os.chmod(self.old, 0o755)
        # an unrelated tool with the same dir must stay
        self.other = os.path.join(self.olddir, "other-tool")
        open(self.other, "w").close()
        # shell profile: two duplicate blocks + one stale
        with open(os.path.join(h, ".bashrc"), "w") as f:
            f.write(f"alias ll='ls -l'\n\n# gui-gate\nexport PATH=\"{self.bin}:$PATH\"\n\n# gui-gate\nexport PATH=\"{self.bin}:$PATH\"\n"
                    f"\n# gui-gate\nexport PATH=\"/nowhere/bin:$PATH\"\n")
        # MCP configs pointing at the old copy, with removed flags, + our own .bak
        os.makedirs(os.path.join(h, ".pi", "agent"))
        cfg = {"mcpServers": {"gui": {"command": self.old, "args": ["--policy", "gate", "--decider", "model"]},
                              "web": {"command": "npx", "args": ["-y", "web"]}}}
        self.pi = os.path.join(h, ".pi", "agent", "mcp.json")
        json.dump(cfg, open(self.pi, "w"))
        json.dump({"mcpServers": {"web": {"command": "npx", "args": ["-y", "web"]}}}, open(self.pi + ".bak", "w"))
        os.makedirs(os.path.join(h, ".codex"))
        self.codex = os.path.join(h, ".codex", "config.toml")
        open(self.codex, "w").write(f'model = "o3"\n\n[mcp_servers.gui]\ncommand = "{self.old}"\nargs = ["--decider", "model"]\n')
        open(self.codex + ".bak", "w").write('model = "o3"\n')
        # user's own unrelated backup must stay
        self.userbak = os.path.join(h, ".cursor", "mcp.json.bak")
        os.makedirs(os.path.dirname(self.userbak))
        json.dump({"theme": "mine"}, open(self.userbak, "w"))
        json.dump({"theme": "changed", "mcpServers": {}}, open(os.path.join(h, ".cursor", "mcp.json"), "w"))
        # caches and logs
        c = os.path.join(h, ".cache", "gui-gate")
        os.makedirs(c)
        open(os.path.join(c, "driver-latest.json"), "w").write("{}")
        open(os.path.join(c, "pypi-cua-driver.json"), "w").write("{}")
        logs = os.path.join(h, ".gui-gate", "logs")
        os.makedirs(logs)
        self.oldlog, self.newlog = os.path.join(logs, "old.jsonl"), os.path.join(logs, "new.jsonl")
        for p in (self.oldlog, self.newlog):
            open(p, "w").write("{}\n")
        t = time.time() - 40 * 86400
        os.utime(self.oldlog, (t, t))
        self.env = {"HOME": h, "PATH": f"{self.bin}:{self.olddir}:/usr/bin:/bin", "XDG_CACHE_HOME": os.path.join(h, ".cache"),
                    "TMPDIR": h}

    def tearDown(self):
        shutil.rmtree(self.h, ignore_errors=True)

    def run_gate(self, *a):
        p = subprocess.run([self.me, *a], env=self.env, capture_output=True, text=True, timeout=60)
        return p.returncode, json.loads(p.stdout) if p.stdout.strip().startswith("{") else p.stdout

    def test_cleanup_leaves_nothing_old(self):
        code, rep = self.run_gate("cleanup")
        self.assertEqual(code, 0, rep)
        self.assertFalse(os.path.exists(os.path.join(self.bin, ".gui-gate.new")))
        self.assertFalse(os.path.exists(self.old))
        self.assertTrue(os.path.exists(self.other))                 # not ours
        rc = open(os.path.join(self.h, ".bashrc")).read()
        self.assertEqual(rc.count("# gui-gate"), 1)
        self.assertNotIn("/nowhere/bin", rc)
        self.assertIn("alias ll", rc)
        pi = json.load(open(self.pi))
        self.assertEqual(pi["mcpServers"]["gui"], {"command": self.me, "args": ["--policy", "gate"]})
        self.assertEqual(pi["mcpServers"]["web"], {"command": "npx", "args": ["-y", "web"]})
        self.assertFalse(os.path.exists(self.pi + ".bak"))
        cx = open(self.codex).read()
        self.assertIn(f'command = "{self.me}"', cx)
        self.assertNotIn("--decider", cx)
        self.assertFalse(os.path.exists(self.codex + ".bak"))
        self.assertTrue(os.path.exists(self.userbak))               # differs by more than our entry
        self.assertEqual(os.listdir(os.path.join(self.h, ".cache", "gui-gate")), ["driver-latest.json"])
        self.assertFalse(os.path.exists(self.oldlog))
        self.assertTrue(os.path.exists(self.newlog))
        # idempotent: a second run has nothing left to do
        code, rep2 = self.run_gate("cleanup", "--quiet")
        self.assertEqual(rep2, "", rep2)

    def test_dry_run_changes_nothing(self):
        before = {p: open(p).read() for p in (self.pi, self.codex, os.path.join(self.h, ".bashrc"))}
        code, rep = self.run_gate("cleanup", "--dry-run")
        self.assertTrue(all(x.startswith("would ") for x in rep["cleaned"]), rep)
        self.assertTrue(os.path.exists(self.old))
        self.assertEqual(before, {p: open(p).read() for p in before})

    def test_uninstall_removes_everything_of_ours(self):
        code, rep = self.run_gate("uninstall")
        self.assertEqual(code, 0, rep)
        self.assertFalse(os.path.exists(self.me))
        self.assertNotIn("gui", json.load(open(self.pi))["mcpServers"])
        self.assertIn("web", json.load(open(self.pi))["mcpServers"])
        cx = open(self.codex).read()
        self.assertNotIn("gui-gate", cx)
        self.assertIn('model = "o3"', cx)
        self.assertFalse(os.path.exists(os.path.join(self.h, ".cache", "gui-gate")))
        self.assertFalse(os.path.exists(os.path.join(self.h, ".gui-gate")))
        self.assertNotIn("# gui-gate", open(os.path.join(self.h, ".bashrc")).read())
        self.assertTrue(os.path.exists(self.other))


if __name__ == "__main__":
    unittest.main()
