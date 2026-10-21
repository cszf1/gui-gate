"""Packaging: driver discovery, client configuration, doctor (against the fake driver), --mode single."""
import json
import os
import sys
import tempfile
import time
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


class UpdateTests(unittest.TestCase):
    def setUp(self):
        from gui_gate import updates
        self.u = updates

    def test_version_order(self):
        v = self.u.vtuple
        self.assertGreater(v("0.37.10"), v("0.37.9"))
        self.assertGreater(v("0.38.0"), v("0.37.0"))
        self.assertEqual(v("v0.37.0"), (0, 37, 0))
        self.assertEqual(v("0.38.0rc1")[:2], (0, 38))

    def test_no_network_when_disabled(self):
        os.environ.pop("GUI_GATE_LATEST_OVERRIDE", None)
        os.environ["GUI_GATE_NO_UPDATE_CHECK"] = "1"
        self.assertIsNone(self.u.latest())

    def test_note_when_newer(self):
        st = {"installed": "0.37.0", "latest": "0.38.1", "tested_with": "0.37.0",
              "update_available": True, "untested": False}
        self.assertIn("gui-gate update", self.u.note(st))
        st2 = dict(st, latest="0.37.0", update_available=False)
        self.assertEqual(self.u.note(st2), "")
        st3 = dict(st2, installed="0.39.0", untested=True)
        self.assertIn("--to 0.37.0", self.u.note(st3))

    def test_missing_tools(self):
        self.assertEqual(self.u.missing_tools(["click", "type_text"]),
                         ["get_window_state", "list_windows", "press_key"])

    def test_server_tells_client_about_new_driver(self):
        os.environ["GUI_GATE_LATEST_OVERRIDE"] = "0.99.0"
        try:
            c = Client(env={"GUI_GATE_LATEST_OVERRIDE": "0.99.0", "GUI_GATE_NO_UPDATE_CHECK": ""})
            try:
                time.sleep(2.5)
                c.rpc("tools/list", {})
                seen = json.dumps(c.init) + json.dumps(c.extra)
                self.assertIn("Cua Driver 0.99.0 is available", seen)
            finally:
                c.close()
        finally:
            del os.environ["GUI_GATE_LATEST_OVERRIDE"]


if __name__ == "__main__":
    unittest.main()
