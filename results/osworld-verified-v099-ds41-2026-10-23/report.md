# OSWorld-Verified report

- tasks expected 361, finished rows 86, valid 78, infra 8 {'relay_error': 1, 'setup_failed': 6, 'eval_error': 1}
- **success 45/361 = 12.47 %** (official-style: every expected task counts, infra/missing = 0); 45/78 = 57.69 % of valid runs; score sum 46.75
- stops {'agent': 77, 'max_turns': 1}; turns mean 29.9 / median 22.0; wall mean 411.5 s / median 262.4 s; total 9.67 h
- tokens: input 3,922,049 + cache read 99,256,832, output 675,059; screenshots returned 700; tool calls 2495
- request_human calls 5 (human=deny); FAIL answers 6
- gate: {'actions': 2237, 'executed': 2172, 'interventions': 65, 'human_events': 27, 'loop_notes': 0, 'loop_refused': 1}
- reviewer (System One): {'calls': 35, 'decisions': {'deny': 17, 'allow': 18}, 'ms_mean': 880.3, 'ms_median': 693.0}

| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |
|---|---|---|---|---|---|---|
| feasibility | 78 | 78 | 465 | 2 | 63759.2 | 49540.3 |
| planner | 78 | 78 | 285 | 3 | None | None |
| verifier | 77 | 82 | 577 | 4 | 59919.7 | 51166.9 |
| judge | 0 | 0 | 0 | 0 | None | None |
| search | 1 | 1 | 1 | 0 | 18235.6 | 18235.6 |
| compact | 62 | 214 | 214 | 0 | 8693.9 | 7864.5 |
| grounder | 3 | 4 | 4 | 0 | 7050.5 | 3728.6 |

(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)

| domain | tasks | valid | success | rate (of tasks) | score sum | infra |
|---|---|---|---|---|---|---|
| libreoffice_impress | 8 | 8 | 2 | 25.0 % | 2.0 | 0 |
| os | 4 | 2 | 2 | 50.0 % | 2.0 | 2 |
| multi_apps | 24 | 20 | 8 | 33.3 % | 9.75 | 4 |
| libreoffice_writer | 4 | 3 | 3 | 75.0 % | 3.0 | 1 |
| vs_code | 7 | 7 | 6 | 85.7 % | 6.0 | 0 |
| libreoffice_calc | 14 | 13 | 13 | 92.9 % | 13.0 | 1 |
| vlc | 6 | 6 | 4 | 66.7 % | 4.0 | 0 |
| chrome | 9 | 9 | 2 | 22.2 % | 2.0 | 0 |
| gimp | 6 | 6 | 3 | 50.0 % | 3.0 | 0 |
| thunderbird | 4 | 4 | 2 | 50.0 % | 2.0 | 0 |

tools: {'run_command': 692, 'run_actions': 450, 'get_window_state': 372, 'click': 302, 'set_plan': 180, 'list_windows': 142, 'press_key': 105, 'inspect': 103, 'check_done': 82, 'type_text': 24, 'scroll': 20, 'list_apps': 17, 'request_human': 5, 'update_plan': 1}

## infra rows

- libreoffice_writer-e246f6d8-78d7-44ac-b668-fcf47946cb50: relay_error (attempts 3) 503: {"code":"SERVICE_BUSY","message":"服务繁忙，请稍后重试","traceId":"trace_b2875c7b-3153-415d-81e0-3a239d84a6ec"}
- os-28cc3b7e-b194-4bc9-8353-d04c0f4d56d2: setup_failed (attempts 1) Exception('Setup step 2 failed: _execute_with_verification_setup - Command verification failed: {\n  "error": "bash: line 1: pulseaudio: command not found\\nbash: line 1: pactl: command not found\\nbash: line 1: pactl: command not found\\nbash: line 1: pactl: command not found\\nbash: line 1: pactl:
- multi_apps-a82b78bb-7fde-4cb3-94a4-035baf10bcf0: setup_failed (attempts 1) Exception("Setup step 5 failed: _open_setup - Failed to open file '/home/user/Desktop/2206.08853.pdf'. An error occurred while trying to send the request or the server responded with an error: 500 Server Error: INTERNAL SERVER ERROR for url: http://127.0.0.1:5000/setup/open_file")
- os-4127319a-8b79-4410-b58a-7a151e15f3d7: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- multi_apps-2b9493d7-49b8-493a-a71b-56cd1f4d6908: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- multi_apps-2c9fc0de-3ee7-45e1-a5df-c86206ad78b5: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- multi_apps-f5c13cdd-205c-4719-a562-348ae5cd1d91: setup_failed (attempts 1) Exception('Setup step 5 failed: _launch_setup - Failed to launch application: HTTP 500')
- libreoffice_calc-30e3e107-1cfb-46ee-a755-2cd080d7ba6a: eval_error (attempts 1) TypeError("'NoneType' object is not subscriptable")
