#!/bin/bash
# Real-model evaluation: Pi (main model) drives the GTK fixture through gui-gate.
# Usage: bash pi_eval.sh OUTDIR "seeds" "conditions"
#   conditions: direct full formula model gate gate_noobs
set -u
OUT=${1:-/data/workspace/runs/pi1}
SEEDS=${2:-"0 1 2"}
CONDS=${3:-"direct full formula model"}
THINK=${PI_THINK:-minimal}
TMO=${PI_TIMEOUT:-600}
mkdir -p $OUT
export PATH=$HOME/.local/bin:$PATH DISPLAY=${DISPLAY:-:1} XDG_RUNTIME_DIR=/run/user/$(id -u) \
  DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$(id -u)/bus GDK_BACKEND=x11 NO_AT_BRIDGE=0
[ -f ~/.pi/agent/proxy.env ] && . ~/.pi/agent/proxy.env
HERE=$(cd "$(dirname "$0")" && pwd)
MCP=~/.pi/agent/mcp.json
cp $MCP $OUT/mcp.json.orig
TASK='A GTK window titled "GateFixture" is open on this Linux desktop. Using only the gui tools, click its fruit buttons in exactly the order listed in the window'"'"'s "Sequence:" label. Button captions can be shuffled after a click, buttons can be briefly disabled ("Loading..."), and a "Notice" dialog can pop up; close that dialog with its OK button. The label "Progress k/N" shows how far you are. Stop as soon as it reads N/N and reply DONE.'
for s in $SEEDS; do
  for c in $CONDS; do
    tag=$c-s$s
    case $c in
      direct)  GA='"--policy","direct"';;
      full)    GA='"--policy","full"';;
      formula) GA='"--policy","gate","--decider","formula"';;
      model)   GA='"--policy","gate","--decider","model"';;
      gate)    GA='"--policy","gate","--decider","model","--human-timeout","30"';;
      gate_noobs) GA='"--policy","gate","--decider","model","--human-timeout","30","--no-auto-observe"';;
    esac
    cat > $MCP <<J
{"mcpServers":{"gui":{"command":"$HOME/.local/bin/gui-gate","args":[$GA,"--log","$OUT/gate-$tag.jsonl"],
 "exposure":"hidden","toolExposure":{"list_apps":"direct","list_windows":"direct","get_window_state":"direct",
 "click":"direct","press_key":"direct","type_text":"direct","scroll":"direct","request_human":"direct"}}}}
J
    pkill -f fixture_app.py; sleep 0.5
    setsid python3 $HERE/fixture_app.py --seed $s --length 8 --log $OUT/fixture-$tag.jsonl >/dev/null 2>&1 < /dev/null &
    sleep 2.5
    t0=$(date +%s.%N)
    (cd /tmp && timeout $TMO pi --no-session --tools 'mcp__gui__*' --thinking $THINK --mode json -p "$TASK" </dev/null > $OUT/pi-$tag.jsonl 2> $OUT/pi-$tag.err)
    t1=$(date +%s.%N)
    echo "{\"tag\":\"$tag\",\"cond\":\"$c\",\"seed\":$s,\"wall_s\":$(python3 -c "print(round($t1-$t0,1))")}" >> $OUT/runs.jsonl
    pkill -f fixture_app.py
    sleep 1
  done
done
cp $OUT/mcp.json.orig $MCP
python3 $HERE/pi_score.py $OUT > $OUT/summary.txt 2>&1
echo ALLDONE > $OUT/STATUS
