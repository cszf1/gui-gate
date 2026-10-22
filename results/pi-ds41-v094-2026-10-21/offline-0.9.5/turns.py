import json,glob,collections,re,os
RD='/workspace/work/gui-gate/results/pi-ds41-v094-2026-10-21'
runs=[json.loads(l) for l in open(RD+'/runs_scored.jsonl')]
def classify(name,args,t,err):
    n=name.split('__')[-1]
    t=t.split('[gate] Window right after')[0].split('[gate] Window now')[0][:400]
    if t.startswith('[gate] Not executed'):
        if 'UI still busy' in t: return 'gate_refusal_busy'
        if 'risk' in t or 'refused' in t or 'declined' in t: return 'gate_refusal_risk'
        if 'failed in a row' in t: return 'gate_refusal_after_fail'
        if 'reviewer' in t: return 'gate_refusal_reviewer'
        if 'stale' in t: return 'gate_refusal_stale'
        return 'gate_refusal_look'
    if t.startswith('[gate] Nothing has been done since'): return 'dup_read_cached'
    if n in ('get_window_state','list_windows','list_apps','screenshot'): return 'obs_'+n
    if err:
        tl=t.lower()
        if 'owned by' in tl: return 'err_occluded'
        if 'stale' in tl: return 'err_stale_token'
        if 'no screenshot frame' in tl: return 'err_no_frame'
        if 'run_actions' in tl and 'stopped' in tl: return 'err_batch_stopped'
        if 'invalid arguments' in tl: return 'err_bad_args'
        if 'foreground_unavailable' in tl: return 'err_focus'
        if 'not found' in tl or 'no element' in tl: return 'err_not_found'
        return 'err_other'
    if n=='run_actions': return 'act_batch'
    return 'act_ok'
agg=collections.defaultdict(collections.Counter); nruns=collections.Counter(); errs=collections.Counter()
for r in runs:
    hit = r['stop'] in ('timeout','max_turns') or r['turns']>=30
    if not hit: continue
    cond=r['cond']; nruns[cond]+=1
    f=f"{RD}/pi/{r['tag']}{'-try%d'%r['attempt'] if r['attempt']>1 else ''}.jsonl"
    if not os.path.exists(f): f=f"{RD}/pi/{r['tag']}.jsonl"
    starts={}
    for l in open(f):
        try:e=json.loads(l)
        except:continue
        if e.get('type')=='tool_execution_start': starts[e['toolCallId']]=e.get('args')
        if e.get('type')=='tool_execution_end':
            res=e['result']; t=' '.join(x.get('text','') for x in res.get('content',[]) if isinstance(x,dict))
            err=bool(res.get('isError') or e.get('isError'))
            k=classify(e['toolName'],starts.get(e['toolCallId']),t,err)
            agg[cond][k]+=1
            if k=='err_other': errs[t[:90]]+=1
print(dict(nruns))
keys=sorted(set(k for c in agg for k in agg[c]))
for k in keys: print(f'{k:24}', '  '.join(f'{c}={agg[c][k]:4d} ({agg[c][k]/nruns[c]:.1f}/run)' for c in sorted(agg)))
print(errs.most_common(12))
