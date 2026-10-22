#!/usr/bin/env python3
"""Round 1 offline effect: (a) step reviews that got a window without element rows (fixed by rendering elements-format
reads), (b) refusals a re-sent doubted step would no longer get (reviewer_insist), over every gate log in results/."""
import json,glob,re,collections
blind=collections.Counter(); calls=collections.Counter(); elemfmt=collections.Counter()
saved=collections.Counter(); saved_ex=collections.defaultdict(list); denies=collections.Counter()
for f in sorted(glob.glob('/workspace/work/gui-gate/results/*/gate/*.jsonl')):
    run=f.split('/')[-3]
    acts=[]
    for l in open(f):
        try:e=json.loads(l)
        except:continue
        if e.get('ev')=='action': acts.append(e)
    doubted=set()
    for e in acts:
        j=e.get('judge') if isinstance(e.get('judge'),dict) else None
        tr=e.get('trail') or []
        if j and isinstance(j.get('sent'),dict):
            calls[run]+=1; wt=j['sent'].get('window_tail','')
            if not re.search(r'\[\d+\]',wt):
                blind[run]+=1
                if 'structuredContent.elements' in wt: elemfmt[run]+=1
        if tr and tr[-1]=='reviewer_deny':
            denies[run]+=1
            if e.get('sig') in doubted and e.get('tier')!='rule':
                saved[run]+=1; saved_ex[run].append((f.split('/')[-1][:30],(e.get('target') or {}).get('label','')[:24]))
                doubted.discard(e.get('sig'))   # insist fires once, then the sig is clean again
            else:
                doubted.add(e.get('sig'))
for r in sorted(set(calls)|set(denies)):
    print(r[:28].ljust(29),'step reviews',calls[r],'no rows',blind[r],'(elements-format read',elemfmt[r],') denies',denies[r],'refusals removed by insist',saved[r],saved_ex[r][:6])
print('TOTAL reviews',sum(calls.values()),'no rows',sum(blind.values()),'elements-format',sum(elemfmt.values()),'denies',sum(denies.values()),'removed',sum(saved.values()))
