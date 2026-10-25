# Evaluation protocol (gui-gate bench)

Status: protocol + offline tooling. Written 2026-10-25 in response to the external review (sections 1 and 6, P1/P2).
No result in `results/` was produced under this protocol yet; every earlier number is descriptive (see the last
section). Code: `bench/manifest.py`, `bench/schedule.py`, `bench/cost_accounting.py`, `bench/stats.py`,
`bench/fair_compare.py`, `bench/leakage_lint.py`, `bench/human_pilot.py`, `bench/context_ablation.py`,
`bench/counterfactual/`. Tests: `tests/test_bench_*.py` (offline, no model, no desktop).

## 1. Two different experiments

| | research experiment | product experiment |
|---|---|---|
| question | in safety-allowed states, is an extra observation or extra reasoning worth its cost, and does the value model choose it better than simple rules? | is the new desktop agent (Tauri + Pi + gui-gate + driver) better for a user than the old system? |
| what varies | **only the intervention policy** (`GUI_GATE_VALUE_POLICY`, conds `vp_*`) | the whole system |
| what is fixed | the same acting model, prompt, tools, gate binary and engineering base: safety rules, hard rules, auto-observe, heals, reviewer, turn cap, timeout, budget | the task list, budget, environment and scoring |
| allowed claim | "policy X beats policy Y at equal budget" / "X is non-inferior to Y within margin m and cheaper" | "the new system is better/worse than the old one overall" |
| not allowed | attributing a product gain to the value model | attributing a research gain to the product |

The old `direct` cond changes safety checks, auto-observe, screenshot defaults and some heals at once; it is **not** a
research baseline. `vp_direct` is: gate8 with the value module never requesting an extra intervention.

## 2. Dev set, frozen test set, no tuning on the test set

1. Write the manifest before any test run: `python3 bench/manifest.py create --list LIST --out M.json --margin 0.05`.
   It holds the task list, the dev/test split **by task family** (OSWorld app domain; every task, layout variant,
   branch and repeat of a family on one side), exclusion reasons, retry rules, the pre-registered non-inferiority
   margin, and a sha256 over all of that plus every task JSON's content.
2. Quote the hash in the pre-registration (commit message or this file's appendix). `manifest.py validate` must pass.
3. Tune only on the dev split. Rules, prompts, memory, skills, app hints, calibrators, thresholds, value weights and
   costs are frozen before the test run; looking at test results and changing any of them makes every later test
   number exploratory.
4. `run_bench.sh` (and `counterfactual/osworld.py` per task) refuse a list that touches the test split unless
   `--manifest-hash` equals the recomputed hash, the list is exactly the runnable test split, and it does not mix in
   dev tasks (exit 3 before anything runs). The leakage lint (§8) runs too.
5. The 34 tasks of `bench/tasks/order.txt` were used throughout development (0.2 - 0.10): the committed default
   `bench/tasks/manifest.json` marks them all **dev**. A test split has to be frozen from tasks nobody has looked at
   (e.g. a family-held-out part of OSWorld-Verified), plus the unseen fixture layouts (§8).

Denominators (`manifest.py denominators`, `fair_compare.py --manifest`): **planned** = runnable test tasks x repeats
per policy, with missing runs and every infra failure counted as failures; **valid** = runs without an infra failure
(descriptive only). Infra failures are counted per policy and per kind (`infra_by_kind`). A policy that makes more API
calls must not lose its API errors from the denominator: they are part of its reliability.

Retry rules (manifest `retry_rules`): only `relay_error` / `setup_failed` are retried, at most 3 attempts, the last
attempt counts. Agent outcomes (wrong result, FAIL reply, max_turns, timeout) are never retried.

## 3. Ablations

Main chain (each step adds one thing; all on the same fixed safety + engineering base):

1. fixed safety + engineering base, no value module (`vp_direct`)
2. heuristics only (`vp_heuristic`)
3. P(direct) + fixed gains (`vp_legacy`; and `vp_threshold` as the plain failure-probability rule)
4. learned per-option gains (`vp_learned` with treatment weights trained on branch data)
5. + incremental calibration (the same, with the calibrator fitted on dev branches)

Reference policies at matched budget: every-step observation (`vp_light`, screenshot version `vp_full`), fixed extra
reasoning (`vp_reason`), matched-time wait, and random at the learned policy's intervention rate (`vp_random`,
`VP_RANDOM_RATE`). No Cartesian product of switches.

