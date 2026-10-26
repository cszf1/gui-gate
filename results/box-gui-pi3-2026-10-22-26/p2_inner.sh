set -x
export DISPLAY=:1 XDG_RUNTIME_DIR=/run/user/$(id -u) DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$(id -u)/bus
H=$HOME; mkdir -p $H/.config/gui-gate; chmod 700 $H/.config/gui-gate
cp $H/.pi/agent/tokenrhythm.key $H/.config/gui-gate/role.key; chmod 600 $H/.config/gui-gate/role.key
{ echo GUI_GATE_REVIEWER=api
  echo GUI_GATE_REVIEWER_PROTOCOL=systemone
  echo GUI_GATE_REVIEWER_URL=https://maas.qianwenaiapi.com/compatible-mode/v1
  echo GUI_GATE_REVIEWER_MODEL=decision-model-preview
  echo GUI_GATE_REVIEWER_KEY_FILE=$H/.config/gui-gate/reviewer.key
  echo GUI_GATE_REVIEWER_TIMEOUT=20
  for R in FEASIBILITY PLANNER VERIFIER JUDGE SEARCH COMPACT GROUNDER; do
    echo GUI_GATE_ROLE_${R}_URL=https://tokenrhythm.studio/v1
    echo GUI_GATE_ROLE_${R}_MODEL=deepseek-v4.1-flash-1017
    echo GUI_GATE_ROLE_${R}_PROTOCOL=chat
    echo GUI_GATE_ROLE_${R}_KEY_FILE=$H/.config/gui-gate/role.key
  done; } > $H/.config/gui-gate/reviewer.env
chmod 600 $H/.config/gui-gate/reviewer.env
curl -fsSL https://github.com/cszf1/gui-gate/releases/latest/download/install.sh | bash -s -- --client pi --no-prompt
echo INSTALL_RC=$?
export PATH=$H/.local/bin:$PATH
gui-gate --version; cua-driver --version
gui-gate check-reviewer > /data/workspace/chkrev.json 2>&1; echo CHECKREV_RC=$?
grep -E '"via"|"got"|"expected"|"ok"' /data/workspace/chkrev.json | tr -s ' ' | head -20
cat $H/.pi/agent/mcp.json
python3 - <<'PY'
import json,os
H=os.path.expanduser('~/.pi/agent')
m={"providers":{"tokenrhythm":{"baseUrl":"https://tokenrhythm.studio/v1","api":"openai-completions","apiKey":"!cat ~/.pi/agent/tokenrhythm.key",
 "models":[{"id":"deepseek-v4.1-flash-1017","name":"deepseek-v4.1-flash-1017","input":["text","image"],"reasoning":True,"contextWindow":1000000,"maxTokens":128000,
 "compat":{"supportsDeveloperRole":False,"requiresReasoningContentOnAssistantMessages":False},
 "thinkingLevelMap":{"minimal":"minimal","low":"low","medium":"medium","high":"high","xhigh":"xhigh","max":"max"},
 "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0}}]}}}
open(H+'/models.json','w').write(json.dumps(m,indent=1)); os.chmod(H+'/models.json',0o600)
open(H+'/settings.json','w').write(json.dumps({"defaultProvider":"tokenrhythm","defaultModel":"deepseek-v4.1-flash-1017","defaultThinkingLevel":"medium"})); os.chmod(H+'/settings.json',0o600)
PY
cd /data/workspace && rm -rf gui-gate && git clone -q https://github.com/cszf1/gui-gate.git && cd gui-gate && git log --oneline -1 | cut -c1-60
cp -a /data/workspace/overlay/bench/. bench/
mkdir -p /data/workspace/osworld && python3 -m venv --system-site-packages /data/workspace/osworld/venv
V=/data/workspace/osworld/venv/bin
$V/pip install -q -r /data/workspace/req.txt > /data/workspace/osworld/pip.log 2>&1; echo PIP_RC=$?
$V/pip install -q --index-url https://download.pytorch.org/whl/cpu torch torchvision >> /data/workspace/osworld/pip.log 2>&1; echo TORCH_RC=$?
$V/pip install -q playwright librosa easyocr 'borb<3' pyacoustid >> /data/workspace/osworld/pip.log 2>&1; echo EXTRA_RC=$?
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y libchromaprint-tools > /dev/null 2>&1; echo FPCALC_RC=$?
$V/python -c "import playwright,librosa,easyocr,borb,acoustid,torch;print('deps ok',torch.__version__)"
OSWORLD_COMMIT=f9885c9 bash /data/workspace/gui-gate/bench/box_setup.sh
$V/python /data/workspace/gui-gate/bench/make_verified_list.py /data/workspace/osworld/OSWorld --shuffle 0
cat /usr/local/bin/google-chrome; readlink -f /data/workspace/osworld/bin/chrome
df -h / /data
echo P2_DONE
