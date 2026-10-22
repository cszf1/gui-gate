cat > /tmp/mcpc.py <<'PYEOF'
import json,os,subprocess,sys,time
class G:
    def __init__(s,args,env):
        s.p=subprocess.Popen(args,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=open('/tmp/gate_rc.err','w'),text=True,env=env); s.n=0
        s.rpc("initialize",{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"rolecheck","version":"0"}})
        s.p.stdin.write(json.dumps({"jsonrpc":"2.0","method":"notifications/initialized"})+"\n"); s.p.stdin.flush()
    def rpc(s,m,pa):
        s.n+=1; s.p.stdin.write(json.dumps({"jsonrpc":"2.0","id":s.n,"method":m,"params":pa})+"\n"); s.p.stdin.flush()
        while True:
            l=s.p.stdout.readline()
            if not l: raise SystemExit("gate closed")
            j=json.loads(l)
            if j.get("id")==s.n: return j
    def call(s,name,args):
        t=time.time(); r=s.rpc("tools/call",{"name":name,"arguments":args}); res=r.get("result") or {}
        txt=" ".join(c.get("text","") for c in res.get("content",[]) if c.get("type")=="text")
        imgs=[(c.get("mimeType"),len(c.get("data",""))) for c in res.get("content",[]) if c.get("type")=="image"]
        print(f"## {name} {json.dumps(args)[:150]} ({time.time()-t:.1f}s) err={res.get('isError')} imgs={imgs}\n{txt[:1500]}\n",flush=True)
        return res,txt
PYEOF
export DISPLAY=:1 XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus PATH=$HOME/.local/bin:$PATH GDK_BACKEND=x11 NO_AT_BRIDGE=0
xrandr -s 1440x900 >/dev/null 2>&1 || { xrandr --newmode 1440x900_60 106.50 1440 1528 1672 1904 900 903 909 934 -hsync +vsync; xrandr --addmode VNC-0 1440x900_60; xrandr --output VNC-0 --mode 1440x900_60; } >/dev/null 2>&1
xdpyinfo | awk '/dimensions/{print $2}'
pkill -f soffice; sleep 1
setsid soffice --writer --norestore >/dev/null 2>&1 < /dev/null & sleep 9
mkdir -p /data/workspace/rolecheck; : > /data/workspace/rolecheck/memory.jsonl
echo "Type the line 'Hello gate' into the open LibreOffice Writer document, make it bold, and save it as /home/user/Documents/hello.docx (Word format)." > /data/workspace/rolecheck/task.txt
cd /tmp && timeout 900 python3 - <<'P'
import json,os,sys,re,time
sys.path.insert(0,'/tmp'); from mcpc import G
env=dict(os.environ,GUI_GATE_097="1",GUI_GATE_DIGEST_EVERY="2",GUI_GATE_MEMORY="/data/workspace/rolecheck/memory.jsonl")
for f in ["INSPECT","JOBS","PHANTOM","TWO_STRIKES","FEASIBILITY_FIRST","STATE_PLAN","VERIFY_PLUS","SKILLS","SNAP","PICK","DIGEST","MICRO_VERIFY"]: env["GUI_GATE_"+f]="1"
for l in open(os.path.expanduser('~/.config/gui-gate/reviewer.env')):
    if '=' in l: k,v=l.strip().split('=',1); env[k]=v
g=G([os.path.expanduser("~/.local/bin/gui-gate"),"--policy","gate","--human","deny","--task-file","/data/workspace/rolecheck/task.txt","--log","/data/workspace/rolecheck/gate.jsonl"],env)
r,t=g.call("list_windows",{})
print("SC",json.dumps(r.get("structuredContent"))[:800]); print("TXT",t[-800:])
sc=r.get("structuredContent") or {}
ws=[x for x in sc.get("windows",[]) if 'Writer' in json.dumps(x)] or sc.get("windows",[])
pid=ws[0].get("pid"); wid=ws[0].get("window_id"); print("PID",pid,"WID",wid)
g.call("get_window_state",{"pid":pid,"window_id":wid,"include_screenshot":True})
g.call("set_plan",{"feasible":True,"milestones":["the document contains 'Hello gate' in bold","/home/user/Documents/hello.docx exists in Word format"]})
g.call("inspect",{"kind":"howto","query":"how to make selected text bold in LibreOffice Writer"})
g.call("inspect",{"kind":"locate","describe":"the Bold button in the formatting toolbar","pid":pid,"window_id":wid})
g.call("type_text",{"pid":pid,"window_id":wid,"text":"Hello gate"})
g.call("press_key",{"pid":pid,"window_id":wid,"key":"home","modifiers":["shift"]})
g.call("run_actions",{"pid":pid,"window_id":wid,"pick_one":True,"steps":[{"tool":"press_key","args":{"key":"b","modifiers":["ctrl"]}},{"tool":"click","args":{"target":{"label":"Bold"}}}]})
g.call("press_key",{"pid":pid,"window_id":wid,"key":"end"})
g.call("check_done",{})
g.p.stdin.close(); g.p.wait(timeout=30)
P
pkill -f soffice
python3 - <<'P'
import json,collections
c=collections.Counter(); 
for l in open('/data/workspace/rolecheck/gate.jsonl'):
    try: e=json.loads(l)
    except: continue
    ev=e.get('ev')
    if ev=='role': print('ROLE',e.get('role'),{k:e.get(k) for k in ('calls','ok','error','decision','use','pick')})
    if ev=='briefing': print('BRIEF',{k:e.get(k) for k in ('feasibility','planner')})
    if ev in ('review','risk') or 'reviewer' in json.dumps(e)[:2000]: c['reviewer-ish']+=1
    c[ev]+=1
print(dict(c))
P
