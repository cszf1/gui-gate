# Value model (v0.10.1)

Response to the 2026-10-25 external review (predictor, research, P1, P2). What changed, what is active, and what is
still only scaffolding.

## Which model is active

| | `treatment` (new) | `legacy` (0.9.x prior) |
|---|---|---|
| P_o | q_o(x) = σ(b·x + g_o·x): one shared feature vector plus, per option, a main effect and option × feature interactions (logistic, L2) | P(direct) on hand-edited "after" features, clamped to ≥ P(direct), plus a hand-set residual gain |
| trained from | logged outcomes **with the option actually taken** (`gui-gate train --model treatment`) | nothing: the gains (check 0.03, wait 0.2, look_light 0.15, look_full 0.3, review 0.35, ask_help 0.9, pixel variants) are hand-set, **not estimated** |
| Δ_o = P_o − P_direct | any sign; no clamp | ≥ 0 by construction |
| uncertainty | bootstrap replicates (sessions resampled): interval of every Δ_o and of the net benefit (seconds) of the chosen option over the cheapest one | none |
| `check` | value of information (below) | forced probe before any hand-back (`grey`) |
| active when | trained weights are loaded: `GUI_GATE_VALUE_WEIGHTS=value.json` or a `treatment` block in the `--weights` file, and `GUI_GATE_VALUE_MODEL` is `auto` (default) or `treatment` | otherwise (**the default today: no treatment weights ship**), or `GUI_GATE_VALUE_MODEL=legacy` |

