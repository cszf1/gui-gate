import json,glob,re,collections
rowrx=lambda idx: re.compile(r'^\s*- \[%s\] (.+?) "([^"]*)"'%idx, re.M)
c=collections.Counter(); ex=collections.defaultdict(list)
for f in sorted(glob.glob('/workspace/work/gui-gate/results/*/pi/*.jsonl')):
  res=[]; starts={}
  for l in open(f):
    try:e=json.loads(l)
    except:continue
    if e.get('type')=='tool_execution_start': starts[e['toolCallId']]=e['args']
    if e.get('type')=='tool_execution_end':
      t=' '.join(x.get('text','') for x in e['result'].get('content',[]) if isinstance(x,dict))
      res.append((e['toolName'].split('__')[-1],starts.get(e['toolCallId']),t))
  for i,(n,a,t) in enumerate(res):
    m=re.search(r'element \[(\d+)\] \(pid \d+\) has a snapshot frame at \((\d+), (\d+)\), but that screen point is owned by ([\w ]+?) "([^"]*)"',t)
    if not m: continue
    idx,x,y,orole,olab=m.groups()
    tgt=None
    tok=(a or {}).get('element_token','')
    snap=tok.split(':')[0] if tok else ''
    for j in range(i-1,-1,-1):
      if snap and snap not in res[j][2]: continue
      mm=rowrx(idx).search(res[j][2])
      if mm: tgt=(mm.group(1).split()[-1] if mm.group(1) else '',mm.group(2)); break
    if tgt is None: k='unknown'
    elif tgt[1]==olab and olab: k='same_label'
    elif orole=='menu' : k='other_menu_open'
    else: k='other'
    c[k]+=1; ex[k].append((f.split('/')[-1][:26],tgt,orole,olab))
print(c)
for k in ex: print(k, ex[k][:5])
print(collections.Counter((x[1][0] if x[1] else None) for x in ex['other']))
print(collections.Counter(x[2] for x in ex['other']))
print([x for x in ex['other'] if x[1] and x[1][0] not in ('item','menu')][:12])
