# OSWorld-Verified report

- tasks expected 361, finished rows 200, valid 183, infra 17 {'relay_error': 2, 'setup_failed': 14, 'eval_error': 1}
- **success 102/361 = 28.25 %** (official-style: every expected task counts, infra/missing = 0); 102/183 = 55.74 % of valid runs; score sum 107.35
- stops {'agent': 171, 'max_turns': 3, 'timeout': 9}; turns mean 30.6 / median 25; wall mean 651.7 s / median 334.6 s; total 34.16 h
- tokens: input 9,288,353 + cache read 229,312,000, output 1,657,899; screenshots returned 1690; tool calls 5923
- request_human calls 10 (human=deny); FAIL answers 18
- gate: {'actions': 5697, 'executed': 5540, 'interventions': 157, 'human_events': 58, 'loop_notes': 10, 'loop_refused': 6}
- reviewer (System One): {'calls': 62, 'decisions': {'deny': 36, 'allow': 26}, 'ms_mean': 918.7, 'ms_median': 700.0}

| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |
|---|---|---|---|---|---|---|
| feasibility | 183 | 183 | 1068 | 4 | 65631.1 | 54842.6 |
| planner | 183 | 183 | 606 | 3 | None | None |
| verifier | 166 | 176 | 1226 | 8 | 70048.3 | 54862.1 |
| judge | 0 | 0 | 0 | 0 | None | None |
| search | 4 | 4 | 4 | 0 | 10413.8 | 9443.6 |
| compact | 149 | 538 | 538 | 1 | 9887.0 | 8870.0 |
| grounder | 7 | 8 | 8 | 0 | 10044.1 | 5146.4 |

(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)

| domain | tasks | valid | success | rate (of tasks) | score sum | infra |
|---|---|---|---|---|---|---|
| libreoffice_impress | 25 | 25 | 5 | 20.0 % | 5.93 | 0 |
| os | 13 | 10 | 7 | 53.8 % | 7.0 | 3 |
| multi_apps | 54 | 42 | 18 | 33.3 % | 21.49 | 12 |
| libreoffice_writer | 12 | 11 | 8 | 66.7 % | 8.0 | 1 |
| vs_code | 16 | 16 | 14 | 87.5 % | 14.0 | 0 |
| libreoffice_calc | 33 | 32 | 28 | 84.8 % | 28.0 | 1 |
| vlc | 8 | 8 | 6 | 75.0 % | 6.0 | 0 |
| chrome | 22 | 22 | 6 | 27.3 % | 6.93 | 0 |
| gimp | 11 | 11 | 6 | 54.5 % | 6.0 | 0 |
| thunderbird | 6 | 6 | 4 | 66.7 % | 4.0 | 0 |

tools: {'run_command': 1474, 'run_actions': 1154, 'get_window_state': 931, 'click': 741, 'set_plan': 399, 'list_windows': 342, 'press_key': 322, 'inspect': 218, 'check_done': 176, 'scroll': 58, 'type_text': 56, 'list_apps': 39, 'request_human': 10, 'update_plan': 1, 'mcp__gui_run_actions': 1, 'mcp__gui_set_plan': 1}

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