Grouped ablations, each a separate run against the full base: auto-observe off; heals off; context compression
(`ctx_full` / `ctx_current` / `ctx_new`, §9); the 7-role pack off.

## 4. Labels (three levels, never collapsed)

| level | label | meaning |
|---|---|---|
| 1 | driver error | did the driver accept the action (no error)? cheap, says nothing about usefulness |
| 2 | local subgoal | did the step achieve its **predefined** local subgoal (progress, the blocking dialog closed, the field holds the value)? defined before the run, per task family |
| 3 | task success within budget | the evaluator's verdict with the intervention's own cost charged to the remaining budget |

"No error" must never stand in for "helped the task". The counterfactual harness records `executed`,
`step_success` and `task_success` per branch.

## 5. Counterfactual pilot

Definition: Δ_o(x) = P(Y_o = 1 | x, π, B) − P(Y_direct = 1 | x, π, B): x = what was visible before the
intervention, π = the fixed continuation policy, B = the remaining budget including the intervention's cost. Δ
depends on the acting model, prompt and budget; it is not a property of the GUI state.

Treatments: `direct`, `look_light`, `look_full`, `extra_reason` (re-generates the candidate with no new observation;
never authorises anything; the independent review still runs), `matched_wait` (same wall time, no information), plus
`check` and `ask` in the harness.

Pilot size: **100–200 checkpoints × 5 treatments × 3 repeats**. This is a pilot to estimate variance and catch
protocol bugs; it makes **no power claim**. Decide on scaling only after the pilot's variance is known.

Checkpoint and restore: freeze the environment, the candidate, the full model context and sampling state, the gate's
cache and history, and the remaining budget, right after the candidate is generated. A restore must reproduce
**files, focus, clipboard, windows, app state and timers** (loading timers, auto-save, dialogs that appear later).
**The same screenshot is not the same state**: two identical screenshots can differ in the undo stack, a pending
timer, the clipboard, a hidden dialog or the RNG. Every restore is verified against a state fingerprint/hash;
unverified restores are excluded and counted. Strict branching uses the serialisable fixture twin
(`bench/counterfactual/fixture_env.py`); OSWorld (setup + replay) validates whole tasks only, because a native
setup/reset is not an arbitrary mid-task snapshot.

Analysis: paired Δ per checkpoint with task-family bootstrap CIs; negative / zero / positive counts (negative
increments are expected and kept); look_light − matched_wait (information beyond waiting); the incremental
reliability diagram (§10); Brier/NLL of per-option probabilities when the model gives them (AUC is ranking only).

## 6. Running: interleaving and repeats

- `run_bench.sh --interleave abba`: all conds of a task back to back, their order reversed on every other task and
  repeat (Williams rows for 3+ conds), so drift of the API, relay and box hits every cond equally often first and
  second. The schedule is written to `OUT/schedule.txt`; each row records `order`.
- `--repeats N`: at least **3 independent repeats** to start. They are named "independent repeats"
  (`repeat_kind: independent`): the model API used so far takes no sampling seed, so the repeat label is not a
  controlled seed. `api_seed_accepted` is true only when the operator verified the provider honours a seed
  (`PI_API_SEED_ACCEPTED=1`). `GUI_GATE_VALUE_SEED` seeds only the gate's own random policy.
- Report task-level paired CIs (bootstrap over tasks), sign-flip permutation p, McNemar over (task, repeat).

