#!/usr/bin/env python3
"""Read behaviour of the text-only model: streaks of reads without an action, identical re-reads, unchanged results."""
import json,glob,re,collections,sys
READ={'get_window_state','list_windows','list_apps'}
def norm(t):  # strip snapshot ids / timings so identical trees compare equal
    t=re.sub(r's[0-9a-f]{8}','S',t); t=re.sub(r'walk_ms=\d+','',t); t=re.sub(r'\d+ node\(s\) \(\d+ discovered','N node(s) (D discovered',t)
    return t
def runs(pattern):
    for f in sorted(glob.glob(pattern)):
        seq=[];pending={}
        for l in open(f,errors='replace'):
            try:e=json.loads(l)
            except:continue
            m=e.get('message') or {}
            if e.get('type')=='message_end' and m.get('role')=='assistant':
                for c in m.get('content',[]):
                    if c.get('type')=='toolCall':
                        nm=c['name'].replace('mcp__gui__',''); pending[c['id']]=len(seq); seq.append([nm,json.dumps(c.get('arguments'),sort_keys=True),None,False])
            elif e.get('type')=='message_end' and m.get('role')=='toolResult':
                i=pending.get(m.get('toolCallId'))
                if i is not None:
                    seq[i][2]=' '.join(c.get('text','') for c in m.get('content',[]) if c.get('type')=='text'); seq[i][3]=bool(m.get('isError'))
        yield f,seq
def stats(pattern,label):
    S=collections.Counter(); streaks=collections.Counter(); chars_dup=0; nruns=0
    for f,seq in runs(pattern):
        nruns+=1; streak=0; last_by_win={}; 
        for nm,a,res,err in seq:
            if nm in READ:
                S['reads']+=1; streak+=1
                if streak>=3: S['read_3plus_in_streak']+=1
                if nm=='get_window_state' and res:
                    try: aa=json.loads(a)
                    except: aa={}
                    w=(aa.get('pid'),aa.get('window_id'))
                    body=norm(res.split('\n',1)[1] if '\n' in res else res)
                    if w in last_by_win and last_by_win[w]==body:
                        S['gws_identical_to_prev_read_same_window']+=1; chars_dup+=len(res)
                    last_by_win[w]=body
                    if '"query"' in a: S['gws_query']+=1
                    if 'PARTIAL TREE' in res: S['gws_partial']+=1
                    if 'timeout_ms budget ran out' in res: S['gws_partial_timeout']+=1
                    if 'cell grid' in res: S['gws_partial_grid']+=1
            else:
                if streak: streaks[min(streak,8)]+=1
                streak=0; S['actions']+=1
                last_by_win={}  # an action may change any window
        if streak: streaks[min(streak,8)]+=1
    print(label,'runs',nruns,dict(S),'dup chars',chars_dup); print('   read streak lengths (8=8+):',dict(sorted(streaks.items())))
for cond in ('direct','gate8'):
    stats(f'/workspace/work/gui-gate/results/pi-m3-v095-2026-10-22/pi/*-{cond}-*.jsonl','m3 '+cond)
    stats(f'/workspace/work/gui-gate/results/pi-ds41-v094-2026-10-21/pi/*-{cond}-*.jsonl','ds41 '+cond)
