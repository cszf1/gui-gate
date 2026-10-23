# OSWorld-Verified report

- tasks expected 361, finished rows 94, valid 83, infra 11 {'relay_error': 2, 'setup_failed': 8, 'eval_error': 1}
- **success 46/361 = 12.74 %** (official-style: every expected task counts, infra/missing = 0); 46/83 = 55.42 % of valid runs; score sum 47.75
- stops {'agent': 80, 'max_turns': 1, 'timeout': 2}; turns mean 30.7 / median 24; wall mean 505.2 s / median 280.3 s; total 12.68 h
- tokens: input 4,276,095 + cache read 106,796,800, output 730,459; screenshots returned 774; tool calls 2724
- request_human calls 6 (human=deny); FAIL answers 6
- gate: {'actions': 2455, 'executed': 2387, 'interventions': 68, 'human_events': 30, 'loop_notes': 2, 'loop_refused': 1}
- reviewer (System One): {'calls': 36, 'decisions': {'deny': 18, 'allow': 18}, 'ms_mean': 876.6, 'ms_median': 693.9}

| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |
|---|---|---|---|---|---|---|
| feasibility | 83 | 83 | 495 | 2 | 65766.9 | 51031.4 |
| planner | 83 | 83 | 299 | 3 | None | None |
| verifier | 80 | 85 | 602 | 4 | 63067.4 | 52137.3 |
| judge | 0 | 0 | 0 | 0 | None | None |
| search | 1 | 1 | 1 | 0 | 18235.6 | 18235.6 |
| compact | 67 | 235 | 235 | 0 | 9092.6 | 8048.0 |
| grounder | 3 | 4 | 4 | 0 | 7050.5 | 3728.6 |

(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)

| domain | tasks | valid | success | rate (of tasks) | score sum | infra |
|---|---|---|---|---|---|---|
| libreoffice_impress | 8 | 8 | 2 | 25.0 % | 2.0 | 0 |
| os | 5 | 2 | 2 | 40.0 % | 2.0 | 3 |
| multi_apps | 28 | 22 | 8 | 28.6 % | 9.75 | 6 |
| libreoffice_writer | 5 | 4 | 3 | 60.0 % | 3.0 | 1 |
| vs_code | 7 | 7 | 6 | 85.7 % | 6.0 | 0 |
| libreoffice_calc | 15 | 14 | 14 | 93.3 % | 14.0 | 1 |
| vlc | 6 | 6 | 4 | 66.7 % | 4.0 | 0 |
| chrome | 10 | 10 | 2 | 20.0 % | 2.0 | 0 |
| gimp | 6 | 6 | 3 | 50.0 % | 3.0 | 0 |
| thunderbird | 4 | 4 | 2 | 50.0 % | 2.0 | 0 |

tools: {'run_command': 781, 'run_actions': 470, 'get_window_state': 402, 'click': 328, 'set_plan': 189, 'list_windows': 147, 'inspect': 132, 'press_key': 112, 'check_done': 85, 'type_text': 31, 'list_apps': 20, 'scroll': 20, 'request_human': 6, 'update_plan': 1}

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

