# Value model v2 (opt-in) and the offline evaluation on gui-pi3 OSWorld-Verified

> **v0.10.3 (branch v3-res, review 2026-10-26 sections 1, 2, 6):** sections 9-12 below define the two objectives
> (Q^π and the local net gain G_o), the decision data contract (immutable `x_pre`), the three label layers and the three
> evidence chains, and replace parts of sections 3, 5 and 7. The default decision is still exactly acaba69's
> (`value::tests::default_path_output_is_unchanged`). Results of the v0.9.9 run, as everywhere in this repository:
> **185/361 = 51.25 %** of the planned list (infra and the missing task count as failures), **185/332 = 55.72 %** of
> valid runs, **28 infra** (23 setup_failed, 4 relay_error, 1 eval_error), **1 task never ran**; a **custom environment**
> based on OSWorld tasks and evaluators, not an official OSWorld-Verified score and not leaderboard-comparable. The
> older 137/255 = 53.7 % was a partial snapshot of the same run and is superseded.

Status 2026-10-26, branch `v2-val`. Response to the external review of the treatment / IPW value model and to the
predictor check of 2026-10-25 (built-in osworld_v3 AUC 0.626 / ECE 0.197 with commands mixed in; replay intervening on
32.3 % of steps with 7.4 % precision). Everything below that changes a decision is **off by default**; the default
gate makes exactly the decisions of c3893bc (test `value::tests::default_path_output_is_unchanged`, a golden file of
189 decisions + value logs generated on c3893bc before any change).

Numbers: `results/offline-v2-2026-10-26/` (report.md, report.json, exclusions.json), produced by
`python3 bench/offline_v2.py results/osworld-verified-v099-ds41-2026-10-23 -o results/offline-v2-2026-10-26 --bin target/release/gui-gate`
(seed 0, 200 session-bootstrap replicates, stdlib only). The input run directory is read only.

## 1. Switches and defaults

| switch | values | default | what it changes |
|---|---|---|---|
| `GUI_GATE_VALUE_MODEL` | `legacy`, `treatment`, `auto`, `v2` | **`legacy`** | `legacy`: the hand-set 0.9.x/0.10 prior, as before. `treatment` / `auto`: the treatment model when its weights are loaded (`auto` was the 0.10.1 default; it is now opt-in). `v2`: section 2. An unknown value logs an error and stays `legacy`. |
| `GUI_GATE_VALUE_WEIGHTS` | path | unset | loads a treatment model (`treatment_v1` or `treatment_v2`). **Loading weights no longer switches the model** (0.10.1 did when the request was `auto`); `legacy` ignores them. |
| model file `boot` | bootstrap replicates | - | fewer than `MIN_BOOT` = 20: intervals unknown, model not used (section 3.1) |

Tests: `value::tests::treatment_weights_alone_do_not_switch_the_default`, `default_path_output_is_unchanged`,
`legacy_is_unchanged_by_default`, `features::tests::v2_gui_history_skips_commands_and_the_default_keeps_its_features`
(no v2 feature key is computed or logged on the default path).

## 2. What `GUI_GATE_VALUE_MODEL=v2` does

1. **Commands are not GUI steps.** `run_command` never reaches the GUI predictor: `value::choose` returns `direct` with
   `tier: "command"`, `p_direct: null` and a `model: "command"` value log. (The live gate already executed commands
   directly, server.rs "command (no GUI target)", so no live decision changes; what changes is that commands stop
   carrying a GUI probability that every report then averaged in.) The safety path (risk rules, reviewer, person) and
   the loop breaker run before/after exactly as before.
2. **GUI history without commands.** `GateState::gui_history`: `prev_none`, `sess_fail`, `consec_fail`, `sig_failed`
   and `last_err` / `last_unverif` / `last_popup` are computed over GUI actions only. A `grep` without a match
   (exit 1) is not a failed click.
3. **v2 features** (`features::V2_FEATURES`, decision-time, present in every log): `tool_click`, `tool_scroll`,
   `tool_type`, `tool_key`, `tool_set_value`, `scroll_pixel`, `role_button`, `role_text`, `role_link_tab`,
   `role_list_item`, `role_combo`. Not in the logs and therefore not features: whether the target is in the viewport,
   occluded or enabled (proposed 2026-10-25; needs new logging first).
