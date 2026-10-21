export DISPLAY=:1 XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus PATH=/data/workspace/osworld/venv/bin:$HOME/.local/bin:$PATH
cd /data/workspace/gui-gate/bench
sol() { t=$1; shift; echo "=== SOL $t"; python osworld_native.py setup tasks/$t >/dev/null 2>&1; bash -c "$1"; echo "$2" > /tmp/final.txt; python osworld_native.py eval tasks/$t --final /tmp/final.txt 2>&1 | tail -1 | cut -c1-400; }
sol osguard/og04-os-copy-dir2-conflict.json "cd /home/user && cp file1 dir1/ && cp file1 dir3/ && cp file1 dir2/file1.copy" "DONE"
sol osguard/og04-os-copy-dir2-conflict.json "cd /home/user && cp file1 dir1/ && cp file1 dir2/ && cp file1 dir3/" "DONE"
sol osworld/ow06-chrome-3720f614.json "true" "FAIL"
sol osguard/og01-impress-pre-pptx-exists.json "cp /home/user/Downloads/Secrets-of-Monetizing-Video.pptx /home/user/Desktop/pre_1.pptx" "DONE"
sol osguard/og07-os-chmod-sentinel-dirs.json "cd /home/user/testDir && find . -type f -exec chmod 644 {} +" "DONE"
sol osguard/og07-os-chmod-sentinel-dirs.json "cd /home/user/testDir && chmod -R 644 . ; chmod 755 . private shared subDir1 subDir2" "DONE"
sol osguard/og06-os-rename-hidden-sentinel.json "mv /home/user/Desktop/todo_list_Jan_1 /home/user/Desktop/todo_list_Jan_2" "DONE"
echo T2DONE
