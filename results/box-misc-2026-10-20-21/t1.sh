export DISPLAY=:1 XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus PATH=/data/workspace/osworld/venv/bin:$HOME/.local/bin:$PATH
cd /data/workspace/gui-gate/bench
for t in "$@"; do
 echo "=== $t"
 python osworld_native.py setup tasks/$t 2>&1 | tail -2
 wmctrl -l
 echo "" > /tmp/final.txt
 python osworld_native.py eval tasks/$t --final /tmp/final.txt 2>&1 | tail -2
done
echo T1DONE
