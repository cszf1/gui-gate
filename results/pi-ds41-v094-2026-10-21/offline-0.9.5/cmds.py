import json,glob,re,os,collections
TASKS={}
for f in glob.glob('/workspace/work/gui-gate/bench/tasks/*/*.json'):
    d=json.load(open(f)); TASKS[os.path.basename(f)[:-5]]=d.get('instruction','')
def task_of(fn):
    b=os.path.basename(fn)
    for k in TASKS:
        if b.startswith(k): return k
    m=re.match(r'(og\d\d|ow\d\d)',b)
    if m:
        for k in TASKS:
            if k.startswith(m.group(1)): return k
    return None
def texts(args):
    out=[]
    if isinstance(args,dict):
        for k in ('text','value'):
            if isinstance(args.get(k),str): out.append(args[k])
        for k in ('actions','steps'):
            if isinstance(args.get(k),list):
                for s in args[k]: out+=texts(s)
        if isinstance(args.get('args'),dict): out+=texts(args['args'])
    return out
RX=re.compile(r'\b(mv|cp|rsync|git\s+clone|rm|chmod|rename)\b')
rows=[]
for f in sorted(glob.glob('/workspace/work/gui-gate/results/*/pi/*.jsonl')):
    run=f.split('/')[-3]
    for l in open(f):
        try: e=json.loads(l)
        except: continue
        if e.get('type')!='tool_execution_start': continue
        for t in texts(e.get('args')):
            if RX.search(t): rows.append(dict(run=run,file=os.path.basename(f),task=task_of(f),tool=e.get('toolName'),text=t.strip()))
json.dump(rows,open('cmds.json','w'),indent=0)
c=collections.Counter(RX.search(r['text']).group(1).split()[0] for r in rows)
print(len(rows),c)
