# OSWorld-Verified report

- tasks expected 361, finished rows 28, valid 23, infra 5 {'relay_error': 1, 'setup_failed': 4}
- **success 16/361 = 4.43 %** (official-style: every expected task counts, infra/missing = 0); 16/23 = 69.57 % of valid runs; score sum 16.0
- stops {'agent': 23}; turns mean 29.4 / median 23; wall mean 421.8 s / median 292.7 s; total 2.87 h
- tokens: input 1,014,608 + cache read 25,214,336, output 187,330; screenshots returned 187; tool calls 723
- request_human calls 4 (human=deny); FAIL answers 3
- gate: {'actions': 567, 'executed': 551, 'interventions': 16, 'human_events': 10, 'loop_notes': 0, 'loop_refused': 0}
- reviewer (System One): {'calls': 6, 'decisions': {'deny': 4, 'allow': 2}, 'ms_mean': 818.4, 'ms_median': 710.8}

| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |
|---|---|---|---|---|---|---|
| feasibility | 23 | 23 | 153 | 1 | 75544.7 | 51031.4 |
| planner | 23 | 23 | 77 | 0 | None | None |
| verifier | 23 | 24 | 161 | 2 | 47362.0 | 43232.8 |
| judge | 0 | 0 | 0 | 0 | None | None |
| search | 0 | 0 | 0 | 0 | None | None |
| compact | 17 | 55 | 55 | 0 | 8812.1 | 7821.0 |
| grounder | 1 | 1 | 1 | 0 | 3396.8 | 3396.8 |

(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)

| domain | tasks | valid | success | rate (of tasks) | score sum | infra |
|---|---|---|---|---|---|---|
| libreoffice_impress | 1 | 1 | 1 | 100.0 % | 1.0 | 0 |
| os | 3 | 1 | 1 | 33.3 % | 1.0 | 2 |
| multi_apps | 7 | 5 | 2 | 28.6 % | 2.0 | 2 |
| libreoffice_writer | 2 | 1 | 1 | 50.0 % | 1.0 | 1 |
| vs_code | 3 | 3 | 3 | 100.0 % | 3.0 | 0 |
| libreoffice_calc | 6 | 6 | 6 | 100.0 % | 6.0 | 0 |
| vlc | 2 | 2 | 1 | 50.0 % | 1.0 | 0 |
| chrome | 1 | 1 | 0 | 0.0 % | 0.0 | 0 |
| gimp | 2 | 2 | 1 | 50.0 % | 1.0 | 0 |
| thunderbird | 1 | 1 | 0 | 0.0 % | 0.0 | 0 |

tools: {'run_command': 187, 'get_window_state': 128, 'run_actions': 111, 'click': 88, 'set_plan': 53, 'press_key': 45, 'list_windows': 43, 'check_done': 24, 'inspect': 24, 'list_apps': 5, 'type_text': 5, 'scroll': 5, 'request_human': 4, 'update_plan': 1}

## infra rows

- libreoffice_writer-e246f6d8-78d7-44ac-b668-fcf47946cb50: relay_error (attempts 3) 503: {"code":"SERVICE_BUSY","message":"服务繁忙，请稍后重试","traceId":"trace_b2875c7b-3153-415d-81e0-3a239d84a6ec"}
- os-28cc3b7e-b194-4bc9-8353-d04c0f4d56d2: setup_failed (attempts 1) Exception('Setup step 2 failed: _execute_with_verification_setup - Command verification failed: {\n  "error": "bash: line 1: pulseaudio: command not found\\nbash: line 1: pactl: command not found\\nbash: line 1: pactl: command not found\\nbash: line 1: pactl: command not found\\nbash: line 1: pactl:
- multi_apps-a82b78bb-7fde-4cb3-94a4-035baf10bcf0: setup_failed (attempts 1) Exception("Setup step 5 failed: _open_setup - Failed to open file '/home/user/Desktop/2206.08853.pdf'. An error occurred while trying to send the request or the server responded with an error: 500 Server Error: INTERNAL SERVER ERROR for url: http://127.0.0.1:5000/setup/open_file")
- os-4127319a-8b79-4410-b58a-7a151e15f3d7: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- multi_apps-2b9493d7-49b8-493a-a71b-56cd1f4d6908: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
