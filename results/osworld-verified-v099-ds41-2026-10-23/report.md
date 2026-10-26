# OSWorld-Verified report

- tasks expected 361, finished rows 360, valid 332, infra 28 {'eval_error': 1, 'relay_error': 4, 'setup_failed': 23}
- **success 185/361 = 51.25 %** (official-style: every expected task counts, infra/missing = 0); 185/332 = 55.72 % of valid runs; score sum 192.24
- stops {'agent': 308, 'max_turns': 4, 'timeout': 20}; turns mean 31.0 / median 25.0; wall mean 684.0 s / median 344.8 s; total 65.19 h
- tokens: input 16,880,098 + cache read 417,426,560, output 3,048,146; screenshots returned 3014; tool calls 10860
- request_human calls 20 (human=deny); FAIL answers 35
- gate: {'actions': 10054, 'executed': 9719, 'interventions': 335, 'human_events': 136, 'loop_notes': 21, 'loop_refused': 10}
- reviewer (System One): {'calls': 125, 'decisions': {'deny': 81, 'allow': 44}, 'ms_mean': 945.3, 'ms_median': 696.9}

| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |
|---|---|---|---|---|---|---|
| feasibility | 332 | 332 | 1970 | 7 | 68621.8 | 56057.6 |
| planner | 332 | 332 | 1104 | 3 | None | None |
| verifier | 299 | 313 | 2223 | 15 | 70520.3 | 58842.6 |
| judge | 0 | 0 | 0 | 0 | None | None |
| search | 8 | 9 | 9 | 0 | 17036.1 | 18235.6 |
| compact | 266 | 957 | 957 | 1 | 10311.7 | 9288.0 |
| grounder | 11 | 13 | 13 | 0 | 9324.8 | 6493.7 |

(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)

| domain | tasks | valid | success | rate (of tasks) | score sum | infra |
|---|---|---|---|---|---|---|
| libreoffice_impress | 47 | 47 | 12 | 25.5 % | 12.93 | 0 |
| os | 24 | 21 | 12 | 50.0 % | 12.0 | 3 |
| multi_apps | 92 | 71 | 34 | 37.0 % | 37.49 | 21 |
| libreoffice_writer | 23 | 22 | 17 | 73.9 % | 17.0 | 1 |
| vs_code | 23 | 23 | 19 | 82.6 % | 19.0 | 0 |
| libreoffice_calc | 47 | 45 | 39 | 83.0 % | 39.0 | 2 |
| vlc | 17 | 16 | 9 | 52.9 % | 10.89 | 1 |
| chrome | 46 | 46 | 15 | 32.6 % | 15.93 | 0 |
| gimp | 26 | 26 | 16 | 61.5 % | 16.0 | 0 |
| thunderbird | 15 | 15 | 12 | 80.0 % | 12.0 | 0 |

tools: {'run_command': 2664, 'run_actions': 2087, 'get_window_state': 1754, 'click': 1313, 'set_plan': 728, 'press_key': 637, 'list_windows': 629, 'inspect': 401, 'check_done': 314, 'scroll': 113, 'type_text': 108, 'list_apps': 85, 'request_human': 20, 'update_plan': 1, 'mcp__gui_run_actions': 1, 'mcp__gui_set_plan': 1, 'mcp__gui_inspect': 1, 'mcp__gui_press_key': 1, 'run_commands': 1, 'close_window': 1}

## infra rows

