# OSWorld-Verified full run: gui-gate v0.9.9 (all 0.9.7+ features, 7 DeepSeek roles) + Pi + deepseek-v4.1-flash, box gui-pi3

Started 2026-10-23 00:07:58 CST. Condition: gate only (bench cond `gate8` = gate policy + task text + reviewer), seed 0.
No direct baseline in this run.

## Setup (exact)
- Box: InstaBox gui-pi3 (new, 4 vCPU / 8 GB / 20 GB), Debian 13, XFCE, screen 1440x900, no VM (OSWorld's own in-guest
  server + SetupController + evaluators run natively, see bench/README.md for the documented differences from the
  OSWorld VM: Debian/XFCE instead of Ubuntu/GNOME, 1440x900 instead of 1920x1080, Chromium 155 instead of Google Chrome
  (Chrome >= 136 refuses --remote-debugging-port on its default profile dir, which OSWorld's getters read), LibreOffice
  25.2, GIMP 3, VLC 3.0.24, Thunderbird 140.17 ESR, VS Code 1.143).
- Tasks: OSWorld commit f9885c9, `evaluation_examples/test_nogdrive.json` = official test_all (369) minus the 8 Google
  Drive tasks = **361**, unmodified (bench/make_verified_list.py; run order shuffled with seed 0, list in setup/).
  Scoring: OSWorld's own `DesktopEnv.evaluate` (getters + metrics unchanged; real playwright / librosa / easyocr / borb
  installed, no stubs in use for these). success = score >= 1.
- Step budget: OSWorld's max_steps 100 -> Pi `--max-turns 100` (one Pi assistant turn = one step; a turn may carry one
  run_actions batch of up to 20 GUI actions, so this is at least as generous as 100 OSWorld steps). The old 480 s wall
  cap is removed; safety timeout 3600 s per task.
