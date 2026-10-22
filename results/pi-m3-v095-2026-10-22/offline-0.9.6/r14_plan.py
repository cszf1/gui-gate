"""R14 offline: the infeasible OSWorld tasks (ow06 Chrome 'Xenothian' UI language, ow17 GIMP CMYK mode) - how the
runs ended and what they cost - and the context cost of the per-action progress line."""
import collections
from pilog import runs
for w in ('m3', 'ds41'):
    rows = [r for r in runs(w) if r['task'][:4] in ('ow06', 'ow17')]
    fail = sum(1 for r in rows if r['final'].strip().upper().startswith('FAIL'))
    done = sum(1 for r in rows if r['final'].strip().upper().startswith('DONE'))
    cap = sum(1 for r in rows if r['stop'] == 'max_turns')
    calls = sum(len(r['calls']) for r in rows)
    print(f'{w}: infeasible-task runs {len(rows)}: replied FAIL {fail}, DONE {done}, cut at max_turns {cap}; tool calls spent {calls} ({calls/len(rows):.1f}/run)')
    acts = [sum(1 for c in r['calls'] if c['name'] not in ('get_window_state', 'list_windows', 'list_apps')) for r in runs(w)]
    print(f'   progress line cost if a plan is set: ~60-110 chars x {sum(acts)/len(acts):.1f} actions/run = ~{0.085*sum(acts)/len(acts):.1f} KB/run')
