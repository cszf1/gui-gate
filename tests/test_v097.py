"""0.9.7: features absorbed from the top OSWorld-Verified frameworks (docs/sota-absorb-2026-10-22.md).
Everything is off by default; GUI_GATE_097=1 turns on the model-free features; model roles talk to a local fake
endpoint here (no paid API is ever called by the tests)."""
import http.server
import json
import os
import tempfile
import threading
import time
import unittest

from test_gate import Client, txt

FLAGS_OFF = {k: "" for k in ["GUI_GATE_097", "GUI_GATE_INSPECT", "GUI_GATE_JOBS", "GUI_GATE_PHANTOM", "GUI_GATE_TWO_STRIKES",
                             "GUI_GATE_FEASIBILITY_FIRST", "GUI_GATE_STATE_PLAN", "GUI_GATE_VERIFY_PLUS", "GUI_GATE_SKILLS",
                             "GUI_GATE_SNAP", "GUI_GATE_PICK", "GUI_GATE_DIGEST", "GUI_GATE_MICRO_VERIFY", "GUI_GATE_MEMORY"]}
for _k in list(os.environ):
    if _k.startswith("GUI_GATE_ROLE_") or _k in FLAGS_OFF:
        del os.environ[_k]

# the advertised tool list of the default (core) toolset, against the fake driver (it has no list_apps, scroll, ...)
DEFAULT_TOOLS = ["get_window_state", "click", "type_text", "press_key", "hotkey", "list_windows", "launch_app", "bring_to_front",
                 "request_human", "gate_task", "gate_report", "run_command", "check_done", "set_plan", "run_actions"]


class FakeLLM:
    """OpenAI-compatible endpoint. `script(system, user) -> reply text`; records every request."""
    def __init__(self, script):
        seen = self.seen = []

        class H(http.server.BaseHTTPRequestHandler):
            def do_POST(self):
                body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                seen.append(body)
                msgs = body.get("messages", [])
                system = next((m["content"] for m in msgs if m["role"] == "system"), "")
                user = next((m["content"] for m in msgs if m["role"] == "user"), "")
                out = json.dumps({"choices": [{"message": {"content": script(system, user)}}]}).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(out)

            def log_message(self, *a):
                pass
        self.srv = http.server.HTTPServer(("127.0.0.1", 0), H)
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()
        self.url = f"http://127.0.0.1:{self.srv.server_port}/v1/chat/completions"

    def env(self, *roles):
        e = {}
        for r in roles:
            R = r.upper()
            e.update({f"GUI_GATE_ROLE_{R}_URL": self.url, f"GUI_GATE_ROLE_{R}_MODEL": f"fake-{r}", f"GUI_GATE_ROLE_{R}_KEY": "k",
                      f"GUI_GATE_ROLE_{R}_TIMEOUT": "5"})
        return e

    def close(self):
        self.srv.shutdown()


def names(c):
    return [t["name"] for t in c.rpc("tools/list", {})["tools"]]


