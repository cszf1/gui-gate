"""Packaging and updates, against the built binary: client configuration, doctor, --mode single, update notice."""
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from test_gate import BIN, FAKE, Client  # noqa: E402


def run(*args, home=None):
    env = dict(os.environ, GUI_GATE_NO_UPDATE_CHECK="1", GUI_GATE_DRIVER="/nonexistent/cua-driver")
    if home:
        env["HOME"] = home
    return subprocess.run([BIN, *args], capture_output=True, text=True, env=env, timeout=60)


@unittest.skipUnless(BIN, "build the gate first: cargo build")
class InstallTests(unittest.TestCase):
    def test_cursor_config_merges(self):
        with tempfile.TemporaryDirectory() as h:
            os.makedirs(os.path.join(h, ".cursor"))
            with open(os.path.join(h, ".cursor", "mcp.json"), "w") as f:
                json.dump({"mcpServers": {"other": {"command": "x"}}}, f)
            run("setup", "--client", "cursor", home=h)
            d = json.load(open(os.path.join(h, ".cursor", "mcp.json")))
            self.assertIn("other", d["mcpServers"])
            self.assertIn("gui", d["mcpServers"])
            self.assertFalse(os.path.exists(os.path.join(h, ".cursor", "mcp.json.bak")))  # 0.9: atomic write, no leftover backup
            self.assertFalse(any(n.endswith("gui-gate-tmp") for n in os.listdir(os.path.join(h, ".cursor"))))

    def test_codex_config_once(self):
        with tempfile.TemporaryDirectory() as h:
            run("setup", "--client", "codex", home=h)
            self.assertIn("already", run("setup", "--client", "codex", home=h).stdout)
            self.assertEqual(open(os.path.join(h, ".codex", "config.toml")).read().count("[mcp_servers.gui]"), 1)

    def test_doctor_end_to_end_with_fake_driver(self):
        r = run("doctor", "--child", FAKE)
        rep = json.loads(r.stdout)
        self.assertTrue(rep["ok"], rep)
        self.assertGreater(rep["driver_tools"], 5)
        self.assertEqual(rep["missing_tools"], [])

    def test_doctor_reports_missing_driver(self):
        r = run("doctor")
        self.assertNotEqual(r.returncode, 0)

    def test_single_mode_hides_batch(self):
        c = Client("--mode", "single")
        try:
            names = {t["name"] for t in c.rpc("tools/list", {})["tools"]}
            self.assertNotIn("run_actions", names)
            self.assertIn("click", names)
        finally:
            c.close()

    def test_update_check_offline(self):
        r = subprocess.run([BIN, "update", "--check"], capture_output=True, text=True, timeout=60,
                           env=dict(os.environ, GUI_GATE_LATEST_OVERRIDE="0.37.0", GUI_GATE_DRIVER="/nonexistent"))
        self.assertIn('"tested_with": "0.', r.stdout)
        self.assertIn('"latest": "0.37.0"', r.stdout)

    def test_server_tells_client_about_new_driver(self):
        c = Client(env={"GUI_GATE_LATEST_OVERRIDE": "0.99.0", "GUI_GATE_NO_UPDATE_CHECK": ""})
        try:
            time.sleep(2.5)
            c.rpc("tools/list", {})
            seen = json.dumps(c.init) + json.dumps(c.extra)
            self.assertIn("Cua Driver 0.99.0 is available", seen)
        finally:
            c.close()


if __name__ == "__main__":
    unittest.main()
