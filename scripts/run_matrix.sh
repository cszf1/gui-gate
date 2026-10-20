#!/bin/bash
# Pilot on a real desktop: same seeds under each policy, plus training data and a trained gate.
# Usage: bash run_matrix.sh OUTDIR
set -u
OUT=${1:-/data/workspace/runs/pilot}
mkdir -p $OUT/logs $OUT/train_logs
export PATH=$HOME/.local/bin:$PATH DISPLAY=${DISPLAY:-:1} XDG_RUNTIME_DIR=/run/user/$(id -u) DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$(id -u)/bus GDK_BACKEND=x11 NO_AT_BRIDGE=0
cd "$(dirname "$0")"
SEEDS_EVAL=${SEEDS_EVAL:-"0 1 2 3 4 5 6 7 8 9 10 11"}
SEEDS_TRAIN=${SEEDS_TRAIN:-"$(seq -s ' ' 100 129)"}
# 1) training data: no intervention, every action shadow-labelled
for s in $SEEDS_TRAIN; do
  timeout 120 python3 sim_agent.py --server "gui-gate --policy direct --collect --log $OUT/train_logs/s$s.jsonl" --seed $s --fixture-log $OUT/train_logs/s$s.fixture.jsonl --out $OUT/train_runs.jsonl >/dev/null 2>&1
done
PYTHONPATH=.. python3 -m gui_gate.train $OUT/train_logs --l2 0.02 -o $OUT/weights_pilot.json > $OUT/train_report.json 2>&1
# 2) evaluation: same seeds under every policy
for s in $SEEDS_EVAL; do
  timeout 120 python3 sim_agent.py --server "cua-driver mcp" --seed $s --fixture-log $OUT/logs/raw-s$s.fixture.jsonl --out $OUT/eval.jsonl >/dev/null 2>&1
  for pol in direct full light gate; do
    timeout 120 python3 sim_agent.py --server "gui-gate --policy $pol --log $OUT/logs/$pol-s$s.jsonl" --seed $s --fixture-log $OUT/logs/$pol-s$s.fixture.jsonl --out $OUT/eval.jsonl >/dev/null 2>&1
  done
  timeout 120 python3 sim_agent.py --server "gui-gate --policy gate --weights $OUT/weights_pilot.json --log $OUT/logs/gatetrained-s$s.jsonl" --seed $s --fixture-log $OUT/logs/gatetrained-s$s.fixture.jsonl --out $OUT/eval.jsonl >/dev/null 2>&1
done
echo ALLDONE > $OUT/STATUS
