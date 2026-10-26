R=${R:-/data/workspace/runs/smoke-v099-ds41}
for T in $TAGS; do echo "=== $T"; python3 - $R/pi/$T.jsonl <<'P'
import json,sys
for l in open(sys.argv[1]):
    try: e=json.loads(l)
    except: continue
    t=e.get('type')
    if t=='tool_execution_start':
        print('CALL',e.get('toolName','').replace('mcp__gui__',''),json.dumps(e.get('args'))[:260])
    elif t=='tool_execution_end':
        r=e.get('result') or {}
        txt=' '.join(c.get('text','') for c in (r.get('content') or []) if isinstance(c,dict) and c.get('type')=='text')
        print('  ->',('ERR ' if e.get('isError') else '')+txt[:400].replace('\n',' | '))
    elif t=='message_end':
        m=e.get('message') or {}
        if m.get('role')=='assistant':
            txt=' '.join(c.get('text','') for c in m.get('content',[]) if c.get('type')=='text')
            if txt.strip(): print('SAY',txt[:300].replace('\n',' | '))
            if m.get('stopReason') in ('error','aborted'): print('STOP',m.get('stopReason'),str(m.get('errorMessage'))[:300])
P
done
