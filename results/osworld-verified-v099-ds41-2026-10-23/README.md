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

## Results (state: partial 70/361)


- tasks expected 361, finished rows 67, valid 60, infra 7 {'relay_error': 1, 'setup_failed': 6}
- **success 37/361 = 10.25 %** (official-style: every expected task counts, infra/missing = 0); 37/60 = 61.67 % of valid runs; score sum 38.75
- stops {'agent': 59, 'max_turns': 1}; turns mean 30.5 / median 25.0; wall mean 382.7 s / median 262.2 s; total 6.55 h
- tokens: input 2,962,189 + cache read 78,835,968, output 554,072; screenshots returned 556; tool calls 1963
- request_human calls 4 (human=deny); FAIL answers 4
- gate: {'actions': 1794, 'executed': 1738, 'interventions': 56, 'human_events': 22, 'loop_notes': 0, 'loop_refused': 1}
- reviewer (System One): {'calls': 33, 'decisions': {'deny': 17, 'allow': 16}, 'ms_mean': 880.1, 'ms_median': 687.8}

| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |
|---|---|---|---|---|---|---|
| feasibility | 60 | 60 | 366 | 2 | 63568.0 | 48773.7 |
| planner | 60 | 60 | 217 | 2 | None | None |
| verifier | 60 | 65 | 451 | 3 | 57617.5 | 46986.2 |
| judge | 0 | 0 | 0 | 0 | None | None |
| search | 1 | 1 | 1 | 0 | 18235.6 | 18235.6 |
| compact | 49 | 172 | 172 | 0 | 8594.0 | 7887.5 |
| grounder | 2 | 2 | 2 | 0 | 3597.9 | 3597.9 |

(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)

| domain | tasks | valid | success | rate (of tasks) | score sum | infra |
|---|---|---|---|---|---|---|
| libreoffice_impress | 7 | 7 | 1 | 14.3 % | 1.0 | 0 |
| os | 3 | 1 | 1 | 33.3 % | 1.0 | 2 |
| multi_apps | 17 | 13 | 6 | 35.3 % | 7.75 | 4 |
| libreoffice_writer | 4 | 3 | 3 | 75.0 % | 3.0 | 1 |
| vs_code | 6 | 6 | 5 | 83.3 % | 5.0 | 0 |
| libreoffice_calc | 13 | 13 | 13 | 100.0 % | 13.0 | 0 |
| vlc | 4 | 4 | 3 | 75.0 % | 3.0 | 0 |
| chrome | 5 | 5 | 2 | 40.0 % | 2.0 | 0 |
| gimp | 5 | 5 | 2 | 40.0 % | 2.0 | 0 |
| thunderbird | 3 | 3 | 1 | 33.3 % | 1.0 | 0 |

tools: {'run_command': 542, 'run_actions': 354, 'get_window_state': 297, 'click': 229, 'set_plan': 138, 'list_windows': 110, 'press_key': 91, 'inspect': 86, 'check_done': 65, 'type_text': 17, 'scroll': 17, 'list_apps': 12, 'request_human': 4, 'update_plan': 1}

## infra rows

- libreoffice_writer-e246f6d8-78d7-44ac-b668-fcf47946cb50: relay_error (attempts 3) 503: {"code":"SERVICE_BUSY","message":"服务繁忙，请稍后重试","traceId":"trace_b2875c7b-3153-415d-81e0-3a239d84a6ec"}
- os-28cc3b7e-b194-4bc9-8353-d04c0f4d56d2: setup_failed (attempts 1) Exception('Setup step 2 failed: _execute_with_verification_setup - Command verification failed: {\n  "error": "bash: line 1: pulseaudio: command not found\\nbash: line 1: pactl: command not found\\nbash: line 1: pactl: command not found\\nbash: line 1: pactl: command not found\\nbash: line 1: pactl:
- multi_apps-a82b78bb-7fde-4cb3-94a4-035baf10bcf0: setup_failed (attempts 1) Exception("Setup step 5 failed: _open_setup - Failed to open file '/home/user/Desktop/2206.08853.pdf'. An error occurred while trying to send the request or the server responded with an error: 500 Server Error: INTERNAL SERVER ERROR for url: http://127.0.0.1:5000/setup/open_file")
- os-4127319a-8b79-4410-b58a-7a151e15f3d7: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- multi_apps-2b9493d7-49b8-493a-a71b-56cd1f4d6908: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- multi_apps-2c9fc0de-3ee7-45e1-a5df-c86206ad78b5: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- multi_apps-f5c13cdd-205c-4719-a562-348ae5cd1d91: setup_failed (attempts 1) Exception('Setup step 5 failed: _launch_setup - Failed to launch application: HTTP 500')
