"""0.9.7: features absorbed from the top OSWorld-Verified frameworks (docs/sota-absorb-2026-10-22.md).
Everything is off by default; GUI_GATE_097=1 turns on the model-free features; model roles talk to a local fake
endpoint here (no paid API is ever called by the tests)."""
import http.server
import json
import os
os.environ.setdefault("GUI_GATE_DIALOG", "none")  # tests answer person requests through the client (MRTR) or --human, never a real desktop dialog
import sys
import shutil
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
if os.name == "nt":  # run_command is bash-only: not offered on Windows
    DEFAULT_TOOLS.remove("run_command")
UNIX_ONLY = unittest.skipIf(os.name == "nt", "bash-only: run_command / terminal rules / jobs are not offered on Windows")


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
            if r in ("planner", "search", "compact"):
                e[f"GUI_GATE_ROLE_{R}_ENABLE"] = "1"  # 0.10.1: these three are off in the gate unless enabled
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
            if os.name != "nt":
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
            if os.name != "nt":
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
        ev = [e for e in self.c.events() if e.get("ev") == "phantom"]
        if os.name == "nt":  # no run_command on Windows: "bash" maps to nothing
            self.assertIn('there is no tool "bash"', txt(r))
            self.assertEqual([e.get("to") for e in ev], ["click", "press_key", None])
        else:
            self.assertIn("phantom-42", txt(r))
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


@UNIX_ONLY
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

    def _job_procs(self, mark):
        """pids of live processes whose command line carries `mark` (Linux /proc)."""
        out = []
        for d in os.listdir("/proc"):
            if not d.isdigit():
                continue
            try:
                cl = open(f"/proc/{d}/cmdline", "rb").read().replace(b"\0", b" ").decode(errors="replace")
                st = open(f"/proc/{d}/stat").read().rsplit(")", 1)[1].split()[0]
            except OSError:
                continue
            if mark in cl and st != "Z":
                out.append(int(d))
        return out

    @unittest.skipUnless(sys.platform.startswith("linux"), "/proc")
    def test_gate_exit_kills_jobs_and_leaves_no_temp(self):
        """0.9.7 hygiene: background jobs (with grandchildren) die with the gate - on stdin EOF and on SIGTERM - and
        no gui-gate temp file stays behind."""
        import signal
        for how in ("eof", "sigterm"):
            tmp = tempfile.mkdtemp()
            mark = f"9{os.getpid()}{len(how)}7"
            c = Client("--human", "deny", env={"GUI_GATE_JOBS": "1", "TMPDIR": tmp})
            try:
                r = c.call("run_command", command=f"(sleep {mark}) & sleep {mark}", mode="start", cwd=tmp)
                self.assertIn("started job", txt(r), txt(r))
                time.sleep(0.5)
                self.assertTrue(self._job_procs(f"sleep {mark}"), "job not running")
                self.assertEqual([n for n in os.listdir(tmp) if n.startswith("gui-gate-job-")], [], "job log left on disk")
            finally:
                if how == "eof":
                    c.close()
                else:
                    c.p.send_signal(signal.SIGTERM)
                    c.p.wait(timeout=5)
            time.sleep(0.5)
            self.assertEqual(self._job_procs(f"sleep {mark}"), [], f"job survived the gate ({how})")
            self.assertEqual([n for n in os.listdir(tmp) if n.startswith("gui-gate-")], [], f"temp left ({how})")
            shutil.rmtree(tmp, ignore_errors=True)

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
            self.assertIn("[gate] Noted", txt(r))
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
            if os.name == "nt":  # run_command is not offered on Windows (the gate answered, nothing ran): make the file
                open(os.path.join(d, "done.txt"), "w").close()
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


class LoopTests(unittest.TestCase):
    def test_two_strikes_on_the_same_mechanism(self):
        c = Client("--human", "deny", env={"GUI_GATE_TWO_STRIKES": "1"})
        try:
            c.call("fake_mutate", fail_next=2)
            r1 = c.call("click", element_token=f"{c.read()}:0")
            self.assertNotIn("Two strikes", txt(r1))
            r2 = c.call("click", element_token=f"{c.read(fresh=True)}:0")
            self.assertIn("[gate] Two strikes: the same mechanism (click on Increment) failed twice", txt(r2) + txt(r1))
            ev = [e for e in c.events() if e.get("ev") == "loop_breaker"]
            self.assertEqual(ev[0]["reason"], "two_strikes")
        finally:
            c.close()

    def test_without_flag_two_failures_give_no_note(self):
        c = Client("--human", "deny")
        try:
            c.call("fake_mutate", fail_next=2)
            r1 = c.call("click", element_token=f"{c.read()}:0")
            r2 = c.call("click", element_token=f"{c.read(fresh=True)}:0")
            self.assertNotIn("Two strikes", txt(r1) + txt(r2))
        finally:
            c.close()

    def test_loop_judge_at_level_three(self):
        llm = FakeLLM(lambda sys, user: json.dumps({"final": {"verdict": "SWITCH", "blacklist": "clicking OK", "advice": "use the keyboard shortcut instead."}}))
        c = Client("--human", "deny", "--task", "press the button", env={"GUI_GATE_TWO_STRIKES": "1", **llm.env("judge")})
        try:
            c.call("fake_mutate", fail_next=60, elements=[["button", f"B{i}"] for i in range(14)])
            out = ""
            for i in range(26):
                out += txt(c.call("click", element_token=f"{c.read(fresh=True)}:{i % 14}"))
                if "Loop judge" in out:
                    break
            self.assertIn("Loop judge (api:chat:fake-judge): SWITCH - use the keyboard shortcut instead.", out)
            self.assertIn("you judge whether a gui agent is stuck", llm.seen[0]["messages"][0]["content"].lower())
        finally:
            c.close()
            llm.close()


