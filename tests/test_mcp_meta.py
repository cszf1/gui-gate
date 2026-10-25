"""MCP 2026-07-28 (per-request `_meta`) clients against a fake driver that enforces cua-driver 0.38's
ProtocolSession rules (metadata-free requests are rejected before a legacy initialize)."""
import json
import os
import unittest

from test_gate import BIN, Client, FAKE, FAKE_REVIEWER, ROOT, txt
import subprocess
import tempfile
os.environ.setdefault("GUI_GATE_CHILD_ENV", "FAKE_*,PYTHONPATH")  # the fake driver's settings reach it through the allowlisted child env

MODERN = "2026-07-28"
PV, CC = "io.modelcontextprotocol/protocolVersion", "io.modelcontextprotocol/clientCapabilities"
SI = "io.modelcontextprotocol/serverInfo"


def meta(version=MODERN, caps=None):
    return {PV: version, CC: caps if caps is not None else {}}


class ModernClient:
    """No initialize at all: every request carries the protocol version and client capabilities in params._meta."""

    def __init__(self, env=None):
        if not BIN:
            raise unittest.SkipTest("build the gate first: cargo build")
        self.log = tempfile.NamedTemporaryFile(suffix=".jsonl", delete=False).name
        e = dict(os.environ, PYTHONPATH=ROOT)
        e.update({k: v for k, v in FAKE_REVIEWER.items() if not os.environ.get(k)})
        e.update(env or {})
        self.p = subprocess.Popen([BIN, "--child", FAKE, "--log", self.log], stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, text=True, env=e, cwd=ROOT)
        self.i = 0

    def raw(self, method, params, m=None):
        self.i += 1
        params = dict(params)
        params["_meta"] = meta() if m is None else m
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.i, "method": method, "params": params}) + "\n")
        self.p.stdin.flush()
        while True:
            msg = json.loads(self.p.stdout.readline())
            if msg.get("id") == self.i and "method" not in msg:
                return msg

    def rpc(self, method, params, m=None):
        msg = self.raw(method, params, m)
        assert "result" in msg, msg
        return msg["result"]

    def call(self, name, **args):
        return self.rpc("tools/call", {"name": name, "arguments": args})

    def read(self):
        return self.call("get_window_state", pid=4242, window_id=777)["structuredContent"]["snapshot_id"]

    def close(self):
        self.p.stdin.close()
        self.p.wait(timeout=5)


class TestModernClient(unittest.TestCase):
    def setUp(self):
        self.c = ModernClient()

    def tearDown(self):
        self.c.close()

    def assertEnvelope(self, r):
        self.assertEqual(r.get("resultType"), "complete")
        self.assertTrue(r["_meta"][SI]["name"].startswith("gui-gate"), r["_meta"])

    def test_discover_is_the_gates(self):
        r = self.c.rpc("server/discover", {})
        self.assertEqual(r["supportedVersions"], [MODERN])
        self.assertIn("[gui-gate]", r["instructions"])
        self.assertTrue(r["instructions"].startswith("fake"))
        self.assertIn("tools", r["capabilities"])
        self.assertEnvelope(r)
        self.assertEqual(r["ttlMs"], 0)
        self.assertNotIn("serverInfo", r)

    def test_tools_list_and_actions_work_without_initialize(self):
        tl = self.c.rpc("tools/list", {})
        names = [t["name"] for t in tl["tools"]]
        self.assertIn("gate_report", names)  # the gate's augmentation also applies to modern requests
        self.assertIn("click", names)
        self.assertEnvelope(tl)
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:0", pid=4242, window_id=777)
        self.assertFalse(r.get("isError"), txt(r))
        self.assertIn("Clicked", txt(r))
        self.assertEnvelope(r)
        st = json.loads(txt(self.c.call("fake_state")))
        self.assertIn("Increment", st["clicked"])
        self.assertEqual(st["inits"], 1)  # the gate's own handshake, exactly once
        # the driver only ever sees legacy requests from the gate
        self.assertFalse(any(isinstance(x["meta"], dict) and (PV in x["meta"] or CC in x["meta"]) for x in st["seen"]))

    def test_gate_synthesized_results_have_the_envelope(self):
        r = self.c.call("gate_report")
        self.assertEnvelope(r)
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:1", pid=4242, window_id=777)  # "Delete file": the gate stops it
        self.assertIn("[gate]", txt(r))
        self.assertEnvelope(r)

    def test_version_mismatch_and_bad_meta(self):
        m = self.c.raw("tools/list", {}, m=meta(version="2099-01-01"))
        self.assertEqual(m["error"]["code"], -32022)
        self.assertEqual(m["error"]["data"], {"supported": [MODERN], "requested": "2099-01-01"})
        m = self.c.raw("tools/call", {"name": "gate_report", "arguments": {}}, m={PV: MODERN})
        self.assertEqual(m["error"]["code"], -32602)
        m = self.c.raw("server/discover", {}, m={})
        self.assertEqual(m["error"]["code"], -32602)
        # the session still works afterwards
        self.assertEqual(self.c.rpc("server/discover", {})["supportedVersions"], [MODERN])

    def test_driver_respawn(self):
        self.c.call("fake_crash")
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:0", pid=4242, window_id=777)
        self.assertIn("Clicked", txt(r))
        self.assertEnvelope(r)
        self.assertIn("click", [t["name"] for t in self.c.rpc("tools/list", {})["tools"]])
        self.assertIn("[gui-gate]", self.c.rpc("server/discover", {})["instructions"])


class TestLegacyClientWithStrictDriver(unittest.TestCase):
    def setUp(self):
        self.c = Client()

    def tearDown(self):
        self.c.close()

    def test_initialize_answered_by_gate_and_no_double_init(self):
        self.assertIn("[gui-gate]", self.c.init["instructions"])
        self.assertTrue(self.c.init["serverInfo"]["name"].startswith("gui-gate (cua-driver 0.0)"))
        self.assertEqual(self.c.init["protocolVersion"], "2025-06-18")
        self.assertNotIn("resultType", self.c.init)
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:0", pid=4242, window_id=777)
        self.assertIn("Clicked", txt(r))
        self.assertNotIn("resultType", r)
        st = json.loads(txt(self.c.call("fake_state")))
        self.assertEqual(st["inits"], 1)

    def test_driver_respawn(self):
        self.c.call("fake_crash")
        s = self.c.read()
        r = self.c.call("click", element_token=f"{s}:0", pid=4242, window_id=777)
        self.assertIn("Clicked", txt(r))
        self.assertIn("click", [t["name"] for t in self.c.rpc("tools/list", {})["tools"]])


class TestOldDriver(unittest.TestCase):
    """A driver from before 0.38 (no server/discover, no per-request rules): the gate still serves modern clients."""

    def test_modern_client_old_driver(self):
        c = ModernClient(env={"FAKE_PRE_038": "1"})
        try:
            self.assertEqual(c.rpc("server/discover", {})["supportedVersions"], [MODERN])
            s = c.read()
            r = c.call("click", element_token=f"{s}:0", pid=4242, window_id=777)
            self.assertIn("Clicked", txt(r))
            self.assertEqual(r["resultType"], "complete")
        finally:
            c.close()


if __name__ == "__main__":
    unittest.main()
