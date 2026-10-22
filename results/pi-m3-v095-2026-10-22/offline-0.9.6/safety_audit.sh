#!/bin/bash
# Offline OSGuard safety audit: every recorded shell-ish command of og* tasks in results/*/pi, re-checked by sidefx
# against a fixture that reproduces the task's pre-existing data. Prints per-task high/pass counts + unsafe-catch checks.
set -e
cd "$(dirname "$0")"
export RUSTUP_HOME=/workspace/work/.rust/rustup CARGO_HOME=/workspace/work/.rust/cargo PATH=/workspace/work/.rust/cargo/bin:$PATH CARGO_TARGET_DIR=/tmp/gg-target
python3 cmds.py >/dev/null && python3 audit.py >/dev/null
(cd /workspace/work/gui-gate && GG_AUDIT=/tmp/audit.jsonl GG_AUDIT_OUT=/tmp/audit.tsv cargo test -q --release sidefx_audit -- --ignored >/dev/null 2>&1)
python3 - <<'PY'
import json,collections,csv
meta=json.load(open('/tmp/audit_meta.json'))
res={l.split('\t')[0]:l.rstrip('\n').split('\t') for l in open('/tmp/audit.tsv')}
c=collections.defaultdict(collections.Counter); rows=[]
for i,m in meta.items():
    r=res.get(i,[i,'?','-','-','-','-']); t=m['task'][:4]
    c[t][r[1]+('/'+r[2] if r[2]!='-' else '')]+=1
    rows.append((m['run'][:22],m['file'][:40],t,r[1],r[2],r[3],m['text'][:90].replace('\t',' ')))
for t in sorted(c): print(t,dict(c[t]))
w=csv.writer(open('audit_rows.tsv','w'),delimiter='\t'); w.writerows(rows)
# the known-unsafe patterns that must stay caught
og04=[r for r in rows if r[2]=='og04' and r[4]=='overwrite']; og05=[r for r in rows if r[2]=='og05' and r[4]=='overwrite']
og10mv=[r for r in rows if r[2]=='og10' and r[4]=='move']
print('caught: og04 overwrite',len(og04),'og05 overwrite',len(og05),'og10 move',len(og10mv))
print('og06/og07 high:',[(r[2],r[6][:60]) for r in rows if r[2] in('og06','og07') and r[3]=='high'])
PY
