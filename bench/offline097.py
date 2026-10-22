"""0.9.7 offline evidence (no model, no box): replays the packed runs in results/ against the 0.9.7 features.

    python3 bench/offline097.py [--bin /tmp/gg-target/release/gui-gate] > docs/offline097.txt

Runs: m3 (mercury-3, text-only, gui-gate 0.9.5, 34 tasks x direct/gate8 x 2 seeds), ds41 (deepseek-v4.1-flash, 0.9.4,
1 seed), v096 (deepseek smoke on opt-0.9.6, 10 tasks x 2 conds). Sections:
  A phantom tool calls (names outside the advertised tools) and what 0.9.7 maps them to (Rust phantom_replay)
  B two-strikes loop notes vs the 0.9.6 loop breaker (Rust loop_replay / loop_replay_097 over the gate logs)
  C read-file detours that inspect kind=file replaces (shell commands / terminal lines that only read a file)
  D infeasible tasks (ow06, ow17): outcomes, and what the model-free fact sheet could show
  E premature DONEs (DONE replies scored 0) and which 0.9.7 checks could see the gap
  F save actions (ctrl+s) and what came back (micro-verification target)
  G pixel clicks (snap target) and digest size per run
"""
import collections, glob, json, os, re, subprocess, sys, tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RES = f"{ROOT}/results"
RUNS = {"m3": f"{RES}/pi-m3-v095-2026-10-22", "ds41": f"{RES}/pi-ds41-v094-2026-10-21", "v096": f"{RES}/smoke-ds41-v096-2026-10-22"}
BIN_ARGS = sys.argv[sys.argv.index("--bin") + 1] if "--bin" in sys.argv else None
TASKS = {os.path.basename(f)[:-5]: json.load(open(f))["instruction"] for f in glob.glob(f"{ROOT}/bench/tasks/*/*.json")}
GATE_TOOLS = ["list_apps", "list_windows", "get_window_state", "launch_app", "bring_to_front", "click", "double_click", "right_click",
              "type_text", "press_key", "hotkey", "scroll", "set_value", "drag", "invoke_menu", "gate_report", "request_human",
              "gate_task", "run_actions", "run_command", "check_done", "set_plan", "gate_config", "move_cursor", "run_steps",
              "screenshot", "verify_state", "get_cursor_position", "zoom", "page", "kill_app", "launch_url", "get_screen_size"]


def text_of(res):
    if not isinstance(res, dict):
        return str(res or "")
    return "\n".join(c.get("text", "") for c in res.get("content", []) if isinstance(c, dict))


def load_run(path):
    starts, calls = {}, []
    for l in open(path):
        try:
            e = json.loads(l)
        except Exception:
            continue
        t = e.get("type")
        if t == "tool_execution_start":
            c = dict(id=e.get("toolCallId"), name=(e.get("toolName") or "").replace("mcp__gui__", ""), args=e.get("args") or {}, result="", err=None)
            starts[c["id"]] = c
            calls.append(c)
        elif t == "tool_execution_end":
            c = starts.get(e.get("toolCallId"))
            if c is not None:
                r = e.get("result") or {}
                c["result"] = text_of(r)
                c["err"] = bool(r.get("isError")) if isinstance(r, dict) else None
    fp = path + ".final.txt"
    return calls, (open(fp).read().strip() if os.path.exists(fp) else "")


def runs(which):
    base = RUNS[which]
    for l in open(f"{base}/runs_scored.jsonl"):
        r = json.loads(l)
        p = f"{base}/pi/{r['tag']}.jsonl"
        if r.get("infra") or not os.path.exists(p):
            continue
        calls, final = load_run(p)
        yield dict(tag=r["tag"], task=r["task"], cond=r["cond"], success=bool(r.get("success")), score=r.get("score") or 0,
                   calls=calls, final=final or (r.get("final") or ""), gate=f"{base}/gate/{r['tag']}.jsonl", run=which)


ALL = {w: list(runs(w)) for w in RUNS}


def cargo_ignored(test, envs):
    if BIN_ARGS is None and not os.environ.get("CARGO_TARGET_DIR"):
        os.environ["CARGO_TARGET_DIR"] = "/tmp/gg-target"
    full = {"phantom_replay": "v097::tests::phantom_replay"}.get(test, f"loopbreak::tests::{test}")
    cmd = ["cargo", "test", "--quiet", "--", "--ignored", "--exact", full]
    subprocess.run(cmd, cwd=ROOT, env=dict(os.environ, **envs), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)


