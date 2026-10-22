# 0.9.6 feature use (bench/tools096_stats.py)

Valid runs only (infra rows excluded).

## direct (10 valid runs)
- stops: {'agent': 6, 'timeout': 2, 'max_turns': 2}; mean turns 18.1; mean wall 245.4 s
- run_command: 36 calls in 9 runs
- check_done: 6 calls in 6 runs
- set_plan: 17 calls in 10 runs
- run_actions: 28 calls in 4 runs
- request_human: 0 calls in 0 runs
- get_window_state: 41 calls in 6 runs
- click: 28 calls in 5 runs
- press_key: 14 calls in 5 runs
- type_text: 5 calls in 3 runs
- plan: {'set': 15, 'infeasible': 2}; check_done gaps: {'calls': 6, 'plan_open': 2}
- commands (gate log): {'run': 36, 'exit_nonzero': 2}
- loop breaker events: {}; runs by max level: {}
- AT-SPI extras: {'cells_full': 8, 'cells_diff': 12, 'cells_error': 4}
- gate decisions: {'direct': 159}; reviewer calls 0 {}; human requests 0; batches 27

## gate8 (10 valid runs)
- stops: {'agent': 10}; mean turns 13.9; mean wall 177.4 s
- run_command: 35 calls in 9 runs
- check_done: 10 calls in 10 runs
- set_plan: 22 calls in 10 runs
- run_actions: 26 calls in 6 runs
- request_human: 1 calls in 1 runs
- get_window_state: 21 calls in 6 runs
- click: 13 calls in 4 runs
- press_key: 1 calls in 1 runs
- type_text: 1 calls in 1 runs
- plan: {'set': 19, 'infeasible': 3}; check_done gaps: {'calls': 10, 'plan_open': 6}
- commands (gate log): {'run': 34, 'exit_nonzero': 1}
- loop breaker events: {}; runs by max level: {}
- AT-SPI extras: {'cells_full': 8, 'cells_diff': 5, 'cells_error': 4}
- gate decisions: {'direct': 103, 'ask_human': 1, 'look_full': 1}; reviewer calls 1 {'risk': 1}; human requests 1; batches 26

