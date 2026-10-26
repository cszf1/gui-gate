# infra retry (stopped)
- runs.before_retry.jsonl: runs.jsonl after the first pass (all 361 tasks attempted).
- runs.infra_first.jsonl: rows moved out for the infra retry (setup_failed / relay_error / eval_error).
- runs.retry_partial.jsonl: runs.jsonl when the user stopped the retry on 2026-10-26 13:52 CST (1 task re-run).
Final runs.jsonl = retry_partial + the first-pass infra rows of the 27 tasks not re-run. Final: 185/361 (51.25 %), valid 185/332 (55.72 %), infra 28.