## 7. Costs and time

Per run (`bench/cost_accounting.py`, merged into `score.py`): Pi turns vs real GUI actions executed by the gate (100
Pi turns ≠ 100 environment actions), blocked actions, run_command executions, observations, screenshots seen by the
model and captured by the gate, reviewer calls, role calls by role, acting-model tokens and cost, role tokens when the
gate logs them (reported as "not logged" otherwise, never as 0), Pi wall time and whole-run wall time incl. setup and
evaluation. Time comparisons use **all** planned runs (failures, timeouts and infra; missing runs at the cap);
success-only times are descriptive and survivor-biased.

## 8. Held-out layouts and leakage

- Unseen-layout test set: `bench/counterfactual/layouts.py write OUT.json` freezes 12 fixture layout variants (new
  caption vocabularies sharing nothing with the dev layout, other button orders, other hazard profiles) marked
  `split: unseen_layout_test`, "final evaluation only". `dryrun.py --unseen-layouts N` evaluates them as their own
  families, never in the fitting split.
- `bench/leakage_lint.py`: (1) no evaluator answer file of a test task (paths in `evaluator.expected`, OSWorld's
  gold-file cache, `bench/tasks`) is inside or symlinked from the agent's run directories; (2) no tuned artefact
  (weights, memory, skills, app hints, calibrators, cost files) contains a test-task id, a gold literal or an unseen
  layout caption. `run_bench.sh` runs it for test-split lists and refuses on a finding. An unrestricted same-user shell
  can still read any user file; the lint shows nothing is put in the agent's way, it is not isolation.

## 9. Context ablation

Conds `ctx_full` (no context extension), `ctx_current` (`bench/pi_ext/obs_prune.ts`), `ctx_new`
(`PI_CONTEXT_NEW_EXT`), all gate8 exactly, interleaved. `bench/context_ablation.py` reports success, stale-token
errors, repeated actions, lost task facts (with a facts file), compactions, tokens and time, paired by task against
`ctx_full`. First verify that diff reconstruction equals the full tree; a shorter context alone is not an improvement.
Expected log fields are listed in the script's docstring.

## 10. Increment reliability

`bench/counterfactual/analyze.py --pred PRED.jsonl --plot OUT.png`: equal-mass bins of predicted Δ (ties never split
across bins), mean predicted vs mean paired observed Δ per bin, task-family bootstrap 95% CIs, one panel per
treatment, diagonal = perfect. PNG via matplotlib; without it `OUT.csv` + `OUT.txt` (ASCII).

## 11. Human pilot

`bench/human_pilot.py assign` makes a counterbalanced plan (each person: half the tasks under threshold asking, half
under value asking; AB/BA alternating; task-to-condition mapping alternating). The analysis reports asks per task,
repeated-question ratio, active vs waiting person-seconds, takeover steps, cancel/timeout rate, wrong-approval rate,
success, total time over all sessions and an interruption rating, with within-person paired CIs and an order-balance
check. Timeouts deny the one action and keep their reason; they are not "the person refused". A simulated person and
`--human deny` test the protocol, not the burden on people.

## 12. Claims and wording

- Strongest acceptable claims: "at a fixed budget, success is higher" or "success is lower by no more than the
  pre-registered margin m (one-sided lower bound > −m) while time is clearly lower".
  `stats.noninferiority` / `fair_compare.py --manifest` (margin from the manifest; a margin chosen after the data is
  marked exploratory).
- **"Not significant" is not "equal"** and not "no drop". Never write "same as", "no loss" or "equivalent" from a
  non-significant difference.
- Results from `bench/` are from a **custom environment based on OSWorld tasks and evaluators** (native Debian 13 /
  XFCE, other app versions, own reset, own budgets). They are not comparable to the OSWorld leaderboard.
- Wilson intervals on valid runs ignore task correlation and valid-run selection; report the planned denominator next
  to them.
