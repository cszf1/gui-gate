import json,glob,re,collections
rowrx=re.compile(r'^(\s*)- \[(\d+)\] ([\w ]+?) "([^"]*)"')
def parent_menu(tree,idx):
    L=tree.split('\n'); pos=None
    for i,l in enumerate(L):
        m=rowrx.match(l)
        if m and int(m.group(2))==idx: pos=i;break
    if pos is None: return None
    ind=len(rowrx.match(L[pos]).group(1))
    for l in reversed(L[:pos]):
        m=rowrx.match(l)
        if not m: continue
        if len(m.group(1))<ind:
            if m.group(3).strip()=='menu' and m.group(4): return m.group(4)
            ind=len(m.group(1))
    return None
c=collections.Counter(); byrun=collections.Counter()
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
    idx=int(m.group(1)); orole,olab=m.group(4),m.group(5)
    tok=(a or {}).get('element_token','') if n=='click' else ''
    snap=tok.split(':')[0] if tok else ''
    tgt=None;tree=None
    for j in range(i-1,-1,-1):
      if snap and snap not in res[j][2]: continue
      for l in res[j][2].split('\n'):
        mm=rowrx.match(l)
        if mm and int(mm.group(2))==idx: tgt=(mm.group(3).strip(),mm.group(4)); tree=res[j][2]; break
      if tgt: break
    if tgt is None: k='unknown'
    elif tgt[1]==olab and olab: k='same_label(heal: desktop click)'
    elif orole=='menu': k='open_menu(heal: Escape+retry)'
    elif ('item' in tgt[0] or tgt[0]=='menu') and parent_menu(tree,idx): k='closed_menu_parent_known(heal: reopen)'
    elif ('item' in tgt[0] or tgt[0]=='menu'): k='closed_menu_no_parent(note only)'
    else: k='other(no heal)'
    c[k]+=1
    if 'pi-ds41-v094' in f: byrun[('gate8' if 'gate8' in f else 'direct',k)]+=1
for k,v in c.most_common(): print(v,k)
print(sorted(byrun.items()))
