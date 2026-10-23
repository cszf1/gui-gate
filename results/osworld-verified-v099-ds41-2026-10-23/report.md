# OSWorld-Verified report

- tasks expected 361, finished rows 111, valid 99, infra 12 {'relay_error': 2, 'setup_failed': 9, 'eval_error': 1}
- **success 55/361 = 15.24 %** (official-style: every expected task counts, infra/missing = 0); 55/99 = 55.56 % of valid runs; score sum 58.42
- stops {'agent': 93, 'max_turns': 1, 'timeout': 5}; turns mean 31.2 / median 25; wall mean 638.1 s / median 292.7 s; total 18.58 h
- tokens: input 5,122,869 + cache read 129,163,904, output 895,896; screenshots returned 908; tool calls 3288
- request_human calls 6 (human=deny); FAIL answers 10
- gate: {'actions': 3002, 'executed': 2918, 'interventions': 84, 'human_events': 33, 'loop_notes': 5, 'loop_refused': 1}
- reviewer (System One): {'calls': 44, 'decisions': {'deny': 24, 'allow': 20}, 'ms_mean': 965.4, 'ms_median': 696.7}

| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |
|---|---|---|---|---|---|---|
| feasibility | 99 | 99 | 588 | 2 | 64398.0 | 49844.5 |
| planner | 99 | 99 | 335 | 3 | None | None |
| verifier | 91 | 98 | 694 | 4 | 66345.5 | 53269.9 |
| judge | 0 | 0 | 0 | 0 | None | None |
| search | 3 | 3 | 3 | 0 | 11083.1 | 10481.4 |
| compact | 80 | 288 | 288 | 0 | 9513.4 | 8651.0 |
| grounder | 5 | 6 | 6 | 0 | 11781.7 | 5209.9 |

(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)

| domain | tasks | valid | success | rate (of tasks) | score sum | infra |
|---|---|---|---|---|---|---|
| libreoffice_impress | 12 | 12 | 4 | 33.3 % | 4.93 | 0 |
| os | 7 | 4 | 3 | 42.9 % | 3.0 | 3 |
| multi_apps | 32 | 25 | 9 | 28.1 % | 11.49 | 7 |
| libreoffice_writer | 6 | 5 | 4 | 66.7 % | 4.0 | 1 |
| vs_code | 7 | 7 | 6 | 85.7 % | 6.0 | 0 |
| libreoffice_calc | 18 | 17 | 17 | 94.4 % | 17.0 | 1 |
| vlc | 6 | 6 | 4 | 66.7 % | 4.0 | 0 |
| chrome | 12 | 12 | 3 | 25.0 % | 3.0 | 0 |
| gimp | 7 | 7 | 3 | 42.9 % | 3.0 | 0 |
| thunderbird | 4 | 4 | 2 | 50.0 % | 2.0 | 0 |

tools: {'run_command': 911, 'run_actions': 569, 'get_window_state': 496, 'click': 407, 'set_plan': 220, 'list_windows': 192, 'inspect': 158, 'press_key': 153, 'check_done': 98, 'type_text': 32, 'scroll': 24, 'list_apps': 21, 'request_human': 6, 'update_plan': 1}

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
