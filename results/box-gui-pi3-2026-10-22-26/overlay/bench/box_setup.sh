#!/bin/bash
# One-time (idempotent) preparation of a plain Linux desktop box so OSWorld tasks
# run natively, without the OSWorld VM: OSWorld's own in-guest HTTP server
# (desktop_env/server/main.py) runs on this desktop, and OSWorld's own
# SetupController / getters / metrics talk to it at 127.0.0.1:5000.
# Box-only data lives in $OSW (outside the repo copy, which deploys wipe).
set -u
OSW=${OSW:-/data/workspace/osworld}
OSWORLD_COMMIT=${OSWORLD_COMMIT:-1a572e627ea0593587f57831a4caf5f5c99d3daf}
HERE=$(cd "$(dirname "$0")" && pwd)
export DISPLAY=${DISPLAY:-:1} XDG_RUNTIME_DIR=/run/user/$(id -u) DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$(id -u)/bus
mkdir -p $OSW/state $OSW/cache $OSW/bin

# 1. OSWorld paths assume /home/user: point it at this user's home.
[ -e /home/user ] || sudo ln -s "$HOME" /home/user
# 2. OSWorld source at a pinned commit (task JSONs + desktop_env).
if [ ! -d $OSW/OSWorld/desktop_env ]; then
  git clone -q --filter=blob:none --sparse https://github.com/xlang-ai/OSWorld.git $OSW/OSWorld
  (cd $OSW/OSWorld && git sparse-checkout set evaluation_examples desktop_env && git checkout -q $OSWORLD_COMMIT)
fi
# 3. heavy optional evaluator deps (torch OCR, audio, Google Drive, Playwright) -> stubs,
#    appended AFTER site-packages so a real install always wins.
SP=$($OSW/venv/bin/python -c 'import site;print(site.getsitepackages()[0])')
echo "$HERE/stubs" > $SP/zz_osworld_stubs.pth
# 4. google-chrome -> real Google Chrome when installed (/opt/google/chrome/chrome, as in the OSWorld VM), else
#    chromium; either way the process is named "chrome" (OSWorld postconfigs `pkill chrome`)
CHROME_REAL=/opt/google/chrome/chrome; [ -x $CHROME_REAL ] || CHROME_REAL=/usr/lib/chromium/chromium
ln -sf $CHROME_REAL $OSW/bin/chrome
sudo tee /usr/local/bin/google-chrome >/dev/null <<X
#!/bin/bash
exec $OSW/bin/chrome --user-data-dir=\$HOME/.config/google-chrome --no-first-run --no-default-browser-check --password-store=basic --force-renderer-accessibility "\$@"
X
sudo chmod +x /usr/local/bin/google-chrome
# 5. gnome-terminal -> xfce4-terminal (Debian/xfce box)
sudo tee /usr/local/bin/gnome-terminal >/dev/null <<'X'
#!/bin/bash
args=()
for a in "$@"; do case "$a" in --working-directory=*) args+=("--working-directory=${a#*=}");; --maximize) args+=("--maximize");; --) ;; *) args+=("$a");; esac; done
exec xfce4-terminal "${args[@]}"
X
sudo chmod +x /usr/local/bin/gnome-terminal
command -v python >/dev/null || sudo ln -sf /usr/bin/python3 /usr/local/bin/python
# 6. LibreOffice profile like the OSWorld image: no alien-format nag, no tips, no recovery dialog.
LO=$HOME/.config/libreoffice/4/user
if [ ! -f $OSW/state/lo_profile_ready ]; then
  rm -rf $HOME/.config/libreoffice
  timeout 60 soffice --headless --terminate_after_init >/dev/null 2>&1
  python3 - "$LO/registrymodifications.xcu" <<'PY'
import sys
p = sys.argv[1]
items = [
 ('/org.openoffice.Office.Common/Save/Document', 'WarnAlienFormat', 'boolean', 'false'),
 ('/org.openoffice.Office.Common/Misc', 'ShowTipOfTheDay', 'boolean', 'false'),
 ('/org.openoffice.Office.Common/Misc', 'FirstRun', 'boolean', 'false'),
 ('/org.openoffice.Office.Recovery/RecoveryInfo', 'Enabled', 'boolean', 'false'),
 ('/org.openoffice.Office.Recovery/AutoSave', 'Enabled', 'boolean', 'false'),
 ('/org.openoffice.Setup/Office', 'ooSetupInstCompleted', 'boolean', 'true'),
 ('/org.openoffice.Office.Common/Misc', 'UseSystemFileDialog', 'boolean', 'false'),
]
s = open(p, encoding='utf-8').read()
add = ''.join(f'<item oor:path="{a}"><prop oor:name="{b}" oor:op="fuse"><value>{d}</value></prop></item>\n' for a,b,c,d in items)
s = s.replace('</oor:items>', add + '</oor:items>')
open(p, 'w', encoding='utf-8').write(s)
PY
  touch $OSW/state/lo_profile_ready
fi
# 6b. VLC: skip the first-run privacy/network dialog (preset in the OSWorld image)
mkdir -p $HOME/.config/vlc
[ -f $HOME/.config/vlc/vlcrc ] || printf '[qt]\nqt-privacy-ask=0\n' > $HOME/.config/vlc/vlcrc
# 7. pristine snapshot of everything a task may touch, restored before every task.
if [ ! -f $OSW/state/baseline_ready ]; then
  mkdir -p $OSW/baseline
  rm -rf $OSW/baseline/*
  for d in .config/libreoffice .config/GIMP .config/vlc .config/xfce4/terminal Desktop Documents Downloads Pictures Music Videos; do
    if [ -e "$HOME/$d" ]; then mkdir -p "$OSW/baseline/$(dirname $d)"; cp -a "$HOME/$d" "$OSW/baseline/$d"; fi
  done
  (cd $HOME && ls -A) > $OSW/state/home_baseline.txt
  touch $OSW/state/baseline_ready
fi
# 8. OSWorld in-guest server on :5000 (cwd = home, like the VM)
if ! curl -s -m 3 http://127.0.0.1:5000/platform >/dev/null; then
  cd $HOME && PATH=$OSW/venv/bin:$PATH setsid nohup $OSW/venv/bin/python $OSW/OSWorld/desktop_env/server/main.py > $OSW/server.log 2>&1 < /dev/null &
  sleep 4
fi
curl -s -m 3 http://127.0.0.1:5000/platform; echo
echo BOX_SETUP_DONE
