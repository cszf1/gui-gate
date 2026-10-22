"""R15 offline: batching. (a) how the existing run_actions batch was used (m3 / ds41 gate8 and direct: calls, steps,
stops), (b) runs of >= 2 consecutive single actions with no read in between where every target was already known
(keys / typing, or element tokens from the same snapshot) - each such run could have been one batch call."""
import collections, json, re
from pilog import runs
ACT = {'click', 'double_click', 'right_click', 'type_text', 'press_key', 'hotkey', 'scroll', 'set_value', 'drag', 'invoke_menu'}
for w in ('m3', 'ds41'):
    agg = collections.defaultdict(collections.Counter)
    for r in runs(w):
        a = agg[r['cond']]; a['runs'] += 1
        seq = []; snap = None
        def flush():
            if len(seq) >= 2:
                a['batchable_runs'] += 1; a['batchable_calls'] += len(seq); a['turns_saveable'] += len(seq) - 1
        for c in r['calls']:
            n = c['name']
            if n == 'run_actions':
                a['ra_calls'] += 1
                st = c['args'].get('steps') or []
                a['ra_steps'] += len(st)
                m = re.search(r'run_actions: (\d+) GUI action', c['result'])
                if m: a['ra_executed'] += int(m.group(1))
                if 'STOPPED' in c['result']: a['ra_stopped'] += 1
                if 'refused by the gate' in c['result']: a['ra_refused'] += 1
                flush(); seq = []; snap = None
                continue
            if n in ACT:
                tok = c['args'].get('element_token') or ''
                s = tok.split(':')[0] if tok else None
                known = (not tok and 'x' not in c['args']) or (s and (snap is None or s == snap))
                if known and not c['err']:
                    seq.append(c); snap = s or snap
                    continue
                flush(); seq = [c] if (known and not c['err']) else []; snap = s
                continue
            flush(); seq = []; snap = None
        flush()
    print('==', w)
    for cond, a in sorted(agg.items()):
        n = a['runs']
        print(f"{cond}: run_actions calls {a['ra_calls']} ({a['ra_calls']/n:.2f}/run), steps {a['ra_steps']}, GUI actions executed in them {a['ra_executed']}, "
              f"stopped early {a['ra_stopped']} (refused {a['ra_refused']}); batchable sequences of single calls {a['batchable_runs']} "
              f"({a['batchable_calls']} calls) -> {a['turns_saveable']} model turns saveable ({a['turns_saveable']/n:.1f}/run)")
