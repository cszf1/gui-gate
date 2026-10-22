import json,glob,re,collections,sys
HDR='[gate] Window right after the action'
HDR2='[gate] Window now (nothing was executed)'
def results(f):
    out=[]
    for l in open(f):
        try:e=json.loads(l)
        except:continue
        if e.get('type')=='tool_execution_end':
            out.append([x.get('text','') for x in e['result'].get('content',[]) if isinstance(x,dict) and x.get('type')=='text'])
    return out
def split_block(t):
    # returns (header_line, note, rows)
    lines=t.split('\n'); return lines[0], lines[1:]
def norm_tok(r): return re.sub(r'\[\d+\]','[]',r)
def diff(prev,cur,mode,thr=0.5):
    if prev is None: return cur
    if mode in ('exact','exanc'): key=lambda r:r
    else: key=norm_tok
    ps=set(key(r) for r in prev)
    changed=[i for i,r in enumerate(cur) if key(r) not in ps]
    if len(changed)>=thr*max(1,len(cur)): return cur
    if mode in ('anc','exanc'):
        keep=set(changed)
        for i in changed:
            ind=len(cur[i])-len(cur[i].lstrip())
            for j in range(i-1,-1,-1):
                d=len(cur[j])-len(cur[j].lstrip())
                if d<ind: keep.add(j); ind=d
                if d==0: break
        return [cur[i] for i in sorted(keep)]+['(+ note)']
    return [cur[i] for i in changed]+['(+ note)']
def run(files,mode):
    tot_ctx=0; obs_ctx=0; obs_ctx_new=0; tot_new=0; nblocks=0
    for f in files:
        R=results(f); n=len(R)
        prev={}  # key: per run last tree (by first frame line?)
        for i,texts in enumerate(R):
            rem=n-i
            for t in texts:
                L=len(t); tot_ctx+=L*rem
                if t.startswith(HDR) or t.startswith(HDR2):
                    nblocks+=1
                    lines=t.split('\n'); head=lines[0]; body=lines[1:]
                    neww = body and body[0].startswith('A new window')
                    if neww: body=body[1:]
                    k='w'  # one key per run; new window -> full + reset
                    old=prev.get(k)
                    out=body if (neww or mode=='none') else diff(old,body,mode)
                    prev[k]=body
                    Lnew=len(head)+sum(len(x)+1 for x in out)
                    obs_ctx+=L*rem; obs_ctx_new+=Lnew*rem; tot_new+=Lnew*rem
                else: tot_new+=L*rem
    return dict(mode=mode,total=tot_ctx/1e6,obs=obs_ctx/1e6,obs_new=obs_ctx_new/1e6,total_new=tot_new/1e6,blocks=nblocks)
if __name__=='__main__':
    G=sorted(glob.glob('/workspace/work/gui-gate/results/pi-ds41-v094-2026-10-21/pi/*gate8*.jsonl'))
    G=[g for g in G if not g.endswith(('.err','.txt','.times'))]
    D=[g for g in glob.glob('/workspace/work/gui-gate/results/pi-ds41-v094-2026-10-21/pi/*direct*.jsonl')]
    print('direct',run(D,'none')['total'])
    for m in ['exact','exanc']: print(run(G,m))
