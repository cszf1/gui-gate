# OSWorld-Verified report

- tasks expected 361, finished rows 209, valid 191, infra 18 {'relay_error': 3, 'setup_failed': 14, 'eval_error': 1}
- **success 106/361 = 29.36 %** (official-style: every expected task counts, infra/missing = 0); 106/191 = 55.5 % of valid runs; score sum 111.35
- stops {'agent': 176, 'max_turns': 3, 'timeout': 12}; turns mean 30.7 / median 25; wall mean 694.7 s / median 335.4 s; total 38.71 h
- tokens: input 9,711,236 + cache read 239,825,280, output 1,733,787; screenshots returned 1767; tool calls 6213
- request_human calls 10 (human=deny); FAIL answers 18
- gate: {'actions': 5966, 'executed': 5801, 'interventions': 165, 'human_events': 62, 'loop_notes': 10, 'loop_refused': 6}
- reviewer (System One): {'calls': 64, 'decisions': {'deny': 38, 'allow': 26}, 'ms_mean': 913.8, 'ms_median': 700.0}

| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |
|---|---|---|---|---|---|---|
| feasibility | 191 | 191 | 1115 | 5 | 67291.4 | 55448.6 |
| planner | 191 | 191 | 640 | 3 | None | None |
| verifier | 172 | 182 | 1249 | 10 | 69381.1 | 54342.4 |
| judge | 0 | 0 | 0 | 0 | None | None |
| search | 4 | 4 | 4 | 0 | 10413.8 | 9443.6 |
| compact | 155 | 566 | 566 | 1 | 10056.0 | 9100.5 |
| grounder | 7 | 8 | 8 | 0 | 10044.1 | 5146.4 |

(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)

| domain | tasks | valid | success | rate (of tasks) | score sum | infra |
|---|---|---|---|---|---|---|
| libreoffice_impress | 25 | 25 | 5 | 20.0 % | 5.93 | 0 |
| os | 14 | 11 | 8 | 57.1 % | 8.0 | 3 |
| multi_apps | 56 | 43 | 18 | 32.1 % | 21.49 | 13 |
| libreoffice_writer | 13 | 12 | 9 | 69.2 % | 9.0 | 1 |
| vs_code | 17 | 17 | 14 | 82.4 % | 14.0 | 0 |
| libreoffice_calc | 33 | 32 | 28 | 84.8 % | 28.0 | 1 |
| vlc | 8 | 8 | 6 | 75.0 % | 6.0 | 0 |
| chrome | 23 | 23 | 7 | 30.4 % | 7.93 | 0 |
| gimp | 13 | 13 | 6 | 46.2 % | 6.0 | 0 |
| thunderbird | 7 | 7 | 5 | 71.4 % | 5.0 | 0 |

tools: {'run_command': 1560, 'run_actions': 1198, 'get_window_state': 972, 'click': 782, 'set_plan': 415, 'list_windows': 358, 'press_key': 343, 'inspect': 226, 'check_done': 183, 'type_text': 61, 'scroll': 59, 'list_apps': 43, 'request_human': 10, 'update_plan': 1, 'mcp__gui_run_actions': 1, 'mcp__gui_set_plan': 1}

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
