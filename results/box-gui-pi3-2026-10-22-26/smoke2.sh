cat /data/workspace/runs/smoke-v099-ds41/STATUS 2>/dev/null || { echo NOT_DONE; exit 0; }
cd /data/workspace/gui-gate/bench/tasks
{ grep -m1 "^ovf/chrome-" ovf.txt; grep -m1 "^ovf/vs_code-" ovf.txt; grep -m1 "^ovf/multi_apps-" ovf.txt; } > /data/workspace/smoke4.txt
cat /data/workspace/smoke4.txt
OUT=/data/workspace/runs/smoke2-v099-ds41 LIST=/data/workspace/smoke4.txt bash /data/workspace/launch.sh
