# OSWorld-Verified report

- tasks expected 361, finished rows 100, valid 88, infra 12 {'relay_error': 2, 'setup_failed': 9, 'eval_error': 1}
- **success 47/361 = 13.02 %** (official-style: every expected task counts, infra/missing = 0); 47/88 = 53.41 % of valid runs; score sum 49.49
- stops {'agent': 83, 'max_turns': 1, 'timeout': 4}; turns mean 32.5 / median 25.0; wall mean 606.5 s / median 289.3 s; total 15.86 h
- tokens: input 4,690,852 + cache read 122,897,792, output 834,845; screenshots returned 848; tool calls 3049
- request_human calls 6 (human=deny); FAIL answers 6
- gate: {'actions': 2776, 'executed': 2697, 'interventions': 79, 'human_events': 32, 'loop_notes': 3, 'loop_refused': 1}
- reviewer (System One): {'calls': 41, 'decisions': {'deny': 23, 'allow': 18}, 'ms_mean': 893.5, 'ms_median': 695.8}

| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |
|---|---|---|---|---|---|---|
| feasibility | 88 | 88 | 524 | 2 | 65386.9 | 50437.9 |
| planner | 88 | 88 | 307 | 3 | None | None |
| verifier | 83 | 88 | 617 | 4 | 61801.6 | 51166.9 |
| judge | 0 | 0 | 0 | 0 | None | None |
| search | 2 | 2 | 2 | 0 | 14358.5 | 14358.5 |
| compact | 72 | 268 | 268 | 0 | 9385.9 | 8395.0 |
| grounder | 4 | 5 | 5 | 0 | 6964.5 | 3799.0 |

(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)

| domain | tasks | valid | success | rate (of tasks) | score sum | infra |
|---|---|---|---|---|---|---|
| libreoffice_impress | 10 | 10 | 3 | 30.0 % | 3.0 | 0 |
| os | 5 | 2 | 2 | 40.0 % | 2.0 | 3 |
| multi_apps | 31 | 24 | 8 | 25.8 % | 10.49 | 7 |
| libreoffice_writer | 5 | 4 | 3 | 60.0 % | 3.0 | 1 |
| vs_code | 7 | 7 | 6 | 85.7 % | 6.0 | 0 |
| libreoffice_calc | 15 | 14 | 14 | 93.3 % | 14.0 | 1 |
| vlc | 6 | 6 | 4 | 66.7 % | 4.0 | 0 |
| chrome | 10 | 10 | 2 | 20.0 % | 2.0 | 0 |
| gimp | 7 | 7 | 3 | 42.9 % | 3.0 | 0 |
| thunderbird | 4 | 4 | 2 | 50.0 % | 2.0 | 0 |

tools: {'run_command': 859, 'run_actions': 526, 'get_window_state': 459, 'click': 376, 'set_plan': 198, 'list_windows': 174, 'inspect': 148, 'press_key': 139, 'check_done': 88, 'type_text': 32, 'scroll': 22, 'list_apps': 21, 'request_human': 6, 'update_plan': 1}

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
