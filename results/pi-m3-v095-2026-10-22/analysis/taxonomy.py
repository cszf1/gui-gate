#!/usr/bin/env python3
"""Primary failure cause per failed run (rule-based, rules from reading the transcripts; see docs/opt-2026-10-22-m3.md).
Order matters: the first matching rule is the primary cause; secondary flags are listed too."""
import csv,json,os,collections
D=os.path.dirname(os.path.abspath(__file__))+'/..'
R=list(csv.DictReader(open(D+'/analysis/runfeat.csv')))
GRID={'ow01','ow05','ow08','ow12','ow16','ow18','ow20','ow22','ow24','og03','og09'}   # Calc: cell grid not enumerated in the tree
TERM={'og04','og05','og06','og07','og10'}                                              # terminal: output not in the tree
VISION={'ow17','ow21','ow10'}                                                          # GIMP canvas / VLC theme colour
# reviewer step-denies on a step that was needed (hand-labelled from gate logs, see doc)
NEEDED_DENY={'og03-calc-csv-exists-gate8-s1','ow02-libreoffice_writer-0810415c-gate8-s1','ow23-libreoffice_impress-455d3c66-gate8-s0',
             'ow23-libreoffice_impress-455d3c66-gate8-s1','ow13-libreoffice_impress-986fc832-gate8-s0','ow13-libreoffice_impress-986fc832-gate8-s1'}  # denied, needed, then abandoned by the model
def cls(r):
    t=r['task']; flags=[]
    if int(r['maxrep'])>=5 or float(r['read_frac'])>=0.6: flags.append('loop')
    if int(r['errs'])>=5: flags.append('tool_errors')
    if int(r['noshot'])>0: flags.append('pixel_without_frame')
    if r['done']=='1': return 'premature_done',flags
    if r['tag'] in NEEDED_DENY: return 'gate_blocked_needed_step',flags
    if t in TERM: return 'tree_missing:terminal_output',flags
    if t in VISION: return 'needs_vision',flags
    if t in GRID: return 'tree_missing:spreadsheet_grid',flags
    if 'loop' in flags: return 'loop_repeat',flags
    if 'tool_errors' in flags: return 'tool_errors',flags
    return 'other:stalled_no_progress',flags
out=collections.defaultdict(collections.Counter); fl=collections.defaultdict(collections.Counter); rows=[]
for r in R:
    if r['ok']=='1': continue
    c,f=cls(r); out[r['cond']][c]+=1
    for x in f: fl[r['cond']][x]+=1
    rows.append((r['tag'],r['cond'],c,'+'.join(f),r['stop'],r['final']))
with open(D+'/analysis/taxonomy.csv','w') as fh:
    w=csv.writer(fh); w.writerow(['tag','cond','primary','flags','stop','final']); w.writerows(rows)
for c in out: print(c,sum(out[c].values()),dict(out[c].most_common())); print('  flags',dict(fl[c]))
