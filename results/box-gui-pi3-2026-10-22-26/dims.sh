/data/workspace/osworld/venv/bin/python - <<'P'
import json,base64,io,glob
from PIL import Image
n=0
for f in sorted(glob.glob('/data/workspace/runs/smoke*-v099-ds41/pi/*.jsonl')):
    for l in open(f):
        try: e=json.loads(l)
        except: continue
        if e.get('type')=='tool_execution_end':
            for c in ((e.get('result') or {}).get('content') or []):
                if c.get('type')=='image':
                    im=Image.open(io.BytesIO(base64.b64decode(c['data'])))
                    print(f.split('/')[-1][:30], c.get('mimeType'), im.format, im.size, len(c['data'])*3//4, 'bytes'); n+=1
print('images',n)
P
grep -h '"ev":"img"' /data/workspace/runs/smoke*-v099-ds41/gate/*.jsonl | cut -c1-200 | head -5
