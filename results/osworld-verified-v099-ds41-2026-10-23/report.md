# OSWorld-Verified report

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