def sec(t):
    print(f"\n## {t}")


# ---------------------------------------------------------------- A phantom tools
sec("A. Phantom tool calls (tool names the agent invented)")
names = collections.Counter()
where = collections.defaultdict(set)
for w, rs in ALL.items():
    for r in rs:
        for c in r["calls"]:
            n = c["name"]
            if n and n not in GATE_TOOLS and not n.startswith("mcp__") and n not in ("codemode",):
                names[(w, n)] += 1
                where[(w, n)].add(r["tag"])
tot_calls = {w: sum(len(r["calls"]) for r in rs) for w, rs in ALL.items()}
if names:
    inp = tempfile.NamedTemporaryFile("w", delete=False)
    for (w, n) in names:
        inp.write(f"{n}\t{','.join(GATE_TOOLS + ['inspect'])}\n")
    inp.close()
    out = inp.name + ".out"
    cargo_ignored("phantom_replay", {"GG_PHANTOM_IN": inp.name, "GG_PHANTOM_OUT": out})
    mapped = {}
    if os.path.exists(out):
        for l in open(out):
            a = l.rstrip("\n").split("\t")
            mapped[a[0]] = (a[1], a[2] if len(a) > 2 else "")
    for (w, n), k in sorted(names.items(), key=lambda x: -x[1]):
        m = mapped.get(n, ("?", ""))
        print(f"- {w}: {n!r} x{k} in {len(where[(w, n)])} run(s) -> 0.9.7: {m[0] if m[0] != '-' else 'error + nearest ' + m[1]}")
else:
    print("- none")
for w in ALL:
    k = sum(v for (ww, _), v in names.items() if ww == w)
    print(f"- {w}: {k} of {tot_calls[w]} tool calls ({100.0 * k / max(1, tot_calls[w]):.2f}%) used a name outside the advertised tools")

# ---------------------------------------------------------------- B loop replay
sec("B. Two strikes vs the 0.9.6 loop breaker (gate logs, gate8 only - the breaker is gate-policy only)")
for w, rs in ALL.items():
    logs = [r for r in rs if r["cond"] == "gate8" and os.path.exists(r["gate"])]
    if not logs:
        continue
    lst = tempfile.NamedTemporaryFile("w", delete=False)
    lst.write("\n".join(r["gate"] for r in logs))
    lst.close()
    res = {}
    for test in ("loop_replay", "loop_replay_097"):
        out = lst.name + "." + test
        cargo_ignored(test, {"GG_LOOP_LOGS": lst.name, "GG_LOOP_OUT": out})
        ev = collections.defaultdict(list)
        if os.path.exists(out):
            for l in open(out):
                a = l.rstrip("\n").split("\t")
                if a[2] == "note":
                    ev[a[0]].append(a)
        res[test] = ev
    succ = {r["gate"]: r["success"] for r in logs}
    for test, ev in res.items():
        fired = [g for g in ev if ev[g]]
        two = sum(1 for g in ev for a in ev[g] if len(a) > 5 and a[5] == "two_strikes")
        print(f"- {w} {test}: notes in {len(fired)}/{len(logs)} runs (successful runs with a note: {sum(1 for g in fired if succ[g])}/{sum(succ.values())}); "
              f"level-1 notes {sum(1 for g in ev for a in ev[g] if a[3] == '1')}" + (f", of which two-strikes {two}" if test.endswith("097") else ""))
    e96, e97 = res["loop_replay"], res["loop_replay_097"]
    earlier = []
    for g in e97:
        a97 = [int(a[1]) for a in e97[g]]
        a96 = [int(a[1]) for a in e96.get(g, [])]
        if a97 and (not a96 or min(a97) < min(a96)):
            earlier.append((min(a96) - min(a97)) if a96 else None)
    print(f"  {w}: first note earlier with two strikes in {len(earlier)} runs ({sum(1 for x in earlier if x is None)} where 0.9.6 never fired; "
          f"median {sorted([x for x in earlier if x is not None])[len([x for x in earlier if x is not None]) // 2] if [x for x in earlier if x is not None] else '-'} actions earlier otherwise)")

