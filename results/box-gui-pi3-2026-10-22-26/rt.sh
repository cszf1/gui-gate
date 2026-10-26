R=/data/workspace/runs/osworld-verified-v099-ds41-2026-10-23
cd $R
grep -E '"infra":"(setup_failed|relay_error)"' runs.jsonl > runs.infra_first.jsonl
grep -vE '"infra":"(setup_failed|relay_error)"' runs.jsonl > runs.tmp && mv runs.tmp runs.jsonl
wc -l runs.infra_first.jsonl runs.jsonl; rm -f STATUS
