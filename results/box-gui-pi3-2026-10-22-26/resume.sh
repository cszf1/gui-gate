# idempotent: desktop + OSWorld server up, then resume the full run if it is not running and not finished
export DISPLAY=:1 XDG_RUNTIME_DIR=/run/user/$(id -u) DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$(id -u)/bus
R=/data/workspace/runs/osworld-verified-v099-ds41-2026-10-23
if ! xdpyinfo >/dev/null 2>&1; then printf 'start\n1440x900\n\n' | instabox-desktop >/dev/null 2>&1; sleep 5; fi
xdpyinfo >/dev/null 2>&1 && echo desktop_ok || echo desktop_FAIL
bash /data/workspace/gui-gate/bench/box_setup.sh 2>&1 | tail -2
if [ -f $R/STATUS ]; then echo "STATUS $(cat $R/STATUS)"; exit 0; fi
if pgrep -f 'bench/run_bench.sh' >/dev/null; then echo already_running; exit 0; fi
pkill -f pi_monitor.py; pkill -x pi; sleep 1
OUT=$R LIST=/data/workspace/gui-gate/bench/tasks/ovf.txt bash /data/workspace/launch.sh