class ToolListTests(unittest.TestCase):
    def test_default_tool_list_is_unchanged(self):
        c = Client()
        try:
            self.assertEqual(names(c), DEFAULT_TOOLS)
            self.assertNotIn("0.9.7", c.init["instructions"])
            rc = next(t for t in c.rpc("tools/list", {})["tools"] if t["name"] == "run_command")
            self.assertNotIn("mode", rc["inputSchema"]["properties"])
        finally:
            c.close()

    def test_all_flags_add_one_tool_and_modes(self):
        c = Client(env={"GUI_GATE_097": "1"})
        try:
            tools = c.rpc("tools/list", {})["tools"]
            self.assertEqual([t["name"] for t in tools], DEFAULT_TOOLS + ["inspect"])
            self.assertEqual(len(tools), len(DEFAULT_TOOLS) + 1)
            rc = next(t for t in tools if t["name"] == "run_command")
            self.assertEqual(rc["inputSchema"]["properties"]["mode"]["enum"], ["run", "start", "poll", "stop"])
            ins = next(t for t in tools if t["name"] == "inspect")
            self.assertEqual(ins["inputSchema"]["properties"]["kind"]["enum"], ["file", "env", "page", "region", "locate", "howto", "memory"])
            self.assertTrue(ins["annotations"]["readOnlyHint"])
            self.assertIn("[gui-gate 0.9.7]", c.init["instructions"])
            self.assertIn("Two failed attempts", c.init["instructions"])
        finally:
            c.close()

    def test_single_flag_and_opt_out_inside_combined(self):
        c = Client(env={"GUI_GATE_INSPECT": "1"})
        try:
            self.assertEqual(names(c), DEFAULT_TOOLS + ["inspect"])
        finally:
            c.close()
        c = Client(env={"GUI_GATE_097": "1", "GUI_GATE_INSPECT": "0", "GUI_GATE_RUN_COMMAND": "0", "GUI_GATE_CHECK_DONE": "0", "GUI_GATE_PLAN": "0"})
        try:
            self.assertEqual(names(c), [n for n in DEFAULT_TOOLS if n not in ("run_command", "check_done", "set_plan")])
        finally:
            c.close()


class PhantomTests(unittest.TestCase):
    def setUp(self):
        self.c = Client("--human", "deny", env={"GUI_GATE_PHANTOM": "1"})
        self.c.rpc("tools/list", {})

    def tearDown(self):
        self.c.close()

    def test_known_aliases_run_the_real_tool(self):
        s = self.c.read()
        r = self.c.call("left_click_safe", element_token=f"{s}:0")
        self.assertIn('Clicked element [0] button "Increment"', txt(r))
        self.assertIn('[gate] there is no tool "left_click_safe"; ran click', txt(r))
        r = self.c.call("key", pid=4242, keys="Return")
        self.assertIn("press_key", txt(r))
        st = json.loads(txt(self.c.call("fake_state")))
        self.assertIn("Return", st["keys"])
        r = self.c.call("bash", cmd="echo phantom-$((6*7))")
        self.assertIn("phantom-42", txt(r))
        ev = [e for e in self.c.events() if e.get("ev") == "phantom"]
        self.assertEqual([e["to"] for e in ev], ["click", "press_key", "run_command"])

    def test_triple_click_is_emulated(self):
        s = self.c.read()
        r = self.c.call("triple_click_workaround", pid=4242, window_id=777, element_token=f"{s}:0")
        self.assertIn("emulated it: click, then Home and shift+End", txt(r))
        st = json.loads(txt(self.c.call("fake_state")))
        self.assertIn("Increment", st["clicked"])
        self.assertIn("Home", st["keys"])

    def test_unknown_names_get_nearest_tools_and_final_hint(self):
        r = self.c.call("frobnicate_window")
        self.assertTrue(r.get("isError"))
        self.assertIn("do not try variants of the name", txt(r))
        r = self.c.call("done")
        self.assertIn("finish by ending your turn", txt(r))
        t0 = time.time()
        r = self.c.call("wait", seconds=0.3)
        self.assertGreaterEqual(time.time() - t0, 0.25)
        self.assertIn("waited 0.3 s", txt(r))

    def test_off_by_default(self):
        c = Client()
        try:
            c.rpc("tools/list", {})
            r = c.call("left_click", element_token="x")
            self.assertNotIn("there is no tool", txt(r))
        finally:
            c.close()