class GroundingTests(unittest.TestCase):
    def test_pixel_click_next_to_an_element_is_snapped(self):
        c = Client("--human", "deny", env={"GUI_GATE_SNAP": "1"})
        try:
            c.call("get_window_state", pid=4242, window_id=777, tree_format="elements", include_screenshot=True)
            r = c.call("click", pid=4242, window_id=777, x=215, y=25)
            self.assertIn('the gate clicked that element', txt(r))
            st = json.loads(txt(c.call("fake_state")))
            self.assertIn("Increment", st["clicked"])
            # inside an element: unchanged
            c.call("get_window_state", pid=4242, window_id=777, tree_format="elements", include_screenshot=True, fresh=True)
            r = c.call("click", pid=4242, window_id=777, x=100, y=25)
            self.assertNotIn("gate clicked that element", txt(r))
        finally:
            c.close()


class MicroVerifyTests(unittest.TestCase):
    def test_save_check_sees_the_unsaved_marker(self):
        c = Client("--human", "deny", env={"GUI_GATE_MICRO_VERIFY": "1", "FAKE_TITLE": "*notes.txt - Mousepad"})
        try:
            c.read()
            r = c.call("hotkey", pid=4242, window_id=777, keys=["ctrl", "s"])
            self.assertIn("[gate] Save check: \"*notes.txt - Mousepad\" still shows an unsaved-changes marker", txt(r))
        finally:
            c.close()


class PickTests(unittest.TestCase):
    def test_pick_one_prefers_the_safe_likely_candidate(self):
        c = Client("--human", "deny", env={"GUI_GATE_PICK": "1"})
        try:
            tools = {t["name"]: t for t in c.rpc("tools/list", {})["tools"]}
            self.assertIn("pick_one", tools["run_actions"]["inputSchema"]["properties"])
            s = c.read()
            r = c.call("run_actions", pick_one=True, steps=[{"tool": "click", "args": {"element_token": f"{s}:1"}},
                                                            {"tool": "click", "args": {"element_token": f"{s}:0"}}])
            self.assertIn("pick_one: ran candidate 2", txt(r))
            self.assertIn("(high risk)", txt(r))
            st = json.loads(txt(c.call("fake_state")))
            self.assertEqual(st["clicked"], ["Increment"])
        finally:
            c.close()

    def test_judge_picks_among_safe_candidates(self):
        llm = FakeLLM(lambda sys, user: json.dumps({"final": {"pick": 1, "reason": "the delete is what is needed"}}))
        c = Client("--human", "deny", env={"GUI_GATE_PICK": "1", **llm.env("judge")})
        try:
            s = c.read()
            r = c.call("run_actions", pick_one=True, steps=[{"tool": "click", "args": {"element_token": f"{s}:1"}},
                                                            {"tool": "click", "args": {"element_token": f"{s}:0"}},
                                                            {"tool": "click", "args": {"element_token": f"{s}:3"}}])
            self.assertNotIn("ran candidate 1", txt(r))  # the judge cannot pick the risky one over safe ones
            llm2 = FakeLLM(lambda sys, user: json.dumps({"final": {"pick": 3, "reason": "OK closes it"}}))
            c2 = Client("--human", "deny", env={"GUI_GATE_PICK": "1", **llm2.env("judge")})
            s2 = c2.read()
            r2 = c2.call("run_actions", pick_one=True, steps=[{"tool": "click", "args": {"element_token": f"{s2}:0"}},
                                                              {"tool": "click", "args": {"element_token": f"{s2}:3"}}])
            self.assertIn("candidate count error" if False else "pick_one: ran candidate", txt(r2))
            c2.close()
            llm2.close()
        finally:
            c.close()
            llm.close()


