# OSWorld-Verified report

- tasks expected 361, finished rows 246, valid 226, infra 20 {'relay_error': 3, 'setup_failed': 16, 'eval_error': 1}
- **success 122/361 = 33.8 %** (official-style: every expected task counts, infra/missing = 0); 122/226 = 53.98 % of valid runs; score sum 128.33
- stops {'agent': 211, 'max_turns': 3, 'timeout': 12}; turns mean 30.6 / median 25.0; wall mean 639.9 s / median 325.6 s; total 42.02 h
- tokens: input 11,464,479 + cache read 279,179,136, output 2,036,907; screenshots returned 2067; tool calls 7316
- request_human calls 10 (human=deny); FAIL answers 24
- gate: {'actions': 6939, 'executed': 6726, 'interventions': 213, 'human_events': 73, 'loop_notes': 13, 'loop_refused': 7}
- reviewer (System One): {'calls': 79, 'decisions': {'deny': 53, 'allow': 26}, 'ms_mean': 882.0, 'ms_median': 697.4}

| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |
|---|---|---|---|---|---|---|
| feasibility | 226 | 226 | 1330 | 5 | 67040.8 | 55966.1 |
| planner | 226 | 226 | 772 | 3 | None | None |
| verifier | 205 | 216 | 1500 | 12 | 69819.3 | 53814.9 |
| judge | 0 | 0 | 0 | 0 | None | None |
| search | 4 | 4 | 4 | 0 | 10413.8 | 9443.6 |
| compact | 183 | 660 | 660 | 1 | 9975.9 | 9100.5 |
| grounder | 8 | 9 | 9 | 0 | 9530.7 | 5423.4 |

(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)

| domain | tasks | valid | success | rate (of tasks) | score sum | infra |
|---|---|---|---|---|---|---|
| libreoffice_impress | 27 | 27 | 5 | 18.5 % | 5.93 | 0 |
| os | 17 | 14 | 8 | 47.1 % | 8.0 | 3 |
| multi_apps | 65 | 50 | 22 | 33.8 % | 25.49 | 15 |
| libreoffice_writer | 16 | 15 | 12 | 75.0 % | 12.0 | 1 |
| vs_code | 19 | 19 | 15 | 78.9 % | 15.0 | 0 |
| libreoffice_calc | 35 | 34 | 29 | 82.9 % | 29.0 | 1 |
| vlc | 11 | 11 | 8 | 72.7 % | 8.99 | 0 |
| chrome | 33 | 33 | 12 | 36.4 % | 12.93 | 0 |
| gimp | 15 | 15 | 6 | 40.0 % | 6.0 | 0 |
| thunderbird | 8 | 8 | 5 | 62.5 % | 5.0 | 0 |

tools: {'run_command': 1867, 'run_actions': 1378, 'get_window_state': 1146, 'click': 898, 'set_plan': 495, 'list_windows': 421, 'press_key': 396, 'inspect': 266, 'check_done': 217, 'scroll': 92, 'type_text': 71, 'list_apps': 54, 'request_human': 10, 'update_plan': 1, 'mcp__gui_run_actions': 1, 'mcp__gui_set_plan': 1, 'mcp__gui_inspect': 1, 'mcp__gui_press_key': 1}

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