4. **The treatment model is used only with intervals.** With `treatment_v2` weights (`gui-gate train --model treatment
   --features v2`) and >= 20 bootstrap replicates the treatment choice of 0.10.1 runs on the v2 features; otherwise
   the legacy prior decides (on the GUI-only history).

## 3. Mathematics

### 3.1 Treatment-interaction model and its uncertainty

q_o(x) = σ(b·x + g_o·x), g_direct = 0, Δ_o(x) = q_o(x) − q_direct(x); expected cost
E[cost(o)] = λ·cost(o) + (1 − q_o(x))·C_fail(x). Net benefit of the argmin option a over the cheapest option c:
NB = (q_a − q_c)·C_fail − λ·(cost_a − cost_c), with a percentile interval over B session-bootstrap refits.

**Fix:** with B = 0 the 0.10.1 code returned `Interval::point(mean)`, i.e. lo = hi = mean. The "take the cheaper option
when the lower bound of NB is ≤ 0" rule then saw a *certain* benefit whenever the point estimate was positive, so the
less data the model had, the more confident it looked. Now:

* B < MIN_BOOT (20) → `Interval::unknown`: lo = −∞, hi = +∞ (`[null, mean, null]` in the log), `crosses_zero()` true;
* such a model is **not used**: `value::choose` runs the legacy prior with the legacy check rule (exactly the default
  decision) and logs `fallback: "treatment model … has N bootstrap replicates (< 20): interval unknown, legacy prior
  used"`; `resolve_value_model` warns at start-up.

Tests: `no_bootstrap_means_an_unknown_interval_not_a_zero_width_one` (also 3 replicates → unknown),
`a_treatment_model_without_intervals_is_not_used_the_legacy_choice_runs` (same option and scores as legacy).

### 3.2 Leakage review (features that are only known after the outcome / after the decision)

| candidate | finding | fix |
|---|---|---|
| `shadow_check` (collect mode probes every step for labelling) | the trainers copied it into `check_pass` / `check_fail` when the live features had none: the live decision never saw that probe → train/serve skew, information from outside the decision | not copied any more (valuetrain `feats_vec`, tools `load`); it stays the probe model's label. Tests `a_shadow_probe_is_not_a_decision_time_feature`, `the_direct_predictor_trains_on_gui_steps_only_and_without_shadow_probes` |
| `check_pass` / `check_fail` in the **propensity** model | exist only after `check` ran: P(check \| x) became ~1 on every check row, hiding the real overlap | `POST_DECISION_FEATURES` excluded from the propensity fit. Test `propensity_does_not_see_the_result_of_the_option_it_explains` (P(check) = base rate when x carries no pre-decision signal) |
| history features | computed from earlier actions only, identically in gate and trainer | none needed |
| `--label task` | the run's success is the label, never a feature | none needed |
| `occlusion_heal`, `focus_heal`, `effect`, `popup` of the same step | logged after execution | never features (unchanged) |

In this run no `shadow_check` was logged (collect mode off) and 59 logged actions carry a real probe result, so the leak fixes do
not move the numbers below; they matter for collect-mode logs.

### 3.3 Propensities, clipping and overlap

IPW weight w = 1 / max(p̂(o|x), clip), clip = 0.05, normalised to mean 1. Clipping bounds the weights (max 20 before
normalisation) but **creates no overlap**: where the logged policy (almost) never took o, no weight can recover the
outcome of o. New per-option diagnostics: `support` = share of all training states with p̂(o|x) ≥ clip, the number of
clipped rows, and ESS = (Σw)²/Σw². Coverage under IPW (`ipw_effective_counts`): an option counts min(n, ⌊ESS⌋) rows
and 0 when support < `--min-support` (default 0.2); `TreatmentModel::covered` then applies `min_rows` (30). Uncovered
options are never chosen. Test `ipw_coverage_follows_effective_sample_size_and_overlap`.

On this run (v2 features, family split, 4,257 training rows):

| option | rows | ESS | mean p̂ | support | clipped | counted (covered?) |
|---|---|---|---|---|---|---|
| direct | 3,931 | 3,894 | 0.92 | 1.00 | 0 | 3,931 (yes) |
| wait | 184 | 110 | 0.21 | 0.29 | 29 | 110 (yes) |
| look_full | 55 | 43 | 0.13 | 0.04 | 15 | 0 (no) |
| review | 31 | 30 | 0.04 | 0.01 | 24 | 0 (no) |
| heal | 26 | 24 | 0.08 | 0.03 | 5 | 0 (no) |
| look_light | 18 | 18 | 0.02 | 0.00 | 18 | 0 (no) |
| check | 12 | 12 | 0.04 | 0.00 | 11 | 0 (no) |

