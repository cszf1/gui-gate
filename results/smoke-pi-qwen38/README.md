# smoke-pi-qwen38 (2026-10-21, box gui-pi)

1-task smoke (ow01 Calc) x direct / gate8 with Pi main model **tokenrhythm/qwen3.8-flash**, thinking **medium**,
480 s / 30 turns caps, `--human deny`, gui-gate 0.9.1, gate8 reviewer = systemone decision-model-preview.
(runs.jsonl has no `model` field yet: run_bench.sh only started recording it after this smoke.)

- direct: success (10 turns, cut at 480 s, but the evaluator passed). gate8: no success (8 turns, timeout). No reviewer calls.
- Model time per turn is about 43-47 s mean (p90 about 131-136 s), slower than deepseek-v4.1-flash on the same shots.
  The same screenshot TTFT effect applies (imgexp: qwen3.8-flash 1 PNG 29 s vs JPEG q80 15 s). See ../smoke-pi/README.md.
- 2 runs: a smoke test only.
