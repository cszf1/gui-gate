#!/usr/bin/env python3
"""Round 2 offline: where the read-stall hints would fire on recorded transcripts (gate8 and direct, m3 + ds41):
fires per run, first-fire turn (= turns left in the 30 budget), share in successful runs, reads that came after a
hint for the same cause (the waste the hint targets)."""
import json,glob,re,collections,os,sys
sys.argv=['x']; exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)),'reads.py')).read().split('def stats')[0])
TERMS=('Terminal - ','xfce4-terminal')
def ok_of(f):
    d=f.split('/pi/')[0]; b=os.path.basename(f)[:-6]
    try: return bool(json.load(open(f'{d}/eval/{b}.eval.json')).get('success'))
    except: return None
for run in ('pi-m3-v095-2026-10-22','pi-ds41-v094-2026-10-21'):
  for cond in ('direct','gate8'):
    S=collections.Counter(); first=[]; okfires=0; nruns=0; after=collections.Counter()
    for f,seq in runs(f'/workspace/work/gui-gate/results/{run}/pi/*-{cond}-*.jsonl'):
        nruns+=1; streak=0; acted=False; hinted=set(); fired_turn=None; ok=ok_of(f); fires=0
        termpids=set()
        for i,(nm,a,res,err) in enumerate(seq):
            res=res or ''
            if nm=='list_windows':
                for m in re.finditer(r'pid=(\d+) "Terminal - ',res): termpids.add(m.group(1))
            if nm in READ:
                if nm=='get_window_state' and acted: streak+=1
                if nm=='get_window_state' and acted and streak>=3 and streak%3==0: S['streak']+=1; fires+=1; fired_turn=fired_turn or i+1
                if nm=='get_window_state':
                    try: aa=json.loads(a)
                    except: aa={}
                    key=f"{aa.get('pid')}:{aa.get('window_id') or 0}"
                    if 'cell grid' in res:
                        if 'grid:'+key not in hinted: hinted.add('grid:'+key); S['grid']+=1; fires+=1; fired_turn=fired_turn or i+1
                        else: after['grid_reads_after_hint']+=1
                    if str(aa.get('pid')) in termpids:
                        if 'term:'+key not in hinted: hinted.add('term:'+key); S['term']+=1; fires+=1; fired_turn=fired_turn or i+1
                        else: after['term_reads_after_hint']+=1
            elif nm in('click','double_click','right_click','press_key','hotkey','type_text','scroll','set_value','drag','run_actions','invoke_menu','launch_app','bring_to_front'):
                streak=0; acted=True
        if fires: S['runs_with_fire']+=1; first.append(fired_turn)
        if fires and ok: okfires+=1
    first.sort()
    print(f'{run[:14]} {cond}: runs {nruns}, fires {dict(S)}, first fire call# median {first[len(first)//2] if first else None}, runs fired that succeeded {okfires}, {dict(after)}')
