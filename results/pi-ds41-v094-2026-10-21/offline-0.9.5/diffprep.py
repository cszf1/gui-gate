import json,glob,re,sys
HDR=('[gate] Window right after the action','[gate] Window now (nothing was executed)')
out=open('/tmp/diffseq.jsonl','w'); meta=[]
files=[f for f in sorted(glob.glob('/workspace/work/gui-gate/results/pi-ds41-v094-2026-10-21/pi/*gate8*.jsonl'))]
for fi,f in enumerate(files):
    starts={}; n=0; seq=[]
    for l in open(f):
        try:e=json.loads(l)
        except:continue
        if e.get('type')=='tool_execution_start': starts[e['toolCallId']]=e.get('args') or {}
        if e.get('type')=='tool_execution_end': seq.append((e['toolName'].split('__')[-1],starts.get(e['toolCallId'],{}),[x.get('text','') for x in e['result'].get('content',[]) if isinstance(x,dict) and x.get('type')=='text']))
    N=len(seq)
    for i,(name,args,texts) in enumerate(seq):
        pid=args.get('pid') or 0
        for j,t in enumerate(texts):
            rec=None
            if t.startswith(HDR):
                lines=t.split('\n'); body=lines[1:]; nw=bool(body) and body[0].startswith('A new window')
                if nw: body=body[1:]
                more=[k for k,x in enumerate(body) if x.startswith('... (') and 'more rows' in x]
                if more: body=body[:more[0]]
                rec=dict(kind='auto',rows=body,new_window=nw,head=len(lines[0])+(len(lines[1])+1 if nw else 0))
            elif name=='get_window_state' and j==0 and not t.startswith('[gate]'):
                lines=t.split('\n')
                rows=lines[1:] if lines and lines[0].startswith('window_id=') else lines
                rec=dict(kind='read',rows=rows,new_window=False,head=0)
            if rec:
                rec.update(id=f'{fi}:{i}:{j}',key=f'{fi}:{pid}',rem=N-i,full_chars=len(t))
                out.write(json.dumps(rec)+'\n')
print(len(files))
