R=${R:-/data/workspace/runs/smoke-v099-ds41}
tail -3 $R.log; cat $R/progress.log 2>/dev/null | tail -5
python3 - $R <<'P'
import json,sys,os,collections
R=sys.argv[1]
for l in open(R+'/runs.jsonl') if os.path.exists(R+'/runs.jsonl') else []:
    j=json.loads(l)
    try: ev=json.load(open(R+'/eval/'+j['tag']+'.eval.json'))
    except Exception: ev={}
    print(j['tag'][:45],'infra',j.get('infra'),j.get('pi'),'score',ev.get('score'),ev.get('error','')[:200])
P
f=$(ls -t $R/pi/*.jsonl 2>/dev/null | head -1); [ -n "$f" ] && { echo CUR $f $(grep -c '"type":"message_end"' $f) msgs; tail -c 400 $f.err; ls -t $R/eval | head -2; }
