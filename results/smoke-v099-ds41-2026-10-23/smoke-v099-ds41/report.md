# OSWorld-Verified report

- tasks expected 3, finished rows 3, valid 1, infra 2 {'setup_failed': 1, 'eval_error': 1}
- **success 1/3 = 33.33 %** (official-style: every expected task counts, infra/missing = 0); 1/1 = 100.0 % of valid runs; score sum 1.0
- stops {'agent': 1}; turns mean 9.0 / median 9; wall mean 268.2 s / median 268.2 s; total 0.18 h
- tokens: input 25,524 + cache read 97,152, output 944; screenshots returned 2; tool calls 9
- request_human calls 0 (human=deny); FAIL answers 0
- gate: {'actions': 6, 'executed': 6, 'interventions': 0, 'human_events': 0, 'loop_notes': 0, 'loop_refused': 0}
- reviewer (System One): {'calls': 0, 'decisions': {}, 'ms_mean': None, 'ms_median': None}

| role | runs using | invocations | HTTP calls | errors | mean ms | median ms |
|---|---|---|---|---|---|---|
| feasibility | 1 | 1 | 6 | 0 | 42294.1 | 42294.1 |
| planner | 1 | 1 | 2 | 0 | None | None |
| verifier | 1 | 2 | 18 | 0 | 80780.8 | 80780.8 |
| judge | 0 | 0 | 0 | 0 | None | None |
| search | 0 | 0 | 0 | 0 | None | None |
| compact | 0 | 0 | 0 | 0 | None | None |
| grounder | 0 | 0 | 0 | 0 | None | None |

(feasibility ms = the whole briefing incl. planner; compact/judge ms = gap to the previous gate event, approximate)

| domain | tasks | valid | success | rate (of tasks) | score sum | infra |
|---|---|---|---|---|---|---|
| chrome | 1 | 0 | 0 | 0.0 % | 0.0 | 1 |
| thunderbird | 1 | 1 | 1 | 100.0 % | 1.0 | 0 |
| libreoffice_calc | 1 | 0 | 0 | 0.0 % | 0.0 | 1 |

tools: {'check_done': 3, 'set_plan': 2, 'get_window_state': 2, 'list_apps': 1, 'run_actions': 1}

## infra rows

- chrome-da46d875-6b82-4681-9284-653b0c7ae241: setup_failed (attempts 1) Exception("Setup step 3 failed: _chrome_open_tabs_setup - Chrome DevTools websocket server not ready at http://127.0.0.1:9222 after 120.0s (last state: debug endpoint not answering: ('Connection aborted.', RemoteDisconnected('Remote end closed connection without response')))")
- libreoffice_calc-4e6fcf72-daf3-439f-a232-c434ce416af6: eval_error (attempts 1) RuntimeError('Employee age reference date was not initialized during setup')
