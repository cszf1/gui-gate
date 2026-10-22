import json,sys,glob
sys.path.insert(0,'.')
from diffsim import results
seq={}
for l in open('/tmp/diffseq.jsonl'):
    r=json.loads(l); seq[r['id']]=r
G=[f for f in sorted(glob.glob('/workspace/work/gui-gate/results/pi-ds41-v094-2026-10-21/pi/*gate8*.jsonl'))]
D=[f for f in sorted(glob.glob('/workspace/work/gui-gate/results/pi-ds41-v094-2026-10-21/pi/*direct*.jsonl'))]
def total(files, sent=None, apps=False):
    tot=0
    for fi,f in enumerate(files):
        R=results(f); n=len(R)
        for i,texts in enumerate(R):
            for j,t in enumerate(texts):
                L=len(t); k=f'{fi}:{i}:{j}'
                if sent is not None and k in sent and seq[k]['kind']=='auto': L=sent[k]
                if apps and t.startswith('\u2705 Found') and 'app(s)' in t: L=int(L*0.2)
                tot+=L*(n-i)
    return tot/1e6
print('direct', round(total(D),1), 'direct+apps filter', round(total(D,apps=True),1))
print('gate8 old', round(total(G),1))
for thr in ['0.3','0.5','0.7','1.01']:
    sent={}
    for l in open(f'/tmp/diffout_{thr}.tsv'):
        k,c=l.split('\t'); sent[k]=int(c)
    a=sum(seq[k]['full_chars']*seq[k]['rem'] for k in sent)/1e6; b=sum(sent[k]*seq[k]['rem'] for k in sent)/1e6
    print('thr',thr,'auto-obs ctx %.1fM -> %.1fM'%(a,b),'gate8 new %.1f'%total(G,sent),'+apps %.1f'%total(G,sent,True), 'diffed blocks', sum(1 for k in sent if sent[k]<seq[k]['full_chars']-300))
