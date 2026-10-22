#!/usr/bin/env python3
"""Compact per-run transcripts (assistant calls + result heads) -> analysis/tx/<tag>.txt"""
import json,os,sys
D=os.path.dirname(os.path.abspath(__file__))+'/..'
os.makedirs(D+'/analysis/tx',exist_ok=True)
for f in sorted(os.listdir(D+'/pi')):
    if not f.endswith('.jsonl'): continue
    out=[]; n=0
    for l in open(D+'/pi/'+f,errors='replace'):
        try:e=json.loads(l)
        except:continue
        m=e.get('message') or {}
        if e.get('type')=='message_end' and m.get('role')=='assistant':
            n+=1
            for c in m.get('content',[]):
                if c.get('type')=='toolCall':
                    a=dict(c.get('arguments') or {}); 
                    out.append(f"T{n} CALL {c['name'].replace('mcp__gui__','')} {json.dumps(a,ensure_ascii=False)[:300]}")
                elif c.get('type')=='text' and c.get('text','').strip():
                    out.append(f"T{n} SAY {c['text'].strip()[:300]}")
        elif e.get('type')=='message_end' and m.get('role')=='toolResult':
            txt=' | '.join(c.get('text','') for c in m.get('content',[]) if c.get('type')=='text')
            out.append(f"   -> {'ERR ' if m.get('isError') else ''}{txt[:400].replace(chr(10),' / ')}")
    open(D+'/analysis/tx/'+f.replace('.jsonl','.txt'),'w').write('\n'.join(out))
print('ok')
