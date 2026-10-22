import json,glob,re
noise=re.compile(r"^(kworker/|ksoftirqd/|migration/|cpuhp/|rcu_|irq/|jbd2/|kthreadd$|pool_workqueue|kdevtmpfs$|kauditd$|khungtaskd$|oom_reaper$|kcompactd|ksmd$|khugepaged$|watchdogd$|kswapd|psimon$|idle_inject/|kblockd$|scsi_|card\d|systemd|\(sd-pam\)$|dbus-daemon$|dbus-broker|agetty$|sshd$|containerd|dockerd$|dconf-service$|at-spi|ssh-agent$|gpg-agent$|polkitd$|udisksd$|rtkit-daemon$|cron$|rsyslogd$|gvfs|xdg-permission-|xdg-document-|gui-gate$|cua-driver$)")
line=re.compile(r"^- (.+) \(pid (\d+)\)\s*$")
n=0;before=0;after=0;kept=set()
for f in glob.glob('/workspace/work/gui-gate/results/pi-ds41-v094-2026-10-21/pi/*.jsonl'):
  for l in open(f):
    try:e=json.loads(l)
    except:continue
    if e.get('type')=='tool_execution_end' and e.get('toolName','').endswith('list_apps'):
      t=e['result']['content'][0]['text']; n+=1; before+=len(t)
      out=[]
      for x in t.split('\n'):
        m=line.match(x)
        if m and noise.match(m.group(1).strip()): continue
        if m: kept.add(m.group(1))
        out.append(x)
      after+=len('\n'.join(out))+90
print(n,before,after, round(1-after/before,3)); print(sorted(kept))
