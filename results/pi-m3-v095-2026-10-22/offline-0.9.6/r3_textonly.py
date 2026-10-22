#!/usr/bin/env python3
"""Round 3 offline: what a text-only session would stop paying for (m3 logs): screenshots captured+re-encoded for a
client that drops images, and gate/driver advice that tells a text-only model to use screenshots or x/y."""
import json,glob,collections,re,statistics as st
for cond in ('direct','gate8'):
    shots=0; ms_shot=[]; ms_tree=[]; img_ms=0; img_out=0; nruns=0
    for f in glob.glob(f'/workspace/work/gui-gate/results/pi-m3-v095-2026-10-22/gate/*-{cond}-*.jsonl'):
        nruns+=1
        for l in open(f):
            e=json.loads(l)
            if e.get('ev')=='obs' and e.get('tool')=='get_window_state' and not e.get('cached'):
                (ms_shot if e.get('screenshot') else ms_tree).append(e.get('ms') or 0)
            if e.get('ev')=='img': img_ms+=e.get('ms') or 0; img_out+=e.get('bytes_out') or 0
    adv=collections.Counter()
    for f in glob.glob(f'/workspace/work/gui-gate/results/pi-m3-v095-2026-10-22/pi/*-{cond}-*.jsonl'):
        for l in open(f,errors='replace'):
            try:e=json.loads(l)
            except:continue
            m=e.get('message') or {}
            if e.get('type')=='message_end' and m.get('role')=='toolResult':
                t=' '.join(c.get('text','') for c in m.get('content',[]) if c.get('type')=='text')
                for k in ('confirm with a screenshot','include_screenshot:true','fresh screenshot','take x,y from that image','No screenshot frame'):
                    adv[k]+=t.count(k)
    print(f'{cond}: runs {nruns}; agent/gate reads with screenshot {len(ms_shot)} (median {st.median(ms_shot):.0f} ms) vs tree-only {len(ms_tree)} (median {st.median(ms_tree):.0f} ms); '
          f're-encode {img_ms/1000:.1f} s total, {img_out/1e6:.1f} MB JPEG produced and dropped by the client; '
          f'est. read time saved {(st.median(ms_shot)-st.median(ms_tree))*len(ms_shot)/1000/nruns:.1f} s/run')
    print('   vision advice in tool results (per run):',{k:round(v/nruns,2) for k,v in adv.items()})