- Acting model: Pi 1.3.0, tokenrhythm `deepseek-v4.1-flash-1017` (OpenAI chat, images on), thinking medium.
- gui-gate 0.9.9 from the official release installer (`install.sh --client pi --no-prompt`), bundled Cua Driver 0.38.0.
  Gate env: GUI_GATE_097=1 and each switch explicitly (INSPECT JOBS PHANTOM TWO_STRIKES FEASIBILITY_FIRST STATE_PLAN
  VERIFY_PLUS SKILLS SNAP PICK DIGEST MICRO_VERIFY), GUI_GATE_MEMORY = an EMPTY file (experience memory switched on but
  empty on purpose: earlier runs share tasks with this benchmark, so filling it would leak answers).
  Bench: PI_TOOLS_096=1 PI_TOOLS_097=1 PI_APP_HINTS=0 (the gate's own skills replace the bench hints) PI_OBS_PRUNE=0.
- Reviewer (decision model): Qianwen System One `decision-model-preview`, protocol systemone (check-reviewer 2/2).
- 7 internal roles (feasibility, planner, verifier, judge, search, compact, grounder): all tokenrhythm
  deepseek-v4.1-flash-1017, protocol chat, max_tokens 8192, timeout 120 s (setup/reviewer.env.redacted). A direct MCP
  check before the run called every role once successfully (setup/rolecheck.log); the grounder role got a native image.
- Screenshots: JPEG q60 at the window's native size (no downscaling).
- --human deny (no simulated user); request_human calls are counted separately.
- Infra: relay errors (400 "thinking must be passed back" / 5xx -> Pi stopReason error) are retried up to 3 times and,
  if still failing, excluded from the valid denominator; setup failures are retried once after 20 s.
- Harness fixes made for this run (bench/, committed before the run): Pi MCP call timeout 600 s (Pi's 60 s default cut
  the verifier inside check_done in the smoke), setup retry, host_reference_date state carried from setup to eval.

## Files
report.md / report.json / tasks.csv (per task), runs.jsonl, eval/ (setup + eval JSON per task), gate/ (gate logs),
pi/ (Pi event streams, base64 screenshots stripped, streaming deltas dropped; .times = event arrival times,
.final.txt = final answer, .err = Pi stderr), task/ (task text given to the gate), setup/ (versions, configs, launcher,
task list, role check, reviewer check).

## Results (state: partial 321/361)


- tasks expected 361, finished rows 290, valid 266, infra 24 {'relay_error': 4, 'setup_failed': 19, 'eval_error': 1}
- **success 143/361 = 39.61 %** (official-style: every expected task counts, infra/missing = 0); 143/266 = 53.76 % of valid runs; score sum 149.33
- stops {'agent': 245, 'max_turns': 3, 'timeout': 18}; turns mean 30.3 / median 24.5; wall mean 718.8 s / median 350.6 s; total 55.23 h
- tokens: input 13,254,707 + cache read 315,316,992, output 2,353,572; screenshots returned 2366; tool calls 8506
- request_human calls 14 (human=deny); FAIL answers 30
- gate: {'actions': 8020, 'executed': 7764, 'interventions': 256, 'human_events': 98, 'loop_notes': 17, 'loop_refused': 8}
- reviewer (System One): {'calls': 97, 'decisions': {'deny': 61, 'allow': 36}, 'ms_mean': 947.7, 'ms_median': 702.6}

| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |
|---|---|---|---|---|---|---|
| feasibility | 266 | 266 | 1550 | 5 | 68012.2 | 56303.2 |
| planner | 266 | 266 | 898 | 3 | None | None |
| verifier | 237 | 248 | 1737 | 13 | 71098.6 | 55358.8 |
| judge | 0 | 0 | 0 | 0 | None | None |
| search | 5 | 5 | 5 | 0 | 13897.6 | 10481.4 |
| compact | 212 | 755 | 755 | 1 | 10225.5 | 9359.0 |
| grounder | 9 | 10 | 10 | 0 | 10042.4 | 5958.5 |

(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)

| domain | tasks | valid | success | rate (of tasks) | score sum | infra |
|---|---|---|---|---|---|---|
| libreoffice_impress | 35 | 35 | 7 | 20.0 % | 7.93 | 0 |
| os | 20 | 17 | 10 | 50.0 % | 10.0 | 3 |
| multi_apps | 73 | 56 | 26 | 35.6 % | 29.49 | 17 |
| libreoffice_writer | 18 | 17 | 13 | 72.2 % | 13.0 | 1 |
| vs_code | 21 | 21 | 17 | 81.0 % | 17.0 | 0 |
| libreoffice_calc | 41 | 39 | 33 | 80.5 % | 33.0 | 2 |
| vlc | 15 | 14 | 8 | 53.3 % | 8.99 | 1 |
| chrome | 39 | 39 | 13 | 33.3 % | 13.93 | 0 |
| gimp | 18 | 18 | 9 | 50.0 % | 9.0 | 0 |
| thunderbird | 10 | 10 | 7 | 70.0 % | 7.0 | 0 |

tools: {'run_command': 2150, 'run_actions': 1563, 'get_window_state': 1356, 'click': 1052, 'set_plan': 577, 'press_key': 497, 'list_windows': 494, 'inspect': 307, 'check_done': 249, 'scroll': 96, 'type_text': 83, 'list_apps': 63, 'request_human': 14, 'update_plan': 1, 'mcp__gui_run_actions': 1, 'mcp__gui_set_plan': 1, 'mcp__gui_inspect': 1, 'mcp__gui_press_key': 1}

## infra rows

- libreoffice_writer-e246f6d8-78d7-44ac-b668-fcf47946cb50: relay_error (attempts 3) 503: {"code":"SERVICE_BUSY","message":"服务繁忙，请稍后重试","traceId":"trace_b2875c7b-3153-415d-81e0-3a239d84a6ec"}
- os-28cc3b7e-b194-4bc9-8353-d04c0f4d56d2: setup_failed (attempts 1) Exception('Setup step 2 failed: _execute_with_verification_setup - Command verification failed: {\n  "error": "bash: line 1: pulseaudio: command not found\\nbash: line 1: pactl: command not found\\nbash: line 1: pactl: command not found\\nbash: line 1: pactl: command not found\\nbash: line 1: pactl:
- multi_apps-a82b78bb-7fde-4cb3-94a4-035baf10bcf0: setup_failed (attempts 1) Exception("Setup step 5 failed: _open_setup - Failed to open file '/home/user/Desktop/2206.08853.pdf'. An error occurred while trying to send the request or the server responded with an error: 500 Server Error: INTERNAL SERVER ERROR for url: http://127.0.0.1:5000/setup/open_file")
- os-4127319a-8b79-4410-b58a-7a151e15f3d7: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- multi_apps-2b9493d7-49b8-493a-a71b-56cd1f4d6908: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- multi_apps-2c9fc0de-3ee7-45e1-a5df-c86206ad78b5: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- multi_apps-f5c13cdd-205c-4719-a562-348ae5cd1d91: setup_failed (attempts 1) Exception('Setup step 5 failed: _launch_setup - Failed to launch application: HTTP 500')
- libreoffice_calc-30e3e107-1cfb-46ee-a755-2cd080d7ba6a: eval_error (attempts 1) TypeError("'NoneType' object is not subscriptable")
- multi_apps-185f29bd-5da0-40a6-b69c-ba7f4e0324ef: setup_failed (attempts 1) Exception("Setup step 4 failed: _open_setup - Failed to open file '/home/user/Desktop/review_template.pdf'. An error occurred while trying to send the request or the server responded with an error: 500 Server Error: INTERNAL SERVER ERROR for url: http://127.0.0.1:5000/setup/open_file")
- multi_apps-5990457f-2adb-467b-a4af-5c857c92d762: setup_failed (attempts 1) Exception('Setup step 4 failed: _launch_setup - Failed to launch application: HTTP 500')
- os-3ce045a0-877b-42aa-8d2c-b4a863336ab8: relay_error (attempts 3) 502 <html>
<head><title>502 Bad Gateway</title></head>
<body>
<center><h1>502 Bad Gateway</h1></center>
<hr><center>nginx/1.30.3</center>
</body>
</html>

- multi_apps-415ef462-bed3-493a-ac36-ca8c6d23bf1b: setup_failed (attempts 1) Exception('Setup step 6 failed: _launch_setup - Failed to launch application: HTTP 500')
- multi_apps-36037439-2044-4b50-b9d1-875b5a332143: setup_failed (attempts 1) Exception("Setup step 2 failed: _open_setup - Failed to open file '/home/user/Desktop/shi17a.pdf'. An error occurred while trying to send the request or the server responded with an error: 500 Server Error: INTERNAL SERVER ERROR for url: http://127.0.0.1:5000/setup/open_file")
- multi_apps-f7dfbef3-7697-431c-883a-db8583a4e4f9: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- multi_apps-42d25c08-fb87-4927-8b65-93631280a26f: setup_failed (attempts 1) Exception('Setup step 9 failed: _launch_setup - Failed to launch application: HTTP 500')
- multi_apps-3680a5ee-6870-426a-a997-eba929a0d25c: setup_failed (attempts 1) Exception('Setup step 2 failed: _execute_setup - Setup command request failed: HTTP 500')
- multi_apps-510f64c8-9bcc-4be1-8d30-638705850618: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- multi_apps-82e3c869-49f6-4305-a7ce-f3e64a0618e7: relay_error (attempts 3) Request timed out.
- multi_apps-aceb0368-56b8-4073-b70e-3dc9aee184e0: setup_failed (attempts 1) Exception('Setup step 5 failed: _launch_setup - Failed to launch application: HTTP 500')
- multi_apps-b5062e3e-641c-4e3a-907b-ac864d2e7652: setup_failed (attempts 1) Exception('Setup step 3 failed: _launch_setup - Failed to launch application: HTTP 500')
- vlc-386dbd0e-0241-4a0a-b6a2-6704fba26b1c: setup_failed (attempts 1) Exception("Setup step 4 failed: _open_setup - Failed to open file '/home/user/Desktop/lecture.pdf'. An error occurred while trying to send the request or the server responded with an error: 500 Server Error: INTERNAL SERVER ERROR for url: http://127.0.0.1:5000/setup/open_file")
- multi_apps-ee9a3c83-f437-4879-8918-be5efbb9fac7: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- libreoffice_calc-2bd59342-0664-4ccb-ba87-79379096cc08: relay_error (attempts 3) 503: {"code":"LITELLM_UNAVAILABLE","message":"模型服务暂时不可用，请稍后重试","traceId":"trace_06b17e68-c6d5-4c7a-b6f3-a06dca5b09fd"}
- multi_apps-0e5303d4-8820-42f6-b18d-daf7e633de21: setup_failed (attempts 1) Exception('Setup step 6 failed: _launch_setup - Failed to launch application: HTTP 500')
