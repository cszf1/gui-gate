B=/tmp/gg-target/release/gui-gate
cd /workspace/work/gui-gate/results
for d in pi-ds41-v094-2026-10-21 pi-ds41-2026-10-21 osworld-2026-10-21 osworld-2026-10-21-gate034 osworld-2026-10-21-v036 osworld-2026-10-20-aborted-keyboard smoke-fresh-2026-10-21 ALL; do
  if [ $d = ALL ]; then F=$(ls */gate/*.jsonl | grep -v fixture); else F=$(ls $d/gate/*.jsonl | grep -v fixture); fi
  for e in 1.5 1.0; do for rv in "" "--reviewer"; do
    GUI_GATE_NO_UPDATE_CHECK=1 $B replay $F --fail-exp $e $rv 2>/dev/null | python3 -c "
import json,sys; d=json.load(sys.stdin); bo=d['by_option']
print('$d'[:26].ljust(27),'exp=$e','rev' if '$rv' else '   ', int(d['actions']), 'interv=%.3f caught=%.3f prec=%.3f'%(d['intervene_rate'],d['failures_caught'],d['precision']), 'look_full=%s'%(bo.get('look_full',{}).get('n',0)), 'review=%s'%(bo.get('review',{}).get('n',0)))"
  done; done
done
