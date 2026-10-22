"""R13 offline: loop breaker replayed on every gate log of m3 (0.9.5) and ds41 (0.9.4) with the shipped Rust code
(cargo test --ignored loop_replay -> r13_loop_events.tsv). Joins run outcomes and the m3 failure taxonomy."""
import csv, json, collections, os
ROOT = '/workspace/work/gui-gate/results'
ev = collections.defaultdict(list)
for l in open('r13_loop_events.tsv'):
    p, aid, kind, lvl, mode = l.rstrip('\n').split('\t')
    ev[p].append((int(aid), kind, int(lvl), mode))
meta = {}
for d in ('pi-m3-v095-2026-10-22', 'pi-ds41-v094-2026-10-21'):
    for l in open(f'{ROOT}/{d}/runs_scored.jsonl'):
        r = json.loads(l); meta[f'{d}/gate/{r["tag"]}.jsonl'] = r
tax = {r['tag']: r for r in csv.DictReader(open(f'{ROOT}/pi-m3-v095-2026-10-22/analysis/taxonomy.csv'))}
for d in ('pi-m3-v095-2026-10-22', 'pi-ds41-v094-2026-10-21'):
    print('==', d)
    agg = collections.defaultdict(collections.Counter)
    for p, es in ev.items():
        if not p.startswith(d): continue
        r = meta.get(p)
        if not r: continue
        n_act = [e[0] for e in es if e[1] == 'actions'][0]
        notes = [e for e in es if e[1] == 'note']; blocks = [e for e in es if e[1] == 'block']
        g = (r['cond'], 'success' if r.get('success') else 'fail')
        a = agg[g]; a['runs'] += 1; a['actions'] += n_act
        for L in (1, 2, 3):
            a[f'L{L}'] += any(e[2] == L for e in notes)
        a['blocks'] += len(blocks); a['runs_blocked'] += bool(blocks)
        if notes:
            first = min(e[0] for e in notes)
            a['acts_after_first_note'] += sum(1 for _ in range(n_act)) - first if n_act > first else 0
        t = tax.get(r['tag']) if 'm3' in d else None
        if t and ('loop' in t['primary'] or 'loop' in (t['flags'] or '')):
            a['tax_loop_runs'] += 1; a['tax_loop_noted'] += bool(notes)
        for e in notes: a[f'mode_{e[3]}'] += 1
    for g, a in sorted(agg.items()):
        extra = f", m3 taxonomy loop runs {a['tax_loop_runs']} -> noted {a['tax_loop_noted']}" if a['tax_loop_runs'] else ''
        modes = {k[5:]: v for k, v in a.items() if k.startswith('mode_')}
        print(f"{g[0]:6s} {g[1]:7s} runs {a['runs']:3d}: level1 in {a['L1']}, level2 in {a['L2']}, level3 in {a['L3']} runs; "
              f"steps refused at level>=2: {a['blocks']} in {a['runs_blocked']} runs; logged actions after the first note {a['acts_after_first_note']}{extra}; modes {modes}")
