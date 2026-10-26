# usage: R=<rundir> NAME=<results folder name> bash /data/workspace/pack.sh -> /data/workspace/pack/NAME.tgz
cd /data/workspace/gui-gate
P=/data/workspace/pack/$NAME; rm -rf $P; mkdir -p $P
/data/workspace/osworld/venv/bin/python bench/score.py $R --pack $P > /dev/null 2>&1
python3 bench/osworld_verified_report.py $R --expected ${EXPECTED:-361} --md $P/report.md --json $P/report.json --csv $P/tasks.csv > /dev/null
mkdir -p $P/task $P/setup; cp $R/task/* $P/task/ 2>/dev/null
cp $R.start $P/started.txt 2>/dev/null; cp $R.launches $P/launches.txt 2>/dev/null; cp $R/preflight.txt $R/mcp.json.orig $R/STATUS $P/ 2>/dev/null
tail -200 $R.log > $P/run_bench.log.tail 2>/dev/null
cp bench/tasks/ovf.txt $P/setup/task_list_ovf.txt
cp /data/workspace/launch.sh $P/setup/launch.sh
sed -E 's/(KEY=).*/\1<redacted>/' ~/.config/gui-gate/reviewer.env > $P/setup/reviewer.env.redacted
python3 -c "import json,os;d=json.load(open(os.path.expanduser('~/.pi/agent/models.json')));d['providers']['tokenrhythm']['apiKey']='<key file>';print(json.dumps(d,indent=1))" > $P/setup/pi_models.json
cp ~/.pi/agent/settings.json $P/setup/pi_settings.json
{ echo "gui-gate $(~/.local/bin/gui-gate --version)"; ~/.local/lib/gui-gate/cua-driver/cua-driver --version 2>&1 | head -1; echo "pi $(pi --version)";
  echo "OSWorld $(cd /data/workspace/osworld/OSWorld && git log --oneline -1 | cut -c1-60)"; echo "repo $(git log --oneline -1 | cut -c1-60)";
  soffice --version | head -1; gimp --version | head -1; vlc --version 2>/dev/null | head -1; thunderbird --version; code --version | head -1; /usr/lib/chromium/chromium --version; uname -a; cat /etc/os-release | head -1; } > $P/setup/versions.txt 2>&1
cp /data/workspace/rolecheck.log $P/setup/rolecheck.log 2>/dev/null; cp /data/workspace/chkrev.json $P/setup/check_reviewer.json 2>/dev/null
grep -rlE '"data":"[A-Za-z0-9+/]{200}' $P 2>/dev/null | head -3
cd /data/workspace/pack && tar czf $NAME.tgz $NAME && ls -la $NAME.tgz && du -sh $NAME