Every action line of the gate log has a `value` record: `model` (`legacy_prior` with `gains: "hand-set (not
estimated)"`, or the treatment model's name with `kind: treatment_v1`), `policy`, `p_direct`, `q_direct`,
`delta` per option as `[lo, mean, hi]`, `net` (the interval test), `uncovered` options, `costs` per option with
`src: measured|prior`, `voi`, `chosen`. `GUI_GATE_VALUE_MODEL=treatment` without weights logs an error on stderr and
runs the legacy prior.

## Choice rule

E[cost(o)] = λ·cost(o) + (1 − P_o)·C_fail over the options available in the state (unchanged hard rules and safety
rules run first; safety outcomes are never valued). With the treatment model:

1. options with fewer than `min_rows` (30) training rows are *uncovered* and never chosen;
2. `best` = argmin; if `best` is not the cheapest option and the lower bound of its net-benefit interval
   (level 0.9) is ≤ 0 on a low-risk step (no risk level, no hand-off), the cheapest option is taken (`tier=model_ci`).
   Model doubt is not turned into a person pop-up or an extra turn;
3. `check`: EV(check) = λ·cost(check) + P(pass)·min_o E[cost(o) | pass] + P(fail)·min_o E[cost(o) | fail] versus
   min_o E[cost(o)] now; probe when EV(check) is lower (`tier=voi`). P(pass) comes from a fitted probe model when the
   logs have probe results, otherwise from coherence with q_direct (q = P(pass)·q_pass + P(fail)·q_fail). A probe whose
   result cannot change the choice is pure cost and is never taken.
4. `reason` (one more think of the acting model, no new observation) is an option of the treatment model only
   (and of the `extra_reason` fixed policy); see *extra_reason* below.

## Costs

`gui-gate costs LOGS -o costs.json` measures per option: extra wall seconds before the step is attempted (gate-side
options: their gate time over the median direct gate time; hand-backs: refusal → the session's next action, i.e. the
model's extra turn), the model's real tokens of the turn(s) in between when the client
sends `_meta["gui_pi/usage"]` (0.10.1), else an image-token estimate of the observations in between (px/750) and person seconds (`human.ms`). `GUI_GATE_COSTS=costs.json` uses them; cost =
extra_s + tokens/1000·`GUI_GATE_COST_TOKEN_S` (default 0) + `GUI_GATE_COST_PERSON_W` (1)·person_s, and options with
fewer than `GUI_GATE_COST_MIN_N` (5) measurements fall back to the prior seconds (direct 0, check 0.2, heal 0.3,
wait 1, look_light 7, look_full 9, review 3, reason 7, ask 45). On the v0.9.9 OSWorld-Verified logs the measured mean
is look_light 9.1 s, look_full 24.2 s, review 17.0 s, wait 1.1 s, heal 0.2 s (look_full was priced at 9 s).

## Training-label hygiene (review §2)

* **Labels only on executed actions** → rows now carry the option actually taken; gate-side wait / check / heal /
  reviewer-allowed steps are their own options (0.9.x counted them as direct); handed-back steps are labelled with the
  next executed action of the session (≤ 180 s). Still impossible from logs: the outcome of `direct` in a state the gate
  blocked, and of any option not taken. q_o is therefore an *observational* estimate, confounded by the logged policy
  (which chose options from p_direct), until counterfactual branches exist (bench/counterfactual).
* **"No error" is not "helped the task"** → `--label task` labels rows with the run's task success
  (`--task-labels runs_scored.jsonl`); the step label stays "no driver error, no stale error, not immediately repeated".
* **Prior leakage** → the L2 centre is the hand-set v1 prior for the direct predictor and zero for the treatment
  model; the built-in osworld_v3 weights were fitted on the same logs, so `--prior builtin` is allowed only with
  `prior_leak_risk: true` in the report; `--report` also scores the built-in weights on the held-out set, labelled as
  optimistic.
* **Held-out by run AND task family** → `--split run+family` (default for `--model treatment`; `gui-gate train`
  keeps `session` as default for comparability): a test row's run and family are both absent from training; rows that
  cross are dropped and counted. With a single run the report says "run split impossible" and keeps the family split.
* **Metrics** → `--report`: Brier, NLL, ECE on equal-mass bins (equal-width ECE kept for comparison), AUC, each with a
  session-bootstrap 95 % CI, and an equal-mass reliability table with Wilson CIs per bin (`--md` writes it as
  markdown). Incremental calibration (predicted Δ vs paired observed Δ) cannot be computed from logs:
  `bench/counterfactual/analyze.py --pred`.

## Fair baselines (P2)

`GUI_GATE_VALUE_POLICY`: `learned` (default: the active value model), `direct`, `observe_light`, `observe_full`,
`extra_reason` (one `reason` hand-back per distinct step), `threshold` (`GUI_GATE_VALUE_THRESHOLD` on p_direct),
`heuristic` (rules only: loading/popup → wait, ≥2 failures → look, duplicate label / failed before → review or look,
≥3 actions since the read → look), `random` (`GUI_GATE_VALUE_RANDOM_RATE`, seeded by `GUI_GATE_VALUE_SEED`). They
replace only the value choice; the policy stays `gate`, so the safety rules, hard rules, auto-observe, heals, batching
and the reviewer are identical. Bench conds `vp_*` (bench/run_bench.sh), analysis `bench/fair_compare.py` (planned and
valid denominators, Wilson CIs, task-level paired bootstrap, McNemar, permutation, budget curve).

## 0.10.1 changes

### extra_reason = rethink, not review

`extra_reason` (option `reason`) is one more reasoning pass of the **acting** model with **no new observation**. The
step is refused (`status: "blocked"`, `gate.decision: "extra_reason"`) with `structuredContent.next_action: "rethink"`
and `reasoning_budget_tokens: 1024`, a fixed hint the client may map to its model's thinking budget. The model then
sends the step it decides on: the same one or a different one. The re-sent step is checked like any other step
(safety rules, risk path, final dispatch check); the rethink authorizes nothing, and the same step is not sent back for
a second rethink. `review` is **not** extra reasoning: it is the independent reviewer allowing or refusing the
original action on a doubtful target. It never regenerates the candidate, so its value is not the value of "thinking
more" and the two are separate options in the logs and in the fair baselines.

### gate_next_step: the pre-step decision

The gate acts after the model has produced an action, so it cannot save the reasoning that already happened. To
schedule the *next* observation or extra thought, the desktop client calls the client-only tool `gate_next_step`
(optional `window_id`) **before** its next model turn. The gate decides with the same policy and value model as for
actions, on the window's state as it knows it (no candidate exists yet, so the features are those of a generic element
step in that window; no observation of the window → look). The answer is `observe: none|tree|tree+screenshot` and
`extra_reason: bool` (plus `option`, `p_direct`, `reason`, `decision_id`, `advisory: true`), logged as `next_step`.
It is advisory and grants nothing: every action still passes `gated()` and the final dispatch check.

### IPW with effective sample size

`gui-gate train --model treatment --ipw [--ipw-clip 0.05]` fits a propensity model of the **logged** policy
(multinomial logistic P(option | x) over the options seen in training) and weights each row by 1 / max(p̂(o|x), clip),
normalised to mean 1. The report gives the clip, the number of clipped rows, and the effective sample size
ESS = (Σw)² / Σw² overall and per option (with each option's mean propensity). A small per-option ESS means the logged
policy almost never took that option in states like these: IPW does not create coverage, and unlogged confounders
remain. IPW is a correction for the observed part of the logged policy's selection, not a substitute for paired
counterfactual branches.

### Model variants compared in `train --report`

With `--report`, the same held-out run+family split scores four variants: `interaction` (shared features + option
main effects + option × feature interactions, the shipped `treatment_v1` form), `interaction_ipw` (same, IPW-weighted,
when `--ipw`), `separate_per_option` (one logistic model per option, no shared base) and `boosted_stumps`
(gradient-boosted decision stumps with logistic loss on features, option one-hots and option × feature products).
Each gets Brier, NLL and equal-mass ECE with cluster-bootstrap CIs (`--md` writes the table). All variants are
observational; metrics are unweighted.

### Legacy is still the default

No treatment weights ship in 0.10.1. Without `GUI_GATE_VALUE_WEIGHTS` (or a `treatment` block in `--weights`) the
gate runs the legacy prior with hand-set gains, exactly as in 0.10.0.

### Real token usage

When the client sends the model's real token usage with each tool call in `_meta["gui_pi/usage"] = {input, output,
cache_read, same_message?}` (see docs/mcp-2026-07-28.md), the gate logs a `usage` event per call, sums it in
`gate_report.model_usage`, and `gui-gate costs` prices a hand-back with the real input + output tokens of the turn(s)
between the refusal and the next action (`tokens_src: "client_usage"`). Without it the cost falls back to the image
token estimate px/750 (`tokens_src: "pixel_estimate"`).

### Person answers in training

Safety-required asks are never valued (they are not an efficiency option). A person request that ended without an
answer (timeout, cancel, declined form, closed window, no channel) is logged as `no_answer` and is never a training
label: it is not evidence that the person was unwilling to help.

## What is scaffolding until real counterfactual data exists

* No treatment weights ship (0.10.1 included); the gate runs the legacy prior by default.
* A treatment model trained on existing logs is observational: it can rank options only as well as the logged
  policy's choices allow, and options the old gate rarely took (reason, ask_help, check) have little or no coverage.
* The counterfactual harness has run only on the fixture twin with a scripted controller; its numbers say nothing
  about real agents. OSWorld checkpoints (setup + prefix replay + fingerprint) are code only.
* The `vp_*` comparison has not been run.
