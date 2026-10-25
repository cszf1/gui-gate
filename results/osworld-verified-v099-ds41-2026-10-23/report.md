# OSWorld-Verified report

- tasks expected 361, finished rows 331, valid 304, infra 27 {'relay_error': 4, 'setup_failed': 22, 'eval_error': 1}
- **success 167/361 = 46.26 %** (official-style: every expected task counts, infra/missing = 0); 167/304 = 54.93 % of valid runs; score sum 174.24
- stops {'agent': 282, 'max_turns': 3, 'timeout': 19}; turns mean 30.4 / median 25.0; wall mean 686.3 s / median 342.2 s; total 60.07 h
- tokens: input 15,266,268 + cache read 368,253,952, output 2,715,174; screenshots returned 2713; tool calls 9763
- request_human calls 20 (human=deny); FAIL answers 34
- gate: {'actions': 9164, 'executed': 8867, 'interventions': 297, 'human_events': 122, 'loop_notes': 18, 'loop_refused': 9}
- reviewer (System One): {'calls': 108, 'decisions': {'deny': 70, 'allow': 38}, 'ms_mean': 922.8, 'ms_median': 697.1}

| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |
|---|---|---|---|---|---|---|
| feasibility | 304 | 304 | 1791 | 7 | 68153.9 | 55949.5 |
| planner | 304 | 304 | 1020 | 3 | None | None |
| verifier | 273 | 287 | 2031 | 15 | 71112.6 | 58339.2 |
| judge | 0 | 0 | 0 | 0 | None | None |
| search | 6 | 7 | 7 | 0 | 17118.6 | 18235.6 |
| compact | 244 | 868 | 868 | 1 | 10193.9 | 9298.5 |
| grounder | 11 | 13 | 13 | 0 | 9324.8 | 6493.7 |

(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)

| domain | tasks | valid | success | rate (of tasks) | score sum | infra |
|---|---|---|---|---|---|---|
| libreoffice_impress | 42 | 42 | 10 | 23.8 % | 10.93 | 0 |
| os | 22 | 19 | 10 | 45.5 % | 10.0 | 3 |
| multi_apps | 83 | 63 | 30 | 36.1 % | 33.49 | 20 |
| libreoffice_writer | 21 | 20 | 15 | 71.4 % | 15.0 | 1 |
| vs_code | 22 | 22 | 18 | 81.8 % | 18.0 | 0 |
| libreoffice_calc | 45 | 43 | 37 | 82.2 % | 37.0 | 2 |
| vlc | 17 | 16 | 9 | 52.9 % | 10.89 | 1 |
| chrome | 44 | 44 | 15 | 34.1 % | 15.93 | 0 |
| gimp | 22 | 22 | 13 | 59.1 % | 13.0 | 0 |
| thunderbird | 13 | 13 | 10 | 76.9 % | 10.0 | 0 |

tools: {'run_command': 2382, 'run_actions': 1883, 'get_window_state': 1562, 'click': 1185, 'set_plan': 666, 'press_key': 572, 'list_windows': 568, 'inspect': 339, 'check_done': 288, 'scroll': 110, 'type_text': 104, 'list_apps': 78, 'request_human': 20, 'update_plan': 1, 'mcp__gui_run_actions': 1, 'mcp__gui_set_plan': 1, 'mcp__gui_inspect': 1, 'mcp__gui_press_key': 1, 'run_commands': 1}

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
- multi_apps-ee9a3c83-f437-4879-8918-be5efbb9fac7: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- libreoffice_calc-2bd59342-0664-4ccb-ba87-79379096cc08: relay_error (attempts 3) 503: {"code":"LITELLM_UNAVAILABLE","message":"模型服务暂时不可用，请稍后重试","traceId":"trace_06b17e68-c6d5-4c7a-b6f3-a06dca5b09fd"}
- multi_apps-0e5303d4-8820-42f6-b18d-daf7e633de21: setup_failed (attempts 1) Exception('Setup step 6 failed: _launch_setup - Failed to launch application: HTTP 500')
- multi_apps-3f05f3b9-29ba-4b6b-95aa-2204697ffc06: setup_failed (attempts 1) Exception('Setup step 9 failed: _launch_setup - Failed to launch application: HTTP 500')
- multi_apps-5df7b33a-9f77-4101-823e-02f863e1c1ae: setup_failed (attempts 1) Exception('Setup step 3 failed: _launch_setup - Failed to launch application: HTTP 500')
- multi_apps-788b3701-3ec9-4b67-b679-418bfa726c22: setup_failed (attempts 1) Exception('Setup step 5 failed: _launch_setup - Failed to launch application: HTTP 500')
