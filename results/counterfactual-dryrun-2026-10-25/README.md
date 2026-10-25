# counterfactual-dryrun-2026-10-25: offline dry run of the P1 protocol (NOT model data)

**What this is:** the P1 counterfactual harness (`bench/counterfactual/`) run end to end on the serialisable fixture
twin with a **scripted controller** standing in for the acting model. No desktop, no model, no network. Every number
here exercises the protocol, the restore checks, the task-family split, the statistics and the reliability plot; **it
says nothing about real agents or about gui-gate's value model**. Not an OSWorld result of any kind.

Command (deterministic; regenerates `branches.jsonl` / `checkpoints.jsonl` / `preds.jsonl`, not committed, ~6.6 MB):

```bash
python3 bench/counterfactual/dryrun.py --out DIR --seeds 5 --prefixes 2,4,6 --repeats 3 --unseen-layouts 12
python3 bench/counterfactual/analyze.py DIR/branches.jsonl --pred DIR/preds.jsonl --plot DIR/reliability_all.png
```

- 60 dev-layout checkpoints (4 hazard families x 5 seeds x 3 prefix lengths) x 7 treatments x 3 repeats = 1260
  branches; 180 unseen-layout checkpoints (12 held-out layouts, `bench/counterfactual/layouts.py`) = 3780 branches;
  0 unfaithful restores (every restore hash-verified).
- Family split (`split.json`): train families `mixed`, `relabel`; test families `loading`, `popup`; the 12 unseen
  layouts are evaluation-only.
- `preds.jsonl` (not committed) = a toy cell-mean increment predictor fitted on **train-family branches only**
  (treatment x the controller's blind-action count x candidate kind x early/late), applied everywhere.
- `reliability.png` / `.csv`: incremental reliability on the **test families** (equal-mass bins of predicted Δ, ties
  never split, paired observed Δ, task-family bootstrap 95% CI). `reliability_unseen.png` / `.csv`: the same on the
  unseen layouts. `reliability_ascii.txt`: the matplotlib-free fallback of the test-family diagram.
- `analysis.md` / `analysis.json`: paired Δ per treatment (step and task level), negative / zero / positive counts,
  look_light − matched_wait, and the reliability tables.

What the dry run shows about the **protocol** (not about agents): negative increments occur and are kept
(extra_reason, ask at task level); matched_wait separates waiting from information; with two test families the CIs
are wide, as they should be. Protocol and pilot sizes: `docs/eval-protocol.md`.
