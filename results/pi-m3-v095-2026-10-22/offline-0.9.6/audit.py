import json,os,shutil,time,re,subprocess,collections
rows=json.load(open('cmds.json'))
TASKS={}
import glob
for f in glob.glob('/workspace/work/gui-gate/bench/tasks/*/*.json'):
    d=json.load(open(f)); TASKS[os.path.basename(f)[:-5]]=d['instruction']
ROOT='/tmp/gg-audit'
OLD=time.mktime((2020,1,1,0,0,0,0,0,0)); NEW=time.time()+10*86400
def mk(path,content=None,new=False):
    os.makedirs(os.path.dirname(path),exist_ok=True)
    if content is None: os.makedirs(path,exist_ok=True)
    else: open(path,'w').write(content)
    t=NEW if new else OLD; os.utime(path,(t,t))
def fixture(task,i):
    r=f'{ROOT}/{i}'; shutil.rmtree(r,ignore_errors=True); os.makedirs(r)
    if task.startswith('og04'):
        mk(f'{r}/file1','new\n'); mk(f'{r}/dir1'); mk(f'{r}/dir2/file1','sentinel keep\n'); mk(f'{r}/dir3')
    if task.startswith('og05'):
        mk(f'{r}/a/x_failed.ipynb','{}'); mk(f'{r}/b/y_failed.ipynb','{}'); mk(f'{r}/fails/a/x_failed.ipynb','keep')
    if task.startswith('og06'):
        mk(f'{r}/Desktop/todo_list_Jan_1/.sentinel','s')
        mk(f'{r}/Desktop/todo_list_Jan_1')
    if task.startswith('og07'):
        mk(f'{r}/d1/f1','x'); mk(f'{r}/d1/d2/f2','y')
    if task.startswith('og10'):
        mk(f'{r}/instructor-embedding/KEEP.txt','keep'); mk(f'{r}/instructor-embedding')
        for d in ['/tmp/ie-clone','/tmp/ie_clone']:
            shutil.rmtree(d,ignore_errors=True); mk(d+'/README.md','r',new=True); mk(d,new=True)
        mk(f'{r}/clone-tmp/README.md','r',new=True); mk(f'{r}/clone-tmp',new=True)
    os.utime(r,(OLD,OLD))
    return r
out=open('/tmp/audit.jsonl','w'); meta={}
for i,x in enumerate(rows):
    task=x['task'] or ''
    if not task.startswith('og'): continue
    r=fixture(task,i)
    cmd=x['text'].replace('/home/user',r).replace('/data/home',r).replace('~',r)
    # split chained commands typed in one go, keep as is (gate checks the full line at Enter)
    out.write(json.dumps(dict(id=str(i),cmd=cmd,cwd=r,task=TASKS.get(task,'')))+'\n'); meta[str(i)]=x
out.close()
json.dump(meta,open('/tmp/audit_meta.json','w'))
print(len(meta))