Overall ESS 1,007 of 4,257. **Only `direct` and `wait` have enough overlap**; for every look / review / probe option
the logged policy was close to deterministic, so an observational treatment model cannot say what those options are
worth. That is a property of the logs, not something a better estimator fixes.

### 3.4 Commands vs GUI

The GUI predictor's `no_obs` weight (−2.41 in osworld_v3) punishes "no window observation", which is always true for
a command. Commands are now: out of the GUI rows by default in both trainers (`--commands exclude|include|only`;
`include` = 0.10.1), counted apart in `gui-gate replay` (`commands: {n, failed}`), out of the GUI history under v2,
and calibrated separately in the offline report. Test `commands_and_later_sessions_are_not_gui_training_rows`,
`replay_counts_commands_apart_from_gui_interventions`.

## 4. Data and exclusions (口径)

Source run `results/osworld-verified-v099-ds41-2026-10-23/`, current repository state (final commit 9450527 + c3893bc
leftovers). `report.json`: expected 361, rows 360, valid 332, infra 28 (setup_failed 23, relay_error 4, eval_error 1),
success 185 → **185/361 = 51.25 %**, **185/332 = 55.72 %**. The offline audit reproduces exactly these numbers.

Exclusions (all listed in `exclusions.json`):

* **corrupt line**: the first-pass `runs.jsonl` had one truncated row, `multi_apps-48d05431-…-gate8-s0` (380 bytes,
  ends in `"pi":}`). The repository's final `runs.jsonl` was rebuilt for the retry and no longer contains it; the
  truncated line survives in `retry/runs.before_retry.jsonl:240` and `retry/runs.retry_partial.jsonl:199` (and was line
  240 of `runs.jsonl` at 06f854b). `runs_scored.jsonl` never had that task. It is the "1 task without a result row":
  excluded (no label), although its gate and Pi logs exist.
* **infra** (28 tasks): no task label; excluded from all analyses.
* **repeated attempts**: 25 earlier attempt rows (relay errors) of 22 tasks that later got a valid attempt. Policy
  `first_valid` (default) uses the first non-infra attempt, which is what the report's 332 counts. The final run files
  keep only the last attempt per task; the earlier infra attempts are read from `retry/*.jsonl`. Sensitivity policy
  `--attempt first` (attempt 1 only) drops those 22 tasks: 173/310 = 55.8 % valid.
* **later sessions in one gate log**: `gate/multi_apps-98e8e339-…-gate8-s0.jsonl` holds attempt 1 (60 actions) plus
  an aborted re-run of the stopped infra retry (41 actions, 2026-10-26 13:49 CST). That file is attempt 1 (relay
  error), so it is not used at all; in general only the first session of a file is used (`--all-sessions` in the
  trainer restores the old behaviour).
