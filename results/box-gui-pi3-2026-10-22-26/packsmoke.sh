cd /data/workspace
for n in smoke-v099-ds41 smoke2-v099-ds41; do R=/data/workspace/runs/$n NAME=$n EXPECTED=3 bash /data/workspace/pack.sh > /data/workspace/pack/$n.packlog 2>&1; done
cp /data/workspace/runs/smoke2-v099-ds41/STOPPED /data/workspace/pack/smoke2-v099-ds41/ 2>/dev/null
mkdir -p /data/workspace/pack/smoke-v099-ds41-2026-10-23 && rm -rf /data/workspace/pack/smoke-v099-ds41-2026-10-23/*
mv /data/workspace/pack/smoke-v099-ds41 /data/workspace/pack/smoke2-v099-ds41 /data/workspace/pack/smoke-v099-ds41-2026-10-23/
mkdir -p /data/workspace/pack/smoke-v099-ds41-2026-10-23/rolecheck && cp /data/workspace/rolecheck.log /data/workspace/rolecheck/gate.jsonl /data/workspace/rolecheck/task.txt /data/workspace/pack/smoke-v099-ds41-2026-10-23/rolecheck/
cp /data/workspace/rolecheck_full.sh /data/workspace/pack/smoke-v099-ds41-2026-10-23/rolecheck/rolecheck.sh
cd /data/workspace/pack && tar czf smoke-v099-ds41-2026-10-23.tgz smoke-v099-ds41-2026-10-23 && ls -la smoke-v099-ds41-2026-10-23.tgz
