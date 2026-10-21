import json,base64,io,time,urllib.request
from PIL import Image
K=open('/data/home/.pi/agent/tokenrhythm.key').read().strip()
img=None
for l in open('/data/workspace/runs/smoke-pi/pi/ow01-libreoffice_calc-7efeb4b1-direct-s0.jsonl'):
  if '"image"' in l and 'toolResult' in l and 'message_end' in l:
    e=json.loads(l)
    for c in e['message']['content']:
      if c.get('type')=='image': img=c['data']; break
  if img: break
png=img
im=Image.open(io.BytesIO(base64.b64decode(png))).convert('RGB')
b=io.BytesIO(); im.save(b,'JPEG',quality=80); jpg=base64.b64encode(b.getvalue()).decode()
b=io.BytesIO(); im.resize((im.width//2,im.height//2)).save(b,'JPEG',quality=80); half=base64.b64encode(b.getvalue()).decode()
def call(model,data,mime,n):
  content=[{"type":"text","text":"How many images are attached? Reply with one number only."}]+[{"type":"image_url","image_url":{"url":f"data:{mime};base64,{data}"}}]*n
  body=json.dumps({"model":model,"messages":[{"role":"user","content":content}],"max_tokens":1000,"stream":True,"stream_options":{"include_usage":True}}).encode()
  r=urllib.request.Request("https://tokenrhythm.studio/v1/chat/completions",body,{"Authorization":"Bearer "+K,"Content-Type":"application/json"})
  t=time.time(); ttft=None; pt=None
  try:
    for line in urllib.request.urlopen(r,timeout=200):
      if ttft is None and line.startswith(b'data:') and b'choices' in line: ttft=round(time.time()-t,1)
      if b'"usage"' in line and b'prompt_tokens' in line:
        try: pt=json.loads(line[5:])['usage']['prompt_tokens']
        except Exception: pass
    return ttft,round(time.time()-t,1),pt
  except Exception as ex: return 'ERR',round(time.time()-t,1),str(ex)[:60]
print('KB png',len(png)*3//4//1024,'jpg q80',len(jpg)*3//4//1024,'half jpg',len(half)*3//4//1024,flush=True)
for M in ['deepseek-v4.1-flash-1017','qwen3.8-flash']:
 for rep in range(2):
  for name,d,m in [("png",png,"image/png"),("jpg",jpg,"image/jpeg"),("half-jpg",half,"image/jpeg")]:
   for n in (0,1,3,5):
    if name!='png' and n==0: continue
    print(M,rep,name,n,'ttft,total,prompt_tok=',call(M,d,m,n),flush=True)
print('DONE')
