# OSWorld-Verified report

- tasks expected 361, finished rows 155, valid 141, infra 14 {'relay_error': 2, 'setup_failed': 11, 'eval_error': 1}
- **success 79/361 = 21.88 %** (official-style: every expected task counts, infra/missing = 0); 79/141 = 56.03 % of valid runs; score sum 82.42
- stops {'agent': 134, 'max_turns': 2, 'timeout': 5}; turns mean 32.0 / median 25; wall mean 575.6 s / median 293.6 s; total 23.57 h
- tokens: input 7,558,897 + cache read 194,299,136, output 1,344,714; screenshots returned 1371; tool calls 4772
- request_human calls 6 (human=deny); FAIL answers 13
- gate: {'actions': 4535, 'executed': 4413, 'interventions': 122, 'human_events': 43, 'loop_notes': 7, 'loop_refused': 2}
- reviewer (System One): {'calls': 55, 'decisions': {'deny': 32, 'allow': 23}, 'ms_mean': 918.7, 'ms_median': 695.8}

| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |
|---|---|---|---|---|---|---|
| feasibility | 141 | 141 | 834 | 3 | 63557.3 | 51335.2 |
| planner | 141 | 141 | 470 | 3 | None | None |
| verifier | 130 | 138 | 968 | 5 | 66484.2 | 51773.7 |
| judge | 0 | 0 | 0 | 0 | None | None |
| search | 3 | 3 | 3 | 0 | 11083.1 | 10481.4 |
| compact | 113 | 437 | 437 | 1 | 9698.5 | 8792.0 |
| grounder | 6 | 7 | 7 | 0 | 10551.3 | 3799.0 |

(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)

| domain | tasks | valid | success | rate (of tasks) | score sum | infra |
|---|---|---|---|---|---|---|
| libreoffice_impress | 18 | 18 | 4 | 22.2 % | 4.93 | 0 |
| os | 11 | 8 | 6 | 54.5 % | 6.0 | 3 |
| multi_apps | 42 | 33 | 15 | 35.7 % | 17.49 | 9 |
| libreoffice_writer | 9 | 8 | 6 | 66.7 % | 6.0 | 1 |
| vs_code | 11 | 11 | 9 | 81.8 % | 9.0 | 0 |
| libreoffice_calc | 28 | 27 | 24 | 85.7 % | 24.0 | 1 |
| vlc | 6 | 6 | 4 | 66.7 % | 4.0 | 0 |
| chrome | 16 | 16 | 3 | 18.8 % | 3.0 | 0 |
| gimp | 9 | 9 | 5 | 55.6 % | 5.0 | 0 |
| thunderbird | 5 | 5 | 3 | 60.0 % | 3.0 | 0 |

tools: {'run_command': 1235, 'run_actions': 937, 'get_window_state': 744, 'click': 584, 'set_plan': 308, 'list_windows': 256, 'press_key': 251, 'inspect': 194, 'check_done': 138, 'type_text': 53, 'scroll': 34, 'list_apps': 30, 'request_human': 6, 'update_plan': 1, 'mcp__gui_run_actions': 1}

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