class DigestTests(unittest.TestCase):
    def test_periodic_digest(self):
        c = Client("--human", "deny", "--task", "count to two", env={"GUI_GATE_DIGEST": "1", "GUI_GATE_DIGEST_EVERY": "2"})
        try:
            r1 = c.call("click", element_token=f"{c.read()}:0")
            self.assertNotIn("Progress digest", txt(r1))
            r2 = c.call("click", element_token=f"{c.read(fresh=True)}:0")
            self.assertIn("[gate] Progress digest after 2 steps (0 failed", txt(r2))
            self.assertIn('click button "Increment" -> ok', txt(r2))
        finally:
            c.close()

    def test_compact_role_writes_the_digest(self):
        llm = FakeLLM(lambda sys, user: json.dumps({"final": {"digest": "## Task\ncount\n## State\ntwo clicks done"}}))
        c = Client("--human", "deny", "--task", "count to two", env={"GUI_GATE_DIGEST": "1", "GUI_GATE_DIGEST_EVERY": "2", **llm.env("compact")})
        try:
            c.call("click", element_token=f"{c.read()}:0")
            r = c.call("click", element_token=f"{c.read(fresh=True)}:0")
            self.assertIn("two clicks done", txt(r))
            self.assertIn("ALL STEPS SO FAR", llm.seen[0]["messages"][1]["content"])
        finally:
            c.close()
            llm.close()


class VerifyPlusTests(unittest.TestCase):
    def test_check_done_reads_back_and_evaluates_checks(self):
        d = tempfile.mkdtemp()
        p = os.path.join(d, "result.txt")
        c = Client("--human", "deny", "--task", f"Write the total into {p}", env={"GUI_GATE_VERIFY_PLUS": "1", "GUI_GATE_STATE_PLAN": "1"})
        try:
            c.call("set_plan", feasible=True, milestones=[{"state": f"{p} holds the total 42", "check": {"file_contains": [p, "42"]}}])
            if os.name == "nt":  # no run_command on Windows: the file is written the way an app would
                with open(p, "w") as fh:
                    fh.write("total 41\n")
            else:
                c.call("run_command", command=f"echo 'total 41' > {p}")
            r = c.call("check_done")
            t = txt(r)
            self.assertIn("Saved content of", t)
            self.assertIn("total 41", t)
            self.assertIn("milestone 1 check does not hold", t)
            self.assertIn("plan_check", r["structuredContent"]["kinds"])
        finally:
            c.close()

    def test_verifier_role_verdict_and_budget(self):
        def script(system, user):
            if "verify whether a Linux desktop" in system:
                return json.dumps({"final": {"decision": "failed", "reasoning": "x", "diagnosis": "the file says 41", "suggestion": "write 42 into the named file"}})
            return "{}"
        llm = FakeLLM(script)
        d = tempfile.mkdtemp()
        c = Client("--human", "deny", "--task", f"Write 42 into {d}/r.txt", env={"GUI_GATE_VERIFY_PLUS": "1", **llm.env("verifier")})
        try:
            c.call("run_command", command=f"echo 41 > {d}/r.txt")
            r = c.call("check_done")
            self.assertIn("Independent verifier (api:chat:fake-verifier): FAILED", txt(r))
            self.assertIn("suggestion: write 42 into the named file", txt(r))
            self.assertEqual(r["structuredContent"]["verifier"]["decision"], "failed")
            c.call("check_done")
            r3 = c.call("check_done")
            self.assertNotIn("Independent verifier", txt(r3))  # budget: two verdicts per session
            self.assertEqual(len(llm.seen), 2)
        finally:
            c.close()
            llm.close()


class GrounderRoleTests(unittest.TestCase):
    def test_grounder_role_locates_on_a_screenshot(self):
        llm = FakeLLM(lambda sys, user: json.dumps({"final": {"found": True, "x": 60, "y": 70, "why": "label Delete file"}}))
        c = Client("--human", "deny", env={"GUI_GATE_INSPECT": "1", **llm.env("grounder")})
        try:
            c.read()
            r = c.call("inspect", kind="locate", pid=4242, window_id=777, describe="the Delete file button")
            t = txt(r)
            self.assertIn("grounder (api:chat:fake-grounder): x=60 y=70", t)
            self.assertIn('[1] button "Delete file"', t)
            body = llm.seen[0]["messages"][1]["content"]
            self.assertEqual(body[0]["type"], "image_url")  # the screenshot goes to the vision model natively
            self.assertTrue(body[0]["image_url"]["url"].startswith("data:image/"))
        finally:
            c.close()
            llm.close()

    def test_roles_never_read_the_gates_key_files(self):
        d = tempfile.mkdtemp()
        key = os.path.join(d, "role.key")
        open(key, "w").write("sk-should-not-leak")
        c = Client("--human", "deny", env={"GUI_GATE_INSPECT": "1", "GUI_GATE_ROLE_SEARCH_KEY_FILE": key})
        try:
            r = c.call("inspect", kind="file", path=key)
            self.assertNotIn("sk-should-not-leak", txt(r))
            self.assertIn("refused", txt(r))
        finally:
            c.close()
