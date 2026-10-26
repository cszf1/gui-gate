pkill -TERM -f 'bench/run_bench.sh' ; sleep 1; pkill -TERM -f pi_monitor.py; pkill -TERM -x pi; pkill -f 'pi --no-session'; sleep 3
pgrep -af 'run_bench|pi_monitor|gui-gate' | head
echo "stopped by operator $(date '+%F %T') to start the full run (smoke checks done)" > /data/workspace/runs/smoke2-v099-ds41/STOPPED
cat ~/.pi/agent/mcp.json | head -c 200; echo
cd /data/workspace/gui-gate && /data/workspace/osworld/venv/bin/python bench/osworld_native.py reset >/dev/null 2>&1
R=/data/workspace/runs/osworld-verified-v099-ds41-2026-10-23
[ -e $R ] && { echo EXISTS; exit 1; }
OUT=$R LIST=/data/workspace/gui-gate/bench/tasks/ovf.txt bash /data/workspace/launch.sh
cat $R.start
