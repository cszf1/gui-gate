"""R11 offline: where a gated run_command would replace GUI terminal plumbing (m3 + ds41 logs).
Counts, per run that typed shell commands into a terminal: commands typed, GUI calls spent on the terminal window
(reads, clicks, keys other than the command text), and calls after the first command. Verdicts of the recorded
commands through run_command are in safety_r11.txt (fixture audit, og tasks)."""
import collections, re, sys
from pilog import runs, terminal_pids
SHELL = re.compile(r'^\s*(ls|cd|cp|mv|find|chmod|cat|echo|git|python3?|mkdir|rm|touch|stat|tree|head|tail|wc|du|rsync|pwd|for|xargs)\b')
for which in ('m3', 'ds41'):
    agg = collections.defaultdict(collections.Counter)
    for r in runs(which):
        tp = terminal_pids(r['calls'])
        cmds = [i for i, c in enumerate(r['calls']) if c['name'] in ('type_text', 'run_actions') and
                SHELL.search(str(c['args'].get('text', '')) or str(c['args'].get('steps', ''))[:0])]
        if not cmds: continue
        a = agg[r['cond']]
        a['runs'] += 1; a['success'] += r['success']; a['cmds'] += len(cmds)
        first = cmds[0]
        plumbing = [c for c in r['calls'] if (c['args'].get('pid') in tp) and c['name'] != 'type_text']
        a['term_plumbing_calls'] += len(plumbing)
        a['calls_after_first_cmd'] += len(r['calls']) - first - 1
        a['calls_before_first_cmd'] += first
        a['reads_after_first_cmd'] += sum(1 for c in r['calls'][first+1:] if c['name'] in ('get_window_state', 'list_windows', 'list_apps'))
    print(f'== {which}')
    for cond, a in sorted(agg.items()):
        n = a['runs']
        print(f"{cond}: runs with typed shell commands {n} (success {a['success']}), commands {a['cmds']}, "
              f"calls before the first command {a['calls_before_first_cmd']/n:.1f}/run, after it {a['calls_after_first_cmd']/n:.1f}/run "
              f"(reads {a['reads_after_first_cmd']/n:.1f}), GUI calls on the terminal window {a['term_plumbing_calls']/n:.1f}/run")
