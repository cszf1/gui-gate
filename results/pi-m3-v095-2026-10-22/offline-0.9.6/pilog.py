"""Shared loader for the 0.9.6 R11+ offline analyses: Pi jsonl runs -> ordered tool calls with results."""
import json, glob, os, re
ROOT = '/workspace/work/gui-gate/results'
RUNS = {'m3': f'{ROOT}/pi-m3-v095-2026-10-22', 'ds41': f'{ROOT}/pi-ds41-v094-2026-10-21'}
TASKS = {}
for f in glob.glob('/workspace/work/gui-gate/bench/tasks/*/*.json'):
    TASKS[os.path.basename(f)[:-5]] = json.load(open(f))['instruction']

def text_of(res):
    if not isinstance(res, dict): return str(res or '')
    return '\n'.join(c.get('text', '') for c in res.get('content', []) if isinstance(c, dict))

def load_run(path):
    starts, calls = {}, []
    final = ''
    for l in open(path):
        try: e = json.loads(l)
        except Exception: continue
        t = e.get('type')
        if t == 'tool_execution_start':
            c = dict(id=e.get('toolCallId'), name=(e.get('toolName') or '').replace('mcp__gui__', ''), args=e.get('args') or {}, result='', err=None)
            starts[c['id']] = c; calls.append(c)
        elif t == 'tool_execution_end':
            c = starts.get(e.get('toolCallId'))
            if c is not None:
                r = e.get('result') or {}
                c['result'] = text_of(r); c['err'] = bool(r.get('isError')) if isinstance(r, dict) else None
    fp = path + '.final.txt'
    if os.path.exists(fp): final = open(fp).read().strip()
    return calls, final

def runs(which):
    base = RUNS[which]
    meta = {}
    for l in open(f'{base}/runs_scored.jsonl'):
        r = json.loads(l); meta[r['tag']] = r
    for tag, r in sorted(meta.items()):
        p = f'{base}/pi/{tag}.jsonl'
        if not os.path.exists(p) or r.get('infra'): continue
        calls, final = load_run(p)
        yield dict(tag=tag, task=r['task'], cond=r['cond'], score=r.get('score') or 0, success=bool(r.get('success')),
                   stop=r.get('stop'), final=final or (r.get('final') or ''), instr=TASKS.get(r['task'], ''), calls=calls,
                   gate=f'{base}/gate/{tag}.jsonl')

TERM_RX = re.compile(r'window_id=(\d+) pid=(\d+) "(?:Terminal[^"]*|[^"]*@[^"]*:[^"]*)"')
def terminal_pids(calls):
    pids = set()
    for c in calls:
        if c['name'] == 'list_windows':
            for m in TERM_RX.finditer(c['result']): pids.add(int(m.group(2)))
        if c['name'] == 'get_window_state' and re.search(r'frame = "Terminal', c['result'][:600]):
            p = c['args'].get('pid')
            if p: pids.add(int(p))
    return pids
