# OSWorld-Verified report

- tasks expected 361, finished rows 257, valid 236, infra 21 {'relay_error': 3, 'setup_failed': 17, 'eval_error': 1}
- **success 129/361 = 35.73 %** (official-style: every expected task counts, infra/missing = 0); 129/236 = 54.66 % of valid runs; score sum 135.33
- stops {'agent': 221, 'max_turns': 3, 'timeout': 12}; turns mean 30.7 / median 25.0; wall mean 644.9 s / median 333.8 s; total 44.12 h
- tokens: input 11,948,439 + cache read 289,983,232, output 2,129,446; screenshots returned 2137; tool calls 7665
- request_human calls 12 (human=deny); FAIL answers 26
- gate: {'actions': 7303, 'executed': 7078, 'interventions': 225, 'human_events': 81, 'loop_notes': 14, 'loop_refused': 8}
- reviewer (System One): {'calls': 87, 'decisions': {'deny': 56, 'allow': 31}, 'ms_mean': 875.8, 'ms_median': 696.9}

| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |
|---|---|---|---|---|---|---|
| feasibility | 236 | 236 | 1389 | 5 | 67009.3 | 55966.1 |
| planner | 236 | 236 | 809 | 3 | None | None |
| verifier | 213 | 224 | 1563 | 12 | 70547.5 | 54342.4 |
| judge | 0 | 0 | 0 | 0 | None | None |
| search | 4 | 4 | 4 | 0 | 10413.8 | 9443.6 |
| compact | 192 | 693 | 693 | 1 | 10088.8 | 9240.0 |
| grounder | 8 | 9 | 9 | 0 | 9530.7 | 5423.4 |

(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)

| domain | tasks | valid | success | rate (of tasks) | score sum | infra |
|---|---|---|---|---|---|---|
| libreoffice_impress | 28 | 28 | 5 | 17.9 % | 5.93 | 0 |
| os | 17 | 14 | 8 | 47.1 % | 8.0 | 3 |
| multi_apps | 67 | 52 | 23 | 34.3 % | 26.49 | 15 |
| libreoffice_writer | 17 | 16 | 13 | 76.5 % | 13.0 | 1 |
| vs_code | 20 | 20 | 16 | 80.0 % | 16.0 | 0 |
| libreoffice_calc | 36 | 35 | 30 | 83.3 % | 30.0 | 1 |
| vlc | 12 | 11 | 8 | 66.7 % | 8.99 | 1 |
| chrome | 34 | 34 | 12 | 35.3 % | 12.93 | 0 |
| gimp | 17 | 17 | 8 | 47.1 % | 8.0 | 0 |
| thunderbird | 9 | 9 | 6 | 66.7 % | 6.0 | 0 |

tools: {'run_command': 1956, 'run_actions': 1431, 'get_window_state': 1205, 'click': 934, 'set_plan': 516, 'list_windows': 441, 'press_key': 437, 'inspect': 278, 'check_done': 225, 'scroll': 92, 'type_text': 76, 'list_apps': 57, 'request_human': 12, 'update_plan': 1, 'mcp__gui_run_actions': 1, 'mcp__gui_set_plan': 1, 'mcp__gui_inspect': 1, 'mcp__gui_press_key': 1}

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
