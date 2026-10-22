#!/usr/bin/env python3
"""Round 4 offline: foreground_unavailable failures (m3 + ds41): by tool, token vs x/y, focus_within_target, and what the
model did next (same call again = a wasted turn)."""
import json,glob,collections,re,os,sys
sys.argv=['x']; exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)),'reads.py')).read().split('def stats')[0])
for run in ('pi-m3-v095-2026-10-22','pi-ds41-v094-2026-10-21'):
  for cond in ('direct','gate8'):
    c=collections.Counter(); runs_hit=0; n=0
    for f,seq in runs(f'/workspace/work/gui-gate/results/{run}/pi/*-{cond}-*.jsonl'):
        n+=1; hit=False
        for i,(nm,a,res,err) in enumerate(seq):
            if err and res and 'foreground_unavailable' in res:
                hit=True
                tok='token' if 'element_token' in a else ('xy' if '"x"' in a else 'pid')
                fwt='fwt' if 'focus_within_target=true' in res else 'nofwt'
                c[(nm if nm!='run_actions' else 'run_actions',tok,fwt)]+=1
                if i+1<len(seq) and seq[i+1][0]==nm: c['next_same_tool']+=1
        runs_hit+=hit
    tot=sum(v for k,v in c.items() if isinstance(k,tuple))
    print(f'{run[:14]} {cond}: fg failures {tot} in {runs_hit}/{n} runs; next call same tool {c["next_same_tool"]}')
    for k,v in sorted(((k,v) for k,v in c.items() if isinstance(k,tuple)),key=lambda x:-x[1])[:8]: print('   ',v,k)