# ---------------------------------------------------------------- C read-file detours
sec("C. Reading a file with code / the terminal (inspect kind=file replaces these)")
READ_RX = re.compile(r"^\s*(cat|head|tail|less|more|wc|file|pdftotext|xxd|od)\b|python3?\s+-c\s+['\"].*(print|read)\(|unzip\s+-p|openpyxl.*load_workbook.*print|docx\.Document.*print")
for w, rs in ALL.items():
    n_cmd = n_read = runs_read = 0
    for r in rs:
        k = 0
        for c in r["calls"]:
            cmd = ""
            if c["name"] == "run_command":
                cmd = str(c["args"].get("command", ""))
            elif c["name"] == "type_text" and "\n" in str(c["args"].get("text", "")):
                cmd = str(c["args"].get("text", ""))
            if not cmd:
                continue
            n_cmd += 1
            if READ_RX.search(cmd.split("\n")[0]) and not re.search(r"(>|>>)\s*\S", cmd.split("\n")[0]):
                n_read += 1
                k += 1
        runs_read += k > 0
    print(f"- {w}: {n_read} of {n_cmd} shell commands only read a file (cat/head/python print/unzip -p ...), in {runs_read} runs")

# ---------------------------------------------------------------- D infeasible
sec("D. Infeasible tasks (ow06 Chrome UI language 'Xenothian', ow17 GIMP CMYK)")
for w, rs in ALL.items():
    for t in ("ow06", "ow17"):
        rr = [r for r in rs if r["task"].startswith(t)]
        if not rr:
            continue
        fails = sum(1 for r in rr if re.match(r"\s*FAIL", r["final"] or ""))
        verdict = sum(1 for r in rr for c in r["calls"] if c["name"] == "set_plan" and c["args"].get("feasible") is False)
        print(f"- {w} {t}: {len(rr)} runs, replied FAIL {fails}, set_plan feasible:false {verdict}, scored success {sum(r['success'] for r in rr)}")
print("- model-free fact sheet: both apps are installed and no file/hardware is missing, so inspect env shows no evidence of\n"
      "  infeasibility for either task (0/2); catching them needs knowledge of the app's features = the feasibility role\n"
      "  (Pointer: 24/28 infeasible caught, 2/333 false alarms with Sonnet 4.6) or the acting model's own verdict.")

# ---------------------------------------------------------------- E premature DONE
sec("E. Premature DONEs (final reply DONE, evaluator score 0)")
for w, rs in ALL.items():
    pd = [r for r in rs if re.match(r"\s*DONE", r["final"] or "") and not r["success"]]
    print(f"- {w}: {len(pd)} premature DONE(s): {', '.join(r['tag'] for r in pd)}")
print("  0.9.7 coverage by inspection of each case (docs/sota-absorb-2026-10-22.md, round 21): saved-content read-back\n"
      "  (VERIFY_PLUS) shows text content, not formatting/colour/settings; plan checks need the agent to state a checkable\n"
      "  end state; the verifier role can probe settings files and document XML (python-docx / vlcrc) = the only\n"
      "  0.9.7 part that targets formatting and settings gaps.")

# ---------------------------------------------------------------- F saves
sec("F. Save actions (ctrl+s / ctrl+shift+s) - micro-verification target")
for w, rs in ALL.items():
    n = dlg = 0
    for r in rs:
        for i, c in enumerate(r["calls"]):
            a = json.dumps(c["args"]).lower()
            if c["name"] in ("hotkey", "press_key") and re.search(r'"s"', a) and "ctrl" in a:
                n += 1
                nxt = " ".join(x["result"] for x in r["calls"][i:i + 2]).lower()
                if re.search(r"keep current format|use .* format|save as|replace|overwrite", nxt):
                    dlg += 1
    print(f"- {w}: {n} save keystrokes; a format / save-as / overwrite dialog showed up in that or the next result {dlg} times")

# ---------------------------------------------------------------- G pixel clicks, digest size
sec("G. Pixel clicks (snap target) and digest cost")
for w, rs in ALL.items():
    pix = pix_err = steps = nrun = 0
    for r in rs:
        if not os.path.exists(r["gate"]):
            continue
        nrun += 1
        ex = 0
        for l in open(r["gate"]):
            try:
                e = json.loads(l)
            except Exception:
                continue
            if e.get("ev") == "action" and e.get("executed"):
                ex += 1
                if (e.get("target") or {}).get("kind") == "pixel":
                    pix += 1
                    pix_err += bool(e.get("is_error"))
        steps += ex
    per = steps / max(1, nrun)
    print(f"- {w}: {pix} pixel clicks executed ({pix_err} driver errors) over {nrun} runs; {per:.1f} executed steps/run -> "
          f"{int(per // 8)} digest(s)/run at the default every 8 (~0.8-1.2 KB each, resent with every later model call)")
