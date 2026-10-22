#!/usr/bin/env python3
"""Per-run behaviour features from Pi streams + gate logs -> runfeat.csv"""
import json,os,re,csv,collections
D=os.path.dirname(os.path.abspath(__file__))+'/..'
rows={json.loads(l)['tag']:json.loads(l) for l in open(D+'/runs_scored.jsonl')}
READ={'get_window_state','list_windows','list_apps'}
out=[]
for tag,r in sorted(rows.items()):
    calls=[];res=[];final=''
    for l in open(D+'/pi/'+tag+'.jsonl',errors='replace'):
        try:e=json.loads(l)
        except:continue
        m=e.get('message') or {}
        if e.get('type')=='message_end' and m.get('role')=='assistant':
            for c in m.get('content',[]):
                if c.get('type')=='toolCall':
                    calls.append((c['name'].replace('mcp__gui__',''),json.dumps(c.get('arguments'),sort_keys=True)))
                elif c.get('type')=='text': final=c.get('text','')
        elif e.get('type')=='message_end' and m.get('role')=='toolResult':
            txt=' '.join(c.get('text','') for c in m.get('content',[]) if c.get('type')=='text')
            res.append((m.get('isError'),txt))
    n=len(calls); names=[c[0] for c in calls]
    reads=sum(x in READ for x in names)
    dup=sum(1 for i in range(1,n) if calls[i]==calls[i-1])
    cnt=collections.Counter(calls); maxrep=max(cnt.values()) if cnt else 0
    # repeated non-read action signature anywhere
    actrep=sum(v-1 for k,v in cnt.items() if k[0] not in READ and v>1)
    errs=sum(1 for e,_ in res if e)
    noshot=sum(1 for _,t in res if 'No screenshot frame' in t)
    partial=sum(1 for _,t in res if 'PARTIAL TREE' in t)
    pix=sum(1 for nm,a in calls if nm in('click','double_click','right_click') and '"x"' in a and 'element_token' not in a)
    fg=sum(1 for _,t in res if 'foreground_unavailable' in t)
    owned=sum(1 for _,t in res if 'is owned by' in t or 'owned by' in t and 'rather than' in t)
    stale=sum(1 for _,t in res if 'stale' in t.lower() and ('token' in t.lower()) and _)
    g=collections.Counter(); rv=collections.Counter()
    gp=D+'/gate/'+tag+'.jsonl'
    if os.path.exists(gp):
        for l in open(gp):
            try:e=json.loads(l)
            except:continue
            ev=e.get('ev')
            if ev=='action': g['dec_'+str(e.get('decision'))]+=1
            elif ev: g['ev_'+ev]+=1
    ff=(final or '').strip().split('\n')[0][:60]
    out.append(dict(tag=tag,task=r['task'][:4],cond=r['cond'],seed=r['seed'],ok=int(bool(r.get('success'))),
      unsafe=int(bool(r.get('unsafe_completion'))),stop=r['stop'],turns=r.get('turns'),calls=n,reads=reads,
      read_frac=round(reads/max(n,1),2),dup_consec=dup,maxrep=maxrep,actrep=actrep,errs=errs,noshot=noshot,partial=partial,
      pix=pix,fg=fg,owned=owned,done=int(ff.upper().startswith('DONE')),failsaid=int(ff.upper().startswith('FAIL')),
      interv=sum(v for k,v in g.items() if k.startswith('dec_') and k not in('dec_direct','dec_None')),
      gate=json.dumps({k:v for k,v in g.items()}),final=ff))
w=csv.DictWriter(open(D+'/analysis/runfeat.csv','w'),fieldnames=list(out[0].keys())); w.writeheader(); w.writerows(out)
import statistics as st
for c in('direct','gate8'):
    R=[o for o in out if o['cond']==c]
    print(c,{k:round(st.mean(o[k] for o in R),2) for k in('calls','reads','read_frac','dup_consec','maxrep','actrep','errs','noshot','partial','pix','fg','done','failsaid','interv')})
