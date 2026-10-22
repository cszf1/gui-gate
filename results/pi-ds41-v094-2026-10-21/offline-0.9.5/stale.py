import json,glob,collections,re
RD='/workspace/work/gui-gate/results/pi-ds41-v094-2026-10-21'
c=collections.Counter(); ex=collections.defaultdict(list)
for f in glob.glob(RD+'/pi/*gate8*.jsonl'):
    starts={}
    for l in open(f):
        try:e=json.loads(l)
        except:continue
        if e.get('type')=='tool_execution_start': starts[e['toolCallId']]=e.get('args')
        if e.get('type')=='tool_execution_end':
            res=e['result']; t=' '.join(x.get('text','') for x in res.get('content',[]) if isinstance(x,dict))
            if 'stale' in t.lower() and (t.startswith('[gate] Not executed') or res.get('isError')):
                k=t[:110]
                k=re.sub(r's0000\w+','S',k); k=re.sub(r'\d+','N',k)
                c[k]+=1; ex[k].append((f.split('/')[-1][:20], json.dumps(starts.get(e['toolCallId']))[:150]))
for k,v in c.most_common(10): print(v,k,'\n    ',ex[k][:2])
