# bench/counterfactual/: P1 counterfactual harness

Goal: measure, for a state x reached in a real run, Δ_o(x) = P(Y_o = 1 | x, π, B) − P(Y_direct = 1 | x, π, B), by
running the SAME step under every treatment from the same checkpoint. Only such paired branches can say whether an
extra observation or extra reasoning is worth its cost, including when it is worth **nothing or less than nothing**.
The gate's logged decisions cannot: every logged state has the outcome of one option only, and which option was taken
depended on the logged model (confounding).

**Status: code and an offline dry run only.** No real-model branches have been collected. Every number the dry run
prints comes from a scripted controller on a fixture twin; it exercises the protocol and the statistics, it says
nothing about real agents. The gate's treatment-interaction model (src/value.rs) stays an observational estimate until
this harness has produced real branches and the model is retrained on them.

## Files

| file | what |
|---|---|
| `fixture_env.py` | serialisable twin of `scripts/fixture_app.py` (same task and hazards: relabel / loading / popup), virtual time, the full state incl. the RNG is JSON, `state_hash()` |
| `fixture_driver.py` | the twin behind an MCP stdio driver (modelled on `tests/fake_driver.py`) so branches can run through the real `gui-gate` binary |
| `harness.py` | controller (scripted stand-in for the acting model), treatments, checkpoints, restore (snapshot / replay) with hash check, branching, continuation with a fixed policy π |
| `analyze.py` | paired Δ per checkpoint, task-family bootstrap CIs, negative / zero / positive counts, look_light − matched_wait, incremental reliability table vs predicted Δ |
| `dryrun.py` | offline end-to-end run (sim backend, or `--gate target/release/gui-gate`) with a task-family split |
| `osworld.py` | OSWorld checkpoints: setup + replay of the logged Pi tool-call prefix + state fingerprint (code only, never run) |

```bash
python3 bench/counterfactual/dryrun.py --out /tmp/cf --seeds 3 --repeats 2                        # in-process twin
python3 bench/counterfactual/dryrun.py --out /tmp/cf --gate target/release/gui-gate --restore replay # through the real gate
python3 bench/counterfactual/analyze.py /tmp/cf/branches.jsonl --level task --pred preds.jsonl
```

## Protocol

1. **Checkpoint** right after the controller produced its candidate action: environment state, candidate, the
   controller's full context and sampling state, remaining budget B, and (replay mode through the gate) the gate's
   history. The turn that produced the candidate is already spent in every branch.
2. **Restore** once per (treatment, repeat), in a shuffled order. Two ways: `snapshot` (load the exact state; the gate
   is started fresh, so `gate_history_restored=false`) or `replay` (fresh env from the seed + the logged action
   prefix, through the gate when the gate backend is used). Every restore is verified: `restored_ok` = state hash equal
   to the checkpoint's. Unfaithful restores are counted and excluded. *Equal screenshots are not equal state*: the
   hash covers labels, progress, dialogs, the loading timer, the clock and the RNG.
3. **Treatments** (same names as the gate's options where they exist):

   | treatment | what the branch does |
   |---|---|
   | `direct` | execute the candidate |
   | `look_light` | fresh tree read of the main window (no dialog, no image); the same controller re-chooses |
   | `look_full` | fresh read with the screenshot (dialogs visible); re-choose |
   | `check` | the gate's cheap probe of the candidate's element; pass → execute, fail → the look_light path |
   | `extra_reason` | one more think of the controller with NO new observation; may change the candidate; never authorises anything (safety review stays independent) |
   | `matched_wait` | wait exactly as long as look_light takes, no new information, then the original candidate — separates "time passed" from "information gained" |
   | `ask` | a scripted person names the right element; person seconds recorded |

4. **Continue** the task with the fixed policy π (a full read every 3 actions) inside the budget left after the
   treatment's own cost. Labels per branch: `executed` (driver accepted it), `step_success` (the step did what the task
   needed: progress or the blocking dialog closed — not merely "no error"), `task_success` (task done within B).
5. **Costs** per branch: virtual wall seconds of the treatment, tokens (tree 300 / image 1100 / think 400 in the dry
   run), person seconds.
6. **Repeats** re-seed the controller's sampling only; the environment's randomness is part of the checkpoint.

Pilot size suggested by the review: 100–200 checkpoints × 5 treatments × 3 repeats, then estimate variance before
scaling (a pilot, not a power guarantee).

## Task-family split

Δ depends on the controller, its prompt and the budget, not only on the GUI state, and states of one task are highly
correlated. Therefore: train / calibrate / test **by task family**. Every checkpoint, layout variant, branch and repeat
of a family lives on one side only (`dryrun.py` writes `split.json`; `analyze.py` bootstraps families, then checkpoints
within them). Fixture families are hazard profiles (`relabel`, `loading`, `popup`, `mixed`); for OSWorld a family is
the app domain (all checkpoints of all tasks of `libreoffice_calc` together), with unseen-layout and unseen-app tests
as separate held-out sets. A held-out *run* is not an unseen task.

## OSWorld (osworld.py, code only)

A native OSWorld setup/reset is not an arbitrary mid-task snapshot. A checkpoint at step n is rebuilt per branch by
reset + the task's own setup + replaying the logged tool calls 1..n−1 through a fresh gate (stale element tokens are
re-bound by a fresh read), then the state fingerprint (files under the task paths, window titles, focus, clipboard) is
compared with the one the logging run recorded at step n; without a recorded fingerprint the restore is "unverified"
and must not enter the analysis. App-internal state without a trace in files/windows (undo stack, scroll position,
timers) is not restored — that is why strict branching uses the fixture twin and OSWorld only validates whole tasks.

## From branches to the gate

`analyze.py --pred` compares predicted Δ (e.g. the `value.delta` the gate logged for that state) with the observed
paired Δ per equal-mass bin. A retrained treatment model (`gui-gate train --model treatment` on branch logs with the
options actually taken) becomes the gate's default only once its weights are loaded; until then the gate runs the
hand-set legacy prior (src/value.rs).