class InspectTests(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp()
        self.c = Client("--human", "deny", "--task", f"Read {self.d}/notes.txt and put the total into \"zz-missing.xlsx\" with LibreOffice Calc",
                        env={"GUI_GATE_INSPECT": "1"})

    def tearDown(self):
        self.c.close()

    def test_file_env_locate_howto_memory(self):
        p = os.path.join(self.d, "notes.txt")
        open(p, "w").write("total: 42\n")
        r = self.c.call("inspect", kind="file", path=p)
        self.assertIn("total: 42", txt(r))
        self.assertTrue(self.c.call("inspect", kind="file", path=p + ".nope").get("isError"))
        r = self.c.call("inspect", kind="env")
        self.assertIn(f'path "{self.d}/notes.txt": exists', txt(r))
        self.assertIn('"zz-missing.xlsx": NOT found', txt(r))
        self.assertIn("hardware: battery", txt(r))
        self.c.read()
        r = self.c.call("inspect", kind="locate", pid=4242, window_id=777, describe="the Delete file button")
        self.assertIn('[1] button "Delete file"', txt(r).splitlines()[1])
        r = self.c.call("inspect", kind="howto", query="pivot table in calc")
        self.assertIn("libreoffice_calc", txt(r))
        r = self.c.call("inspect", kind="memory")
        self.assertIn("no experience memory is configured", txt(r))
        r = self.c.call("inspect", kind="page")
        self.assertIn("[inspect page]", txt(r))  # no Chrome here: a clear message, not a crash
        self.assertTrue(any(e.get("ev") == "inspect" for e in self.c.events()))

    def test_image_file_is_returned_natively(self):
        import base64
        import fake_driver
        p = os.path.join(self.d, "pic.png")
        open(p, "wb").write(fake_driver.real_png(40, 20))
        r = self.c.call("inspect", kind="file", path=p)
        self.assertIn("image 40x20", txt(r))
        self.assertEqual([c["type"] for c in r["content"]], ["text", "image"])
        self.assertIn(base64.b64decode(r["content"][1]["data"])[:4], (b"\x89PNG", b"\xff\xd8\xff\xe0"))  # PNG or the gate's JPEG re-encode


class HowtoRoleTests(unittest.TestCase):
    def test_search_role_answers_howto(self):
        llm = FakeLLM(lambda sys, user: json.dumps({"final": {"answer": "Slide > Slide Properties > Background", "exists": True}}))
        c = Client(env={"GUI_GATE_INSPECT": "1", **llm.env("search")})
        try:
            r = c.call("inspect", kind="howto", query="slide background colour in Impress")
            self.assertIn("Slide > Slide Properties > Background", txt(r))
            self.assertIn("fake-search", txt(r))
            self.assertIn("practical tutorial", llm.seen[0]["messages"][0]["content"])
            rep = c.call("gate_report")
            self.assertIn("search=api:chat:fake-search", txt(rep))
        finally:
            c.close()
            llm.close()


class JobTests(unittest.TestCase):
    def test_start_poll_stop(self):
        c = Client("--human", "deny", env={"GUI_GATE_JOBS": "1"})
        try:
            r = c.call("run_command", command="echo start; sleep 0.4; echo finished-$((2*21))", mode="start")
            job = r["structuredContent"]["job"]
            self.assertIn("started job", txt(r))
            time.sleep(0.9)
            r = c.call("run_command", mode="poll", job=job)
            self.assertIn("finished, exit 0", txt(r))
            self.assertIn("finished-42", txt(r))
            r = c.call("run_command", command="sleep 30", mode="start")
            r = c.call("run_command", mode="stop", job=r["structuredContent"]["job"])
            self.assertIn("stopped", txt(r))
        finally:
            c.close()

    def test_start_is_risk_checked(self):
        d = tempfile.mkdtemp()
        open(os.path.join(d, "keep.txt"), "w").write("x")
        c = Client("--human", "deny", env={"GUI_GATE_JOBS": "1"})
        try:
            r = c.call("run_command", command=f"rm -rf {d}", mode="start")
            self.assertIn("[gate]", txt(r))
            self.assertNotIn("started job", txt(r))
            time.sleep(0.3)
            self.assertTrue(os.path.exists(os.path.join(d, "keep.txt")))
        finally:
            c.close()


if __name__ == "__main__":
    unittest.main()


class PlanTests(unittest.TestCase):
    def test_state_milestones_checks_facts(self):
        d = tempfile.mkdtemp()
        c = Client("--human", "deny", env={"GUI_GATE_STATE_PLAN": "1"})
        try:
            r = c.call("set_plan", feasible=True, milestones=["Click the Data menu, then Sort", {"state": f"{d}/out.txt holds the total", "check": {"file_contains": [f"{d}/out.txt", "42"]}}],
                       facts={"total": "42"})
            t = txt(r)
            self.assertIn("milestone(s) 1 read like clicks", t)
            self.assertIn("[check: not yet:", t)
            self.assertIn("Facts: total=42", t)
            open(os.path.join(d, "out.txt"), "w").write("sum 42\n")
            r = c.call("set_plan", done=[1], checks=[{"n": 1, "check": {"file_exists": f"{d}/out.txt"}}])
            self.assertIn("1. [x]", txt(r))
            self.assertIn("[check: holds:", txt(r))
            r = c.call("set_plan", milestones=["Click the Data menu, then Sort", "new end state"], facts={"total": None})
            self.assertIn("1. [x]", txt(r))  # unchanged milestone keeps its done mark
            self.assertNotIn("Facts:", txt(r))
            ev = [e for e in c.events() if e.get("ev") == "plan"]
            self.assertEqual(ev[0]["checks"], 1)
        finally:
            c.close()

    def test_without_flag_set_plan_is_096(self):
        c = Client("--human", "deny")
        try:
            tools = {t["name"]: t for t in c.rpc("tools/list", {})["tools"]}
            self.assertNotIn("checks", tools["set_plan"]["inputSchema"]["properties"])
            r = c.call("set_plan", milestones=["Click the Data menu"])
            self.assertNotIn("read like clicks", txt(r))
        finally:
            c.close()


class FeasibilityFirstTests(unittest.TestCase):
    def test_first_action_needs_a_verdict_once(self):
        c = Client("--human", "deny", "--task", "Sort the rows of \"zz-nope.xlsx\" in LibreOffice Calc by date",
                   env={"GUI_GATE_FEASIBILITY_FIRST": "1", "GUI_GATE_SKILLS": "1"})
        try:
            r = c.call("click", element_token=f"{c.read()}:0")
            t = txt(r)
            self.assertIn("Nothing was executed. First decide whether this task can be done", t)
            st = json.loads(txt(c.call("fake_state")))
            self.assertEqual(st["clicked"], [])
            r = c.call("click", element_token=f"{c.read()}:0")
            self.assertIn("Clicked element [0]", txt(r))  # asked once only
            ev = [e for e in c.events() if e.get("ev") in ("verdict_required", "briefing")]
            self.assertEqual([e["ev"] for e in ev], ["briefing", "verdict_required"])
            self.assertIn("libreoffice_calc", ev[0]["skills"])
        finally:
            c.close()

    def test_briefing_text_and_set_plan_unlocks(self):
        c = Client("--human", "deny", "--task", "Sort the rows of \"zz-nope.xlsx\" in LibreOffice Calc by date",
                   env={"GUI_GATE_FEASIBILITY_FIRST": "1", "GUI_GATE_SKILLS": "1"})
        try:
            r = c.call("list_windows")
            t = txt(r)
            self.assertIn("[gate] Task briefing", t)
            self.assertIn('"zz-nope.xlsx": NOT found', t)
            self.assertIn("[libreoffice_calc]", t)
            self.assertIn("[environment]", t)
            self.assertIn("required before your first action", t)
            c.call("set_plan", feasible=True, milestones=["zz-nope.xlsx is sorted by date and saved"])
            r = c.call("click", element_token=f"{c.read()}:0")
            self.assertIn("Clicked element [0]", txt(r))
            self.assertNotIn("Task briefing", txt(r))  # once
        finally:
            c.close()

    def test_gate_task_gets_the_briefing(self):
        c = Client("--human", "deny", env={"GUI_GATE_SKILLS": "1"})
        try:
            r = c.call("gate_task", request="Change the Chrome font size to 20")
            self.assertIn("[gate] Noted.", txt(r))
            self.assertIn("[chrome]", txt(r))
        finally:
            c.close()


class RoleBriefingTests(unittest.TestCase):
    def test_feasibility_role_probes_read_only_and_reports(self):
        victim = tempfile.mkdtemp()
        open(os.path.join(victim, "keep.txt"), "w").write("x")
        calls = []

        def script(system, user):
            calls.append(user)
            if "Feasibility Gate" not in system:
                return "{}"
            if len(calls) == 1:
                return json.dumps({"probe": "sh", "command": f"rm -rf {victim}"})
            if len(calls) == 2:
                return json.dumps({"probe": "sh", "command": "ls /sys/class/power_supply 2>/dev/null; echo probed-ok"})
            return json.dumps({"final": {"feasible": False, "reason": "no battery on this machine", "evidence": ["/sys/class/power_supply is empty"], "notes": []}})
        llm = FakeLLM(script)
        c = Client("--human", "deny", "--task", "Show the battery percentage in the panel",
                   env={"GUI_GATE_FEASIBILITY_FIRST": "1", **llm.env("feasibility")})
        try:
            t = txt(c.call("list_windows"))
            self.assertIn("INFEASIBLE - no battery on this machine", t)
            self.assertIn("call set_plan with feasible:false", t)
            self.assertTrue(os.path.exists(os.path.join(victim, "keep.txt")))  # the destructive probe was refused
            self.assertIn("refused:", calls[1])
            self.assertIn("probed-ok", calls[2])
            self.assertIn("PROBES SO FAR", calls[2])
            b = next(e for e in c.events() if e.get("ev") == "briefing")
            self.assertEqual(b["feasibility"]["verdict"], False)
            self.assertEqual(b["feasibility"]["probes"], 2)
        finally:
            c.close()
            llm.close()

    def test_planner_role_seeds_the_plan(self):
        d = tempfile.mkdtemp()

        def script(system, user):
            return json.dumps({"final": {"milestones": [{"state": f"{d}/done.txt exists", "check": {"file_exists": f"{d}/done.txt"}}, "the app shows the result"], "infeasible_reason": ""}})
        llm = FakeLLM(script)
        c = Client("--human", "deny", "--task", f"Create {d}/done.txt", env={"GUI_GATE_STATE_PLAN": "1", **llm.env("planner")})
        try:
            t = txt(c.call("list_windows"))
            self.assertIn("## Plan (from the gate's planner", t)
            self.assertIn("(check {", t)
            r = c.call("run_command", command=f"touch {d}/done.txt")
            self.assertIn("[plan] 0/2 done; next: 1.", txt(r))
            r = c.call("set_plan", done=[1])
            self.assertIn("holds:", txt(r))
        finally:
            c.close()
            llm.close()

    def test_unreachable_role_is_harmless(self):
        c = Client("--human", "deny", "--task", "Do something", env={"GUI_GATE_FEASIBILITY_FIRST": "1", "GUI_GATE_ROLE_FEASIBILITY_URL": "http://127.0.0.1:9/v1",
                                                                     "GUI_GATE_ROLE_FEASIBILITY_MODEL": "x", "GUI_GATE_ROLE_FEASIBILITY_TIMEOUT": "1"})
        try:
            t = txt(c.call("list_windows"))
            self.assertIn("the feasibility role gave no verdict", t)
            self.assertIn("hardware:", t)
        finally:
            c.close()

    def test_experience_memory_in_briefing(self):
        f = tempfile.NamedTemporaryFile("w", suffix=".jsonl", delete=False)
        f.write(json.dumps({"task": "Make the Chrome font size bigger, 20", "steps": ["hotkey ctrl+l", "type_text chrome://settings/fonts"]}) + "\n")
        f.close()
        c = Client("--human", "deny", "--task", "Set the default font size in Chrome to 24", env={"GUI_GATE_MEMORY": f.name})
        try:
            t = txt(c.call("list_windows"))
            self.assertIn("## Experience", t)
            self.assertIn("chrome://settings/fonts", t)
        finally:
            c.close()
