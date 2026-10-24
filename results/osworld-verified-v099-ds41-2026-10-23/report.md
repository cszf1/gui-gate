# OSWorld-Verified report

- tasks expected 361, finished rows 189, valid 172, infra 17 {'relay_error': 2, 'setup_failed': 14, 'eval_error': 1}
- **success 94/361 = 26.04 %** (official-style: every expected task counts, infra/missing = 0); 94/172 = 54.65 % of valid runs; score sum 99.35
- stops {'agent': 162, 'max_turns': 3, 'timeout': 7}; turns mean 31.1 / median 25.0; wall mean 619.1 s / median 324.6 s; total 30.61 h
- tokens: input 8,869,351 + cache read 222,899,200, output 1,592,302; screenshots returned 1612; tool calls 5659
- request_human calls 10 (human=deny); FAIL answers 17
- gate: {'actions': 5413, 'executed': 5260, 'interventions': 153, 'human_events': 57, 'loop_notes': 10, 'loop_refused': 6}
- reviewer (System One): {'calls': 59, 'decisions': {'deny': 35, 'allow': 24}, 'ms_mean': 922.7, 'ms_median': 696.0}

| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |
|---|---|---|---|---|---|---|
| feasibility | 172 | 172 | 1006 | 4 | 65441.8 | 53562.3 |
| planner | 172 | 172 | 586 | 3 | None | None |
| verifier | 158 | 167 | 1178 | 7 | 70749.1 | 55321.5 |
| judge | 0 | 0 | 0 | 0 | None | None |
| search | 4 | 4 | 4 | 0 | 10413.8 | 9443.6 |
| compact | 139 | 513 | 513 | 1 | 9804.1 | 8846.0 |
| grounder | 7 | 8 | 8 | 0 | 10044.1 | 5146.4 |

(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)

| domain | tasks | valid | success | rate (of tasks) | score sum | infra |
|---|---|---|---|---|---|---|
| libreoffice_impress | 24 | 24 | 5 | 20.8 % | 5.93 | 0 |
| os | 13 | 10 | 7 | 53.8 % | 7.0 | 3 |
| multi_apps | 51 | 39 | 17 | 33.3 % | 20.49 | 12 |
| libreoffice_writer | 11 | 10 | 7 | 63.6 % | 7.0 | 1 |
| vs_code | 13 | 13 | 11 | 84.6 % | 11.0 | 0 |
| libreoffice_calc | 32 | 31 | 27 | 84.4 % | 27.0 | 1 |
| vlc | 7 | 7 | 5 | 71.4 % | 5.0 | 0 |
| chrome | 21 | 21 | 5 | 23.8 % | 5.93 | 0 |
| gimp | 11 | 11 | 6 | 54.5 % | 6.0 | 0 |
| thunderbird | 6 | 6 | 4 | 66.7 % | 4.0 | 0 |

tools: {'run_command': 1427, 'run_actions': 1106, 'get_window_state': 889, 'click': 702, 'set_plan': 375, 'list_windows': 321, 'press_key': 304, 'inspect': 214, 'check_done': 167, 'type_text': 54, 'scroll': 51, 'list_apps': 37, 'request_human': 10, 'update_plan': 1, 'mcp__gui_run_actions': 1}

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
