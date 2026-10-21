export DISPLAY=:1 XDG_RUNTIME_DIR=/run/user/$(id -u) DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$(id -u)/bus GDK_BACKEND=x11 NO_AT_BRIDGE=0
PY=/data/workspace/osworld/venv/bin/python; cd /data/workspace/gui-gate; R=/data/workspace/shotexp/raw3
fix() { xrandr -s 1440x900 >/dev/null 2>&1 || { xrandr --newmode 1440x900_60 106.50 1440 1528 1672 1904 900 903 909 934 -hsync +vsync; xrandr --addmode VNC-0 1440x900_60; xrandr --output VNC-0 --mode 1440x900_60; } >/dev/null 2>&1; }
for t in ow04-chrome-af630914 ow06-chrome-3720f614 ow17-gimp-045bf3ff ow21-gimp-734d6579 ow10-vlc-d06f0d4d; do
  fix
  timeout 120 $PY bench/osworld_native.py setup bench/tasks/osworld/$t.json --out $R/$t.setup.json >/dev/null 2>&1
  fix; echo "screen $(xdpyinfo | awk '/dimensions/{print $2}')"
  (cd /tmp && timeout 60 python3 /data/workspace/shotexp/collect2.py $t $R/$t) 2>&1 | tail -1 | cut -c1-120
  timeout 60 $PY /data/workspace/shotexp/collect.py $t $R/$t.full 0 2>&1 | tail -1
done
$PY bench/osworld_native.py reset >/dev/null 2>&1
echo COLLECT4DONE
