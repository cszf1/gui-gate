# Real Pi run, 2026-10-20 23:20 CST (gui-gate 0.2.0)

- Model: deepseek-v4-flash via AgentRouter relay, Pi 1.3.0, thinking low.
- Task: GateFixture 8-step form (relabel / loading / popup hazards), seeds 0-2.
- Conditions: direct (no gate, raw cua-driver), full (screenshot every step), gate (gui-gate --policy gate --decider model, core toolset, auto-observe).
- Files: summary.txt (aggregate), runs.jsonl (one row per run), pi-*.jsonl (Pi event stream, screenshots stripped), gate-*.jsonl (gate decision log), fixture-*.jsonl (app-side ground truth).
- Note: gate-s2 ended by relay error 400 "content[].thinking must be passed back" — an infrastructure failure of the relay, not a gate decision.

| cond | done | wall s | tool calls | shots | tok_in |
|---|---|---|---|---|---|
| direct | 3/3 | 86.6 | 22.0 | 10.3 | 151K |
| full | 3/3 | 71.2 | 22.3 | 10.3 | 175K |
| gate | 2/3 (1 infra) | 46.4 on finished runs | 13.5 | 1.0 | 79K |
