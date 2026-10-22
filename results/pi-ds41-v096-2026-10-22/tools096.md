# 0.9.6 feature use (bench/tools096_stats.py)

Valid runs only (infra rows excluded).

## direct (34 valid runs)
- stops: {'timeout': 15, 'agent': 17, 'max_turns': 2}; mean turns 18.5; mean wall 322.7 s
- run_command: 102 calls in 26 runs
- check_done: 19 calls in 18 runs
- set_plan: 52 calls in 34 runs
- run_actions: 112 calls in 26 runs
- request_human: 0 calls in 0 runs
- get_window_state: 172 calls in 29 runs
- click: 107 calls in 23 runs
- press_key: 41 calls in 16 runs
- type_text: 3 calls in 3 runs
- plan: {'set': 50, 'infeasible': 2}; check_done gaps: {'calls': 19, 'plan_open': 4}
- commands (gate log): {'run': 100, 'exit_nonzero': 7}
- loop breaker events: {}; runs by max level: {}
- AT-SPI extras: {'cells_full': 31, 'cells_diff': 41, 'cells_error': 11}
- gate decisions: {'direct': 567}; reviewer calls 0 {}; human requests 0; batches 112

## gate8 (34 valid runs)
- stops: {'agent': 23, 'timeout': 10, 'max_turns': 1}; mean turns 17.0; mean wall 256.8 s
- run_command: 139 calls in 32 runs
- check_done: 24 calls in 23 runs
- set_plan: 64 calls in 34 runs
- run_actions: 79 calls in 27 runs
- request_human: 2 calls in 2 runs
- get_window_state: 111 calls in 29 runs
- click: 103 calls in 24 runs
- press_key: 43 calls in 25 runs
- type_text: 6 calls in 5 runs
- plan: {'set': 62, 'infeasible': 2}; check_done gaps: {'calls': 24, 'plan_open': 9, 'last_failed': 1}
- commands (gate log): {'run': 133, 'exit_nonzero': 5}
- loop breaker events: {'level1': 1}; runs by max level: {'max_level1': 1}
- AT-SPI extras: {'cells_full': 38, 'cells_diff': 42, 'cells_error': 34}
- gate decisions: {'direct': 570, 'look_light': 4, 'ask_human': 8, 'deny': 1}; reviewer calls 2 {'step': 2}; human requests 2; batches 79

