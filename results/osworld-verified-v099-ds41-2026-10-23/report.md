# OSWorld-Verified report

> **How to read this number (added 2026-10-25, external review):** 137/256 = 53.5 % of *valid* runs is a
> **custom-environment partial result**, not an official OSWorld-Verified score. Wilson 95 % CI for 137/256:
> **47.4 %–59.5 %**, ignoring task correlation and the selection of valid runs. The run used
> OSWorld's tasks and evaluators natively on Debian 13 / XFCE 1440x900 with Chromium, LibreOffice 25.2 and other app
> versions (no VM, no snapshot revert), stopped before all 361 tasks finished, excluded infra failures from the
> valid denominator, and had no direct baseline. Over the planned list it is 137/361 = 38.0 % (Wilson 33.1 %–43.1 %).
> Do not compare it with leaderboard numbers. Fair comparisons: bench/fair_compare.py.

- tasks expected 361, finished rows 279, valid 256, infra 23 {'relay_error': 4, 'setup_failed': 18, 'eval_error': 1}
- **success 137/361 = 37.95 %** (official-style: every expected task counts, infra/missing = 0); 137/256 = 53.52 % of valid runs; score sum 143.33
- stops {'agent': 237, 'max_turns': 3, 'timeout': 16}; turns mean 30.3 / median 24.0; wall mean 690.1 s / median 340.2 s; total 51.19 h
- tokens: input 12,882,114 + cache read 306,507,136, output 2,269,475; screenshots returned 2276; tool calls 8202
- request_human calls 14 (human=deny); FAIL answers 28
- gate: {'actions': 7759, 'executed': 7517, 'interventions': 242, 'human_events': 89, 'loop_notes': 15, 'loop_refused': 8}
- reviewer (System One): {'calls': 95, 'decisions': {'deny': 59, 'allow': 36}, 'ms_mean': 947.2, 'ms_median': 702.6}

| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |
|---|---|---|---|---|---|---|
| feasibility | 256 | 256 | 1494 | 5 | 67508.2 | 56303.2 |
| planner | 256 | 256 | 872 | 3 | None | None |
| verifier | 229 | 240 | 1679 | 13 | 70919.6 | 54829.6 |
| judge | 0 | 0 | 0 | 0 | None | None |
| search | 5 | 5 | 5 | 0 | 13897.6 | 10481.4 |
| compact | 206 | 733 | 733 | 1 | 10165.4 | 9280.0 |
| grounder | 9 | 10 | 10 | 0 | 10042.4 | 5958.5 |

(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)

| domain | tasks | valid | success | rate (of tasks) | score sum | infra |
|---|---|---|---|---|---|---|
| libreoffice_impress | 31 | 31 | 5 | 16.1 % | 5.93 | 0 |
| os | 18 | 15 | 8 | 44.4 % | 8.0 | 3 |
| multi_apps | 71 | 55 | 25 | 35.2 % | 28.49 | 16 |
| libreoffice_writer | 18 | 17 | 13 | 72.2 % | 13.0 | 1 |
| vs_code | 20 | 20 | 16 | 80.0 % | 16.0 | 0 |
| libreoffice_calc | 41 | 39 | 33 | 80.5 % | 33.0 | 2 |
| vlc | 13 | 12 | 8 | 61.5 % | 8.99 | 1 |
| chrome | 39 | 39 | 13 | 33.3 % | 13.93 | 0 |
| gimp | 18 | 18 | 9 | 50.0 % | 9.0 | 0 |
| thunderbird | 10 | 10 | 7 | 70.0 % | 7.0 | 0 |

tools: {'run_command': 2047, 'run_actions': 1524, 'get_window_state': 1308, 'click': 1023, 'set_plan': 558, 'press_key': 480, 'list_windows': 474, 'inspect': 291, 'check_done': 241, 'scroll': 96, 'type_text': 80, 'list_apps': 61, 'request_human': 14, 'update_plan': 1, 'mcp__gui_run_actions': 1, 'mcp__gui_set_plan': 1, 'mcp__gui_inspect': 1, 'mcp__gui_press_key': 1}

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