* step rows: executed actions of the 332 used logs = 7,239 GUI + 2,480 commands (= the report's 9,719 executed);
  risky steps (20) and safety outcomes are excluded from calibration; 181 GUI refusals.

**Why 06f854b said 184/361 and 331 valid:** that snapshot was taken while `multi_apps-c867c42d-…` was still running
(its gate log runs from 24 min before to 12 min after the 06f854b commit; 9450527 adds its gate/Pi logs). Its row (attempt 1, valid, success) is the
only task added between the snapshots: +1 success, +1 valid row (359 → 360 rows). The 06f854b `runs_scored.jsonl` also
still held 33 superseded relay-error attempt rows (392 lines vs 360) that its report already de-duplicated; the
truncated `48d05431` line was present then and is excluded in both.

## 5. Results

Label of a step: executed without driver error / stale-token error and not immediately redone (navigation-key repeats
are not failures). Base success rate 93.3 % for both GUI steps and commands.

### 5.1 Calibration, separately for GUI and commands

| set | model | n | AUC [95 % CI] | ECE equal-mass [95 % CI] | Brier |
|---|---|---|---|---|---|
| GUI | built-in osworld_v3 (logged live) | 7,225 | 0.694 [0.669, 0.719] | 0.043 [0.037, 0.051] | 0.072 |
| command | built-in osworld_v3 (logged live) | 2,443 | 0.543 [0.501, 0.584] | **0.674** [0.665, 0.682] | 0.519 |
| all (as 2026-10-25) | built-in osworld_v3 | 9,668 | 0.617 [0.594, 0.640] | 0.202 [0.182, 0.222] | 0.185 |
| GUI | v1 features, refit, leave-one-domain-out | 7,225 | 0.773 [0.740, 0.798] | 0.024 [0.019, 0.032] | 0.050 |
| GUI | v2 features, leave-one-domain-out | 7,225 | 0.770 [0.723, 0.803] | 0.028 [0.022, 0.034] | 0.050 |
| command | command model (13 command-text features) | 2,443 | 0.634 [0.590, 0.687] | 0.014 [0.012, 0.029] | 0.062 |
| command | constant base rate | 2,443 | 0.500 | 0.019 | 0.063 |

* The 2026-10-25 headline (AUC 0.626 / ECE 0.197) was mostly the command rows: on GUI steps alone the built-in
  predictor has ECE 0.043, on commands 0.674 (it predicts ~26 % success for commands that succeed 93 % of the time;
  100 % of command rows have `no_obs = 1`). The average hid both: a decent GUI calibration and a broken command one.
* Refitting on this run (cross-fitted, each domain scored by a model that never saw it) raises GUI AUC from 0.694 to
  0.773. The built-in weights were fitted on older runs (to 2026-10-22), so this run is out-of-sample for them.
* **v2 features add nothing measurable:** AUC v2 − v1 = −0.003, paired session-bootstrap 95 % CI [−0.018, +0.012].
  By tool (v1 / v2 refit AUC): click 0.795 / 0.803 (n 2,682, failure 11.1 %), press_key 0.634 / 0.550, type_text
  0.788 / 0.578 (6 failures in 1,023), scroll 0.592 / 0.588 (n 122, failure 28.7 %). The built-in model is weakest on
  clicks (0.552). Scroll stays unpredictable from what is logged; 23 of its 35 "failures" are immediate repeats of the
  same scroll (repeat = failure in the label), which is probably the label being wrong, not the scroll.
* A command model can only weakly rank commands (0.634) and the label is weak (exit ≠ 0 is often intended).

### 5.2 Predicting execution failures (what the logs can say)

**Post-hoc top-k screen** (v0.10.3 wording): flag the share b of executed GUI steps with the lowest predicted success
(out-of-fold for the refits) and count the failures among them afterwards. This cannot be read as "intervening on
b % of steps avoids x % of failures": whether the intervention would have helped is not observed.

| model | b = 5 % precision / recall | 10 % | 20 % | 32.3 % |
|---|---|---|---|---|
| built-in | 0.114 / 0.085 | 0.119 / 0.178 | 0.138 / 0.412 | 0.122 / 0.588 |
| v1 refit | 0.560 / 0.418 | 0.344 / 0.514 | 0.192 / 0.576 | 0.148 / 0.716 |
| v2 | 0.576 / 0.431 | 0.349 / 0.522 | 0.199 / 0.594 | 0.140 / 0.675 |

(base failure rate 6.7 %, 485 failed GUI steps). `gui-gate replay` (current rules, legacy prior, built-in weights) on
the same 332 logs, commands counted apart: intervene 7.9 %, gate-side 6.8 %, failures flagged 11.3 %, precision 9.6 %;
with `--reviewer` 8.7 % / 7.6 % / 12.8 % / 9.9 %. The 2026-10-25 replay (32.3 % intervene, 7.4 % precision) counted the
2,213 commands as GUI steps: the predictor sent nearly every command to `look_light` there, although the live gate
executes commands directly.

### 5.3 Treatment model v2 (gui-gate train --model treatment --features v2, family split, held out: impress, multi_apps, os)

| fit | held-out rows | AUC [95 % CI] | ECE | NLL | covered options |
|---|---|---|---|---|---|
| unweighted | 3,122 | 0.735 | 0.027 | 0.251 | direct, wait, look_full, review (n ≥ 30) |
| IPW (clip 0.05) | 3,122 | 0.642 [0.594, 0.690] | 0.012 | 0.269 | direct, wait (ESS + support) |

IPW reweights training toward rarely-chosen states and costs discrimination on the (unweighted) held-out rows; its
value here is the coverage diagnosis in 3.3, not a better model.

**Withdrawn (v0.10.3): the "AUC 0.795 with much weaker L2" sentence of 0.10.2.** That L2 (`--l2 0.0002 --l2-int 0.002
--l2-main 0.0002 --iters 20000`) was tried on the held-out split itself and the best value was quoted: a post-hoc
number, not a result. It is not used anywhere. Regularisation, calibration, thresholds and budget prices are now
chosen on development task groups only (bench/offline_v2.py `select_l2_on_dev`, section 12).

### 5.4 Costs re-estimated from the logs

Hand-backs: refusal → the session's next action (gate timestamps) and the real tokens of the Pi assistant turns that
ended in between (`turn_end.usage`). Gate-side options: their gate_ms over the median direct gate_ms (2.6 ms).

| option | n | extra s mean (median) | prior s | input / output tokens | cache read | model turns |
|---|---|---|---|---|---|---|
| look_light | 25 | 8.0 (5.3) | 7 | 2,086 / 299 | 49,142 | 1.36 |
| look_full | 96 | 26.4 (9.8) | 9 | 2,575 / 388 | 57,612 | 1.57 |
| review refused | 44 | 22.2 (7.1) | - | 1,984 / 506 | 57,257 | 1.32 |
| review allowed (reviewer call) | 19 | 0.94 | - | 0 | - | - |
| review, both | 63 | 15.8 | 3 | 1,386 / 354 | - | - |
| wait | 294 | 1.17 | 1 | 0 | - | - |
| check | 21 | 0.28 | 0.2 | 0 | - | - |
| heal | 38 | 0.23 | 0.3 | 0 | - | - |
| ask_human (deny mode) | 8 | 44.3 (32.0) | 45 | 1,595 / 886 | 82,896 | 1.88 |

A model turn of this run: median 4.9 s, mean 13.1 s (10,299 turns), median 1,079 uncached input / 188 output tokens
plus 31,872 cache-read tokens. The prior seconds are about right for look_light, wait, check, heal and ask; **look_full
(26 s mean vs 9) and review (16 s vs 3) are priced far too cheaply**. Means are heavy-tailed (look_full median 9.8 s).
`report.json → costs.costs_v1_for_gate` is in the `GUI_GATE_COSTS` format (hand-back tokens = input + output).

## 6. What the logs cannot say

* **"The intervention avoided a failure" is not estimable and is not claimed.** A refused step was not executed, so its
  outcome under `direct` is unobserved; the agent's next step is a different action chosen after the refusal.
  Descriptive only: after 96 look_full refusals the next GUI step succeeded 79 of 91 times, after 63 look_light
  refusals 51 of 63; this is neither the counterfactual outcome nor a success rate of the refusal.
* What is measured is **prediction of execution failures** (5.2): a step that ran and failed was flagged beforehand.
* Treatment effects of look / review / check / heal options: no overlap (3.3). Paired counterfactual branches
  (`bench/counterfactual/`) or a randomized online comparison are needed.
* Task success is not modelled here (`--label task` exists; 332 tasks, one run).

## 7. Online experiment plan (not run)

v0.10.3 replaces this by the two pilots of section 12.3 (`bench/online_plan.py pilots`); the generic plan below is
kept, but it is only called "frozen" when every item of `FREEZE_KEYS` has a hash, it no longer excludes infra cells from
both arms, and it uses McNemar only with one run per task and arm.

`bench/online_plan.py TASK_LIST -o plan.json` freezes: a seeded, domain-stratified held-out task set (fitting and
tuning only on the development set), arms that differ only in `GUI_GATE_VALUE_MODEL` (legacy vs v2; both load the same
weights file, which legacy ignores), a seeded run order blocked by task (both arms of a task back to back, arm order
randomised), the paired analysis (bench/stats.py McNemar, paired bootstrap, non-inferiority with a pre-registered
margin), and the infra rule (same retries in both arms; a cell still infra is excluded from both arms of that task and
reported). The plan carries its sha256. It never starts a box, gate, model or benchmark. Example (not executed):
`results/offline-v2-2026-10-26/online_plan.json` (seed 20261026, 108 held-out / 253 development tasks, 216 cells); its
`gate_commit` and weights must be filled in at freeze time, with v2 weights trained on the development tasks only.

## 8. Open points

* v2 features do not improve AUC on this run; whether v2 helps tasks is unknown until the online comparison.
* Scroll repeats are labelled failures; a scroll-specific repeat rule (like the navigation-key rule) needs a decision.
* Viewport / occlusion / enabled-state features need new logging before they can be learned.
* One run, one model (deepseek-v4.1-flash), one box: domain-held-out CIs do not cover a different agent or desktop.
* The default L2 of the treatment trainer looks too strong for runs of this size (5.3); not changed here.


## 9. Two objectives (v0.10.3)

**Product objective.** h = history, candidate action and observations; b = remaining budget; π = the fixed downstream
agent policy. For an option o (direct, check, heal, wait, look_light, look_full, review, reason, ask_help):

    Q^π(h, b, o) = E[ L·(1 − Y_o^π) + λ·T_o^π + γ·H_o^π + η·N_o^π | h, b, do(o) ],      o* = argmin_{o ∈ A_safe(h)} Q^π(h, b, o)

| symbol | meaning | source |
|---|---|---|
| Y_o^π | task success within the budget after taking o | task evaluator |
| T_o^π | remaining wall time from this decision to the end (failures and timeouts included) | timers |
| H_o^π | a person's ACTIVE time | client interaction records (not available yet) |
| N_o^π | number of optional interventions | decision log |
| L, λ, γ | trade-offs success / time / person time | set by the product or pre-registered; not learnable from logs |
| η | price of one optional intervention (the intervention-budget control) | development set / budget controller |

A_safe(h) is what the safety rules allow: deny, approvals, password hand-offs and required safety checks are rules,
decided before and outside this objective; they never compete on expected value (server.rs asserts this).

**What the gate estimates now: the local net gain over direct**

    G_o(h) = C_recover(h)·Δp_o(h) − λ·Δt_o(h) − γ·ΔH_o(h) − η·1[o ≠ direct]

(`research::local_gain`). Δp_o = P(step succeeds | o) − P(step succeeds | direct); **Δt_o is only the option's EXTRA
time before the step is attempted** (its own wait / probe / extra model turn), never the recovery time after a
failure: that is what C_recover·Δp_o already prices, so counting it in Δt would count it twice. C_recover(h) is the
prior `decide::fail_cost` (2·turn_cost + side_cost, × severity, × stuck factor; documented as a prior, not an
estimate). λ (`GUI_GATE_LAMBDA`) trades time; γ (`GUI_GATE_GAMMA`, default 1) prices person seconds; η (`GUI_GATE_ETA`,
default 0) prices the intervention itself, independent of its duration, because one check and one look_full differ
~100× in time but each spends one intervention of a 5-10 % budget. γ and η act on the treatment model only; the
legacy default is unchanged.

**Choice rule (treatment model).** An option is a candidate only where the state supports both it and direct
(section 10.3). For each candidate, the bootstrap replicates give a percentile interval of G_o; the option with the
largest LOWER bound is taken when that bound is > 0, otherwise direct (`tier: model_lb` when the point estimate alone
would have picked something else). The same rule applies to every option, check included. No full reinforcement
learning: the data and labels do not support it.

**Escalation state machine (advice only).** consecutive failures → switch strategy (≥ 2) → ask for help (≥ 4, or loop
breaker level ≥ 2; only when a person is reachable) → stop (≥ 6, or no person and level ≥ 2/3)
(`research::escalation`). It is logged on every action (`escalation`) and returned by gate_next_step; it never changes a
gate decision, and safety-required approvals never pass through it (the v3-sec branch owns asking a person).

## 10. Decision data contract and labels (v0.10.3)

### 10.1 What every action event records (`contract: decision_v1`)

| field | meaning |
|---|---|
| `decision_id` | `d-<session>-<aid>` |
| `x_pre` | features when the decision was first taken, before any gate-side option ran; immutable |
| `x_post` (= `features`) | features the final decision used |
| `x_post_observed`, `assumed_resets` | after a wait: what was observed, and the keys (`loading_cue`, `last_popup`) the default re-decision clears by assumption |
| `waited`, `rebound` | separate flags: having waited is not "loading finished"; a check / heal that re-bound the target also repaired it |
| `proposed` / `decision` | the option first proposed vs the final one (executed or refused) |
| `candidate` | tool, signature and a structural `effect_id` (tool, app, window, role, label, payload; research only, not the approval key) |
| `chain` | each processing step (check / wait / heal) with `t0_ms` / `t1_ms` since the decision started, its result, `includes_repair` for a re-binding check |
| `exec_t0_ms`, `exec_ms` | driver dispatch start and duration |
| `continues` | on the first action after a hand-back: its decision_id, `same_candidate`, `same_effect`, `model_switched`, `gap_s`, `attributable` |
| `labels` | `exec` (delivered / error / unknown), `local` (met / not_met / unknown, from the driver's effect report), `task` (always null in the gate) |

### 10.2 Labels: three layers, never standing in for each other

1. **Execution**: the driver delivered the step, reported an error, or the outcome is unknown. The step model's
   training label `y` is this layer (no error, no stale token, not immediately redone).
2. **Local progress**: the post-condition of the step (target control / file / app state), only where it can be judged
   (driver effect report today; pre-registered post-conditions in the increment pilot).
3. **Task**: success within the budget and the remaining time, from the evaluator, joined offline per task.

`gui-gate train --model treatment --label task` is refused: the task's success copied onto every action of the task
weights long trajectories more and mixes in the later policy; it is not an action-value label. `load_rows` keeps the
task result in `y_task` for task-level description only.

**Hand-back labels** are attributed only to the same candidate action: with the continuation link, the same decision
and `attributable` (same candidate and effect); in older logs, the same signature within 180 s. Otherwise the hand-back
is "cannot be attributed": no label, counted (`handback_unattributable`).

### 10.3 Coverage per state

The trainer always fits the logged policy's propensity (softmax logistic on decision-time features, probe results
excluded) and saves it with the model (`propensity`, `--support-clip`, default 0.05). At run time an option o is
considered at state x only when e_o(x) ≥ clip AND e_direct(x) ≥ clip AND o has `min_rows` effective rows
(`TreatmentModel::covered_at`). Otherwise the log lists it under `no_estimate_basis` with the reason. When no option
but direct is covered, the learned model has nothing to compare: the legacy prior decides and the log says
`fallback: "legacy_heuristic"`, `learned: false` (never called learned). A model without a saved propensity covers
nothing but direct.

### 10.4 Training only on pre-decision states

The trainers read `x_pre`. Older logs have no `x_pre`: a row's `features` is the pre-decision state only when no
gate-side option ran (no wait / check / heal in the trail). The other rows cannot be restored: they are excluded from
every causal estimate and counted (`pre_decision_state_unrecoverable` in the load statistics; `--include-post` keeps
them for descriptive statistics only). On the v0.9.9 run (`results/offline-v3-2026-10-26/`): 388 GUI rows have no
recoverable pre-decision state, 24 hand-backs are attributable to the same candidate and 133 are not; what remains is
direct 6,863, look_full 16, review 20 rows, and the state-level support of look_full is 0 % and of review 0.18 % of
training states. Under the new coverage rule the logs give **no estimate basis for any intervention**; a treatment
model trained on them falls back to the legacy prior everywhere (logged as such).

### 10.5 Uncertainty

Each bootstrap replicate repeats the whole estimation on its session resample: the propensity is refitted (under
`--ipw`) and the outcome model runs the full iteration count (0.10.2 reused the original weights and a third of the
iterations). `--boot` defaults to 200; 30 replicates (the 0.10.2 report) are a pipeline check, not an interval to
decide on. Optional cross-fitted doubly robust estimate (`--dr`, 5 session folds):

    φ_o = m̂_o(X) − m̂_d(X) + 1[A=o]/ê_o(X)·(Y − m̂_o(X)) − 1[A=d]/ê_d(X)·(Y − m̂_d(X)),   τ̂_o = mean φ_o on the overlap region

It requires one treatment definition per option, pre-decision covariates that contain every common cause (x_pre
only), and overlap (rows with ê_o or ê_d below the clip are trimmed and counted). DR is consistent when either m̂ or ê
is right; it does not create missing counterfactuals.

### 10.6 check

Without a probe model the gate no longer flips the check features to solve for P(pass) (that inverts the direct
predictor; it does not measure the probe). Without probe data check is a **cheap verification heuristic** (the
legacy grey-zone rule) and has no estimate basis in the treatment model. With a probe model its value of information

    VOI_check = min_a Q(h, a) − [ c_check + E_z min_a Q(h, z, a) ]

is computed per bootstrap replicate and enters the same lower-bound rule as every other option. A check that re-binds
the target also repairs it (`includes_repair` in the chain): its effect is not pure information.

### 10.7 Costs

`gui-gate costs` uses component timers when the log has them (chain steps; hand-back → the continuation of the same
candidate); a hand-back continued by another action or not within 600 s is censored and counted, not averaged. Every
option reports mean, median, p90, p99, n and censored. Older logs only give the gap to the next action of any kind:
an **upper bound / correlated proxy** (`timing_src: next_action_gap_upper_bound`), not the option's extra cost.

## 11. Three evidence chains

1. **The gate constrains execution** (v3-sec): every dispatch checked, unknown non-idempotent outcomes not unlocked by
   one read, approvals voided when the business preconditions change. Evidence: attack tests that count what the
   driver actually received.
2. **The estimated quantity is defined correctly** (this branch): immutable pre-decision features, explicit treatment
   chain and continuation, separate label layers, commands modelled apart, coverage per state, unknown coverage
   reported. Evidence: the data contract, the exclusion counts, incremental calibration on paired branches.
3. **Deciding with the estimate is worth it**: a frozen small policy in paired branch and online comparisons, reporting
   incremental calibration, task success, total time (and its tail) and the intervention budget together, with fixed
   failure and infra rules. Offline screening is never reported as online improvement. Evidence: section 12.3, not run.

## 12. Offline and online evaluation (v0.10.3)

### 12.1 bench/offline_v2.py

* A task and its layout variants are one group (`--variants`, or a `-v<N>` / `-layout<N>` suffix); bootstrap and
  splits use groups.
* Generalisation is reported separately: across task families (leave-one-domain-out), across tasks (development task
  groups → held-out test groups, `cross_task_test`); across runs is "not available: one run".
* L2 is chosen by inner cross-validation on development groups only (`hyperparameters`); test groups are scored once.
* Baselines on the same rows: same-budget random and the fixed heuristic (flagging); vp_direct, fixed observe, fixed
  reasoning, risk threshold, heuristic, same-budget random and equal-time wait are online-only arms (listed).
* The flagging table is a post-hoc top-k screen (claims: `flagging_budget: post_hoc_top_k_screen`).
* Incremental calibration (`incremental_calibration`, `--branches`): bins of predicted τ̂ vs the paired-branch
  difference (repeats averaged per checkpoint first, negative allowed, checkpoint bootstrap). Without paired branch
  data the section is empty and says "no paired branch data".
* Scroll-repeat label: `--scroll-repeat failure|not_failure`; both readings are reported (`label_sensitivity`). The
  manual review of those labels is pending (user).
* Costs: median, p90, p99, censored (> 600 s); labelled as an upper bound for logs without component timers.

### 12.2 What still needs online data (recorded, not done)

Incremental value of look / review / check / heal, a probe model for check, active person time (H), the L / λ / γ /
η values, and whether any new decision rule helps tasks.

### 12.3 Pilots (bench/online_plan.py pilots, plans only)

Frozen only when every one of pi, main_model, main_thinking, reviewer_model, roles, prompts, driver, app_env, timeouts,
cost_config, gate_commit, value_weights has a hash; otherwise the plan is written as `status: draft` with the missing
keys (and `--require-frozen` refuses).

* **Policy pilot**: Chrome, Impress, Calc, multi_apps × 3 tasks (seed 20261026); direct-with-safety / legacy /
  frozen-new; seeds 0/1/2; 108 task runs, blocked by task in a seeded order.
* **Increment pilot**: 24 safe, fully restorable checkpoints; direct / look_full / matched wait × 3 repeats = 216
  branch continuations; fixed continuation policy, local post-condition and continuation budget; clustered by
  checkpoint; incremental calibration of the frozen model's τ̂.
* **Analysis**: repeats aggregated within the task (checkpoint) first, then paired bootstrap / sign-flip permutation;
  no McNemar on repeat averages. **Infra (pre-registered)**: the primary analysis keeps every planned unit, external
  infra is re-run at most once with the same rule everywhere, valid pairs are reported apart, algorithm hangs are never
  infra. Example output (draft, nothing frozen yet): `results/online-plan-v3-2026-10-26/`.
