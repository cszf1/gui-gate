"""Packaging: driver discovery, client configuration, doctor (against the fake driver), --mode single."""
import json
import os
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
sys.path.insert(0, HERE)
from gui_gate import install  # noqa: E402
from test_gate import Client  # noqa: E402


class InstallTests(unittest.TestCase):
    def test_env_driver_wins(self):
        os.environ["GUI_GATE_DRIVER"] = "/x/cua-driver"
        try:
            self.assertEqual(install.find_driver(), "/x/cua-driver")
        finally:
            del os.environ["GUI_GATE_DRIVER"]

    def test_cursor_config_merges(self):
        with tempfile.TemporaryDirectory() as h:
            os.makedirs(os.path.join(h, ".cursor"))
            with open(os.path.join(h, ".cursor", "mcp.json"), "w") as f:
                json.dump({"mcpServers": {"other": {"command": "x"}}}, f)
            install.configure("cursor", home=h)
            d = json.load(open(os.path.join(h, ".cursor", "mcp.json")))
            self.assertIn("other", d["mcpServers"])
            self.assertIn("gui", d["mcpServers"])
            self.assertTrue(os.path.exists(os.path.join(h, ".cursor", "mcp.json.bak")))

    def test_codex_config_once(self):
        with tempfile.TemporaryDirectory() as h:
            install.configure("codex", home=h)
            self.assertIn("already", install.configure("codex", home=h))
            self.assertEqual(open(os.path.join(h, ".codex", "config.toml")).read().count("[mcp_servers.gui]"), 1)

    def test_doctor_end_to_end_with_fake_driver(self):
        rep = install.doctor(cmd=[sys.executable, os.path.join(HERE, "fake_driver.py")])
        self.assertTrue(rep["ok"], rep)
        self.assertGreater(rep["driver_tools"], 5)

    def test_single_mode_hides_batch(self):
        c = Client("--mode", "single")
        try:
            names = {t["name"] for t in c.rpc("tools/list", {})["tools"]}
            self.assertNotIn("run_actions", names)
            self.assertIn("click", names)
        finally:
            c.close()


if __name__ == "__main__":
    unittest.main()
