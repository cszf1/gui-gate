import json,requests,time,os
d=json.load(open('/tmp/peval.json'))
key=open(os.path.expanduser('~/.config/gui-gate/reviewer.key')).read().strip()
px={"https":"http://127.0.0.1:10809"}
for name,sysp in (("old",d["old"]),("new",d["new"])):
  ok=0
  for (n,task,label,win,exp) in d["cases"]:
    action={"tool":"click","target_role":"button","target_label":label,"risk_level":"medium","window_titles":["Mail"],"dialog_text":win}
    prompt=f"User's request (verbatim):\n{task}\n\nProposed action:\n{json.dumps(action,indent=2)}"
    t=time.time()
    r=requests.post("https://agentrouter.org/v1/chat/completions",proxies=px,timeout=60,headers={"Authorization":"Bearer "+key,"User-Agent":"codex_cli_rs/0.160.0"},
      json={"model":"deepseek-v4-flash","temperature":0,"max_tokens":1024,"messages":[{"role":"system","content":sysp},{"role":"user","content":prompt}]})
    try: c=r.json()["choices"][0]["message"]["content"]; dec=json.loads(c[c.index("{"):c.rindex("}")+1])["decision"]
    except Exception as e: dec="ERR "+r.text[:80]
    ok+=dec==exp
    print(name,"|",n,"| exp",exp,"| got",dec,"|",round(time.time()-t,1),"s")
  print(name,"correct",ok,"/",len(d["cases"]))
