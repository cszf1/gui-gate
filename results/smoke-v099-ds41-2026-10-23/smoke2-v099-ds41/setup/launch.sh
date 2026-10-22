# usage: OUT=<dir> LIST=<file> bash launch.sh   (resumable: same command again skips finished tags)
cd /data/workspace/gui-gate; export PATH=$HOME/.local/bin:$PATH
mkdir -p /data/workspace/osworld/state; touch /data/workspace/osworld/state/gate_memory_empty.jsonl
F=""; for f in INSPECT JOBS PHANTOM TWO_STRIKES FEASIBILITY_FIRST STATE_PLAN VERIFY_PLUS SKILLS SNAP PICK DIGEST MICRO_VERIFY; do F="$F\"GUI_GATE_$f\":\"1\","; done
export PI_TOOLS_096=1 PI_TOOLS_097=1 PI_APP_HINTS=0 PI_OBS_PRUNE=0 PI_TIMEOUT=3600 PI_THINK=medium PI_ATTEMPTS=3 PI_MCP_TIMEOUT=600 PI_SETUP_RETRY=1
export PI_GATE_ENV="${F}\"GUI_GATE_MEMORY\":\"/data/workspace/osworld/state/gate_memory_empty.jsonl\""
[ -f $OUT.start ] || date '+%F %T %Z' > $OUT.start
echo "$(date '+%F %T') launch gg=$(gui-gate --version) list=$LIST" >> $OUT.launches
setsid nohup bash bench/run_bench.sh --max-turns 100 $OUT gate8 $LIST 0 >> $OUT.log 2>&1 < /dev/null &
sleep 2; pgrep -af run_bench | head -2
