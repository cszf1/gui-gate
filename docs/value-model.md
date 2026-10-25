# Value model (0.9.11, branch fix-predictor)

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
4. `reason` (one more think of the acting model, no new observation) is an option of the treatment model only.

## Costs

`gui-gate costs LOGS -o costs.json` measures per option: extra wall seconds before the step is attempted (gate-side
options: their gate time over the median direct gate time; hand-backs: refusal → the session's next action, i.e. the
model's extra turn), an image-token estimate of the observations in between (px/750; the gate does not see the model's
own token counts) and person seconds (`human.ms`). `GUI_GATE_COSTS=costs.json` uses them; cost =
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

## What is scaffolding until real counterfactual data exists

* No treatment weights ship; the gate runs the legacy prior by default.
* A treatment model trained on existing logs is observational: it can rank options only as well as the logged
  policy's choices allow, and options the old gate rarely took (reason, ask_help, check) have little or no coverage.
* The counterfactual harness has run only on the fixture twin with a scripted controller; its numbers say nothing
  about real agents. OSWorld checkpoints (setup + prefix replay + fingerprint) are code only.
* The `vp_*` comparison has not been run.