- libreoffice_calc-30e3e107-1cfb-46ee-a755-2cd080d7ba6a: eval_error (attempts 1) TypeError("'NoneType' object is not subscriptable")
- libreoffice_writer-e246f6d8-78d7-44ac-b668-fcf47946cb50: relay_error (attempts 1) 503: {"code":"SERVICE_BUSY","message":"服务繁忙，请稍后重试","traceId":"trace_b2875c7b-3153-415d-81e0-3a239d84a6ec"}
- os-28cc3b7e-b194-4bc9-8353-d04c0f4d56d2: setup_failed (attempts 1) Exception('Setup step 2 failed: _execute_with_verification_setup - Command verification failed: {\n  "error": "bash: line 1: pulseaudio: command not found\\nbash: line 1: pactl: command not found\\nbash: line 1: pactl: command not found\\nbash: line 1: pactl: command not found\\nbash: line 1: pactl:
- multi_apps-a82b78bb-7fde-4cb3-94a4-035baf10bcf0: setup_failed (attempts 1) Exception("Setup step 5 failed: _open_setup - Failed to open file '/home/user/Desktop/2206.08853.pdf'. An error occurred while trying to send the request or the server responded with an error: 500 Server Error: INTERNAL SERVER ERROR for url: http://127.0.0.1:5000/setup/open_file")
- os-4127319a-8b79-4410-b58a-7a151e15f3d7: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- multi_apps-2b9493d7-49b8-493a-a71b-56cd1f4d6908: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- multi_apps-2c9fc0de-3ee7-45e1-a5df-c86206ad78b5: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- multi_apps-f5c13cdd-205c-4719-a562-348ae5cd1d91: setup_failed (attempts 1) Exception('Setup step 5 failed: _launch_setup - Failed to launch application: HTTP 500')
- multi_apps-185f29bd-5da0-40a6-b69c-ba7f4e0324ef: setup_failed (attempts 1) Exception("Setup step 4 failed: _open_setup - Failed to open file '/home/user/Desktop/review_template.pdf'. An error occurred while trying to send the request or the server responded with an error: 500 Server Error: INTERNAL SERVER ERROR for url: http://127.0.0.1:5000/setup/open_file")
- multi_apps-5990457f-2adb-467b-a4af-5c857c92d762: setup_failed (attempts 1) Exception('Setup step 4 failed: _launch_setup - Failed to launch application: HTTP 500')
- os-3ce045a0-877b-42aa-8d2c-b4a863336ab8: relay_error (attempts 1) 502 <html>
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
- multi_apps-82e3c869-49f6-4305-a7ce-f3e64a0618e7: relay_error (attempts 1) Request timed out.
- multi_apps-aceb0368-56b8-4073-b70e-3dc9aee184e0: setup_failed (attempts 1) Exception('Setup step 5 failed: _launch_setup - Failed to launch application: HTTP 500')
- multi_apps-b5062e3e-641c-4e3a-907b-ac864d2e7652: setup_failed (attempts 1) Exception('Setup step 3 failed: _launch_setup - Failed to launch application: HTTP 500')
- vlc-386dbd0e-0241-4a0a-b6a2-6704fba26b1c: setup_failed (attempts 1) Exception("Setup step 4 failed: _open_setup - Failed to open file '/home/user/Desktop/lecture.pdf'. An error occurred while trying to send the request or the server responded with an error: 500 Server Error: INTERNAL SERVER ERROR for url: http://127.0.0.1:5000/setup/open_file")
- multi_apps-ee9a3c83-f437-4879-8918-be5efbb9fac7: setup_failed (attempts 1) Exception('Setup step 4 failed: _execute_setup - Setup command request failed: HTTP 500')
- libreoffice_calc-2bd59342-0664-4ccb-ba87-79379096cc08: relay_error (attempts 1) 503: {"code":"LITELLM_UNAVAILABLE","message":"模型服务暂时不可用，请稍后重试","traceId":"trace_06b17e68-c6d5-4c7a-b6f3-a06dca5b09fd"}
- multi_apps-0e5303d4-8820-42f6-b18d-daf7e633de21: setup_failed (attempts 1) Exception('Setup step 6 failed: _launch_setup - Failed to launch application: HTTP 500')
- multi_apps-3f05f3b9-29ba-4b6b-95aa-2204697ffc06: setup_failed (attempts 1) Exception('Setup step 9 failed: _launch_setup - Failed to launch application: HTTP 500')
- multi_apps-5df7b33a-9f77-4101-823e-02f863e1c1ae: setup_failed (attempts 1) Exception('Setup step 3 failed: _launch_setup - Failed to launch application: HTTP 500')
- multi_apps-788b3701-3ec9-4b67-b679-418bfa726c22: setup_failed (attempts 1) Exception('Setup step 5 failed: _launch_setup - Failed to launch application: HTTP 500')
- multi_apps-74d5859f-ed66-4d3e-aa0e-93d7a592ce41: setup_failed (attempts 1) Exception('Setup step 5 failed: _launch_setup - Failed to launch application: HTTP 500')
