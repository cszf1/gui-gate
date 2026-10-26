# Changelog

## 0.10.3 (unreleased; branch v3-res: research / predictor / evaluation part of the 2026-10-26 review)

Checklist with the status of every review item: docs/review-v3-checklist-res.md. The default decision is unchanged
(golden test `value::tests::default_path_output_is_unchanged` passes unmodified); everything below either only adds
log fields, or changes opt-in paths (treatment / v2 value model), the trainers and the offline / online tooling.

- **Decision data contract** (src/research.rs, `contract: decision_v1` on every action event): immutable `x_pre`
  (features before any gate-side option) next to `x_post` (= `features`); after a wait the observed values and the
  keys cleared by assumption (`x_post_observed`, `assumed_resets`) and a separate `waited` flag; `decision_id`,
  `proposed` vs final decision, candidate with a structural `effect_id`, the processing `chain` with start / end times,
  driver `exec_t0_ms`, `continues` (hand-back -> next action: same candidate / same effect / model switched /
  attributable) and three label layers `labels.exec` / `labels.local` / `labels.task` (always null in the gate).
- **Trainers use x_pre only**; older rows whose trail ran wait / check / heal have no recoverable pre-decision state
  and are out of the causal rows (counted, `--include-post` for description). Hand-back labels only for the same
  candidate (and effect / decision where logged); otherwise "cannot be attributed" (counted). `--label task` is refused
  for the treatment model (a task result copied onto every action is not an action-value label).
- **Coverage per state**: the trainer saves the logged policy's propensity; at run time an option is considered only
  where e_o(x) and e_direct(x) reach the clip (`TreatmentModel::covered_at`); otherwise `no_estimate_basis`. With no
  covered option the legacy prior decides and the log says `fallback: "legacy_heuristic"`, `learned: false`.
- **Choice on the conservative lower bound** of the local net gain G_o = C_recover·Δp − λ·Δt − γ·ΔH − η·1[o≠direct]
  for every option (check included), instead of "point argmin, then check its interval". New `GUI_GATE_GAMMA`
  (person seconds) and `GUI_GATE_ETA` (intervention-budget price, separate from λ); both default to the old behaviour.
- **Bootstrap**: every replicate refits the propensity (under `--ipw`) and runs the full iteration count; `--boot`
  default 200 (30 = pipeline check). Optional cross-fitted doubly robust estimate `--dr` (formula and assumptions in
  the report).
- **check**: no more P(pass) by flipping features without a probe model; without probe data it is a "cheap
  verification heuristic" with no estimate basis; with a probe model its VOI interval enters the same rule; a re-binding
  check is marked `includes_repair`.
- **Costs**: `C_fail` documented as a prior; `gui-gate costs` uses component timers when logged (censored hand-backs
  counted, not averaged), reports median / p90 / p99 / censored, and labels next-action gaps of older logs as an upper
  bound.
- **gate_next_step** no longer calls the action value model with a fictitious token click: own freshness rule
  (`research::next_step_rule`) sharing auto-observe's duplicate-read test, plus advisory escalation
  (switch strategy -> ask for help -> stop; never changes a decision, never touches safety asks).
- **bench/offline_v2.py**: task + layout variants grouped; cross-family and cross-task reported apart, cross-run "not
  available"; L2 chosen on development groups only (the 0.10.2 "AUC 0.795 with weaker L2" was tried on the test split
  and is withdrawn as post hoc); same-budget random and heuristic baselines, online-only baseline list; incremental
  calibration code (`--branches`; empty and marked without paired data); "post-hoc top-k screen" wording; scroll-repeat
  label switch with both readings; cost quantiles and censoring; `--boot` (default 200).
- **bench/online_plan.py pilots**: policy pilot (4 domains × 3 tasks, seed 20261026, direct-with-safety / legacy /
  frozen-new × seeds 0/1/2 = 108 runs) and increment pilot (24 checkpoints × direct / look_full / matched wait × 3 = 216
  branches); "frozen" only with a hash for every one of 12 items, else "draft" (`--require-frozen` refuses); analysis
  aggregates repeats within the task first, no McNemar on averages; pre-registered infra rule. Plans only.
- Docs: docs/value-model-v2.md sections 9-12 (two objectives, contract, label layers, three evidence chains); result
  wording 185/361 = 51.25 %, 185/332 = 55.72 %, 28 infra, 1 not run, custom environment.

## 0.10.2 (2026-10-26) - last release of gui-gate as a standalone MCP

**This is the last release of gui-gate as an independent MCP server.** Development moves to
[cszf1/gui-pi-desktop](https://github.com/cszf1/gui-pi-desktop); the gate code evolves with the desktop app from here on
(the desktop builds pin this tag). This repository stays available; no further standalone releases are planned.

Failure analysis of the v0.9.9 OSWorld-Verified run (docs/failure-analysis-v099.md, bench/failure_analysis.py,
results/failure-analysis-v099-2026-10-26/): the 147 valid failures classified by cause from the logs; only generic gate
defects with log evidence are fixed here, everything that could fit the 361 tasks is listed as desktop research.
- The string method `x.replace(a, b)` in interpreter code is no longer read as a file move: `.replace(` with two or more
  top-level arguments is Python str / JS String; `os.replace`, one-argument `Path.replace(target)`, `shutil.move`,
  `os.remove` ... unchanged, an unclosed call keeps the old reading. Evidence: 40 refusals of read-only scripts in the
  v0.9.9 run (39 of the 40 original commands are no longer flagged; the 40th really calls `os.unlink`).
  Test: `command::tests::string_replace_is_not_a_file_move`.
- A menu item whose only activating accessibility action is `open` / `expand` (a submenu parent, e.g. Chromium's
  "Cast, save, and share" `[actions=[open,showContextMenu]]`) is treated like opening a menu: its label words do not make
  it risky. Leaf items (select / click / press ...) and rows without an action list are unchanged. Elements now keep
  their action list (`Element::actions`, from the tree markdown or the elements JSON). Evidence: chrome-35253b65 failed
  after the reviewer refused that click as "share". Test: `features::tests::submenu_parent_is_not_risky_but_leaf_items_are`.
- The value model default stays `legacy`; no thresholds changed.

Merged since 0.10.1 (branches v2-sec, v2-hum, v2-val):

Security regressions (branch v2-sec; tests/test_sec_regressions.py drives each one against tests/fake_driver.py):
- Person answers fail closed on conflict: an accepted form that says yes and no at once (`{allow: true, decision:
  "deny"}`, `done: true` next to `decision: "deny"`, `choice: "did_it"` next to `done: false`), a non-boolean `allow`,
  or a `decision` that is not exactly `allow` / `deny` is a refusal (`human::map_accepted` / `conflicting_form`).
  Before, `allow: true` won over `decision: "deny"`. A native yes/no dialog's output that is none of its buttons is a
  refusal too (`human::map_desktop`; before it was recorded as "accepted").
- A person's answer is bound to the task it was asked under: the MRTR answer carries the operator task version and the
  reset epoch (`task_binding`); after the user wrote again (new task-file version) the person is asked again instead of
  the old yes being used. A synchronous answer (dialog / legacy elicitation) given while the task changed approves
  nothing (`human_outcome: "task_changed"`). Permits carry the version the approval was given under (`approval_tv`).
- A read never writes: a read-only tool called with a destination (`screenshot_out_file`, `output_path`, `save_to`, a
  path-valued argument ...) and any tool with a capture-to-file argument is refused at the call and at the dispatch
  point; nothing is sent (`features::write_args`, `tier: "write_args"`).
- Nobody but the person or the timeout ends a question: `gate_reset` (which the agent can call) voids parked requests
  but no longer withdraws them, so input stays frozen until the person answers or it times out; a retry for another
  call or without an answer burns the approval but does not end the freeze either (`PendingTable::resolve` / `void_all`).
- An unmodelled (pass-through) action whose outcome was unknown is not re-sent blind: the same tool + arguments (key
  order and delivery mode ignored) is refused once with a fresh window list; a deliberate re-send after that is
  decided afresh (`Gate::pass_action`, `unmodelled_key`). Before, only gated GUI tools had this rule.
- results/osworld-verified-v099-ds41-2026-10-23 README / report: the custom-environment wording and Wilson intervals
  dropped by the final 361/361 refresh are restored with the final numbers (185/332 valid, 185/361 planned).

v2 person hand-off protocol and agent context (branch v2-hum; design: docs/gate-design-v2.md).
- Every parked person request now ends in one `human_outcome` log record carrying its request id (`req`): answered,
  late, expired without a retry (`no_retry`), voided by `gate_reset` / gate exit (`void`), or answered but not used by the
  retried call (`unused: frozen | effect_changed | not_needed`). Before, a request whose retry never came, a reset, a gate
  exit, or an answer the retried call did not reach left no outcome at all.
  Merged with v2-sec: a reset logs `void` (why=reset) for each request it newly voids but does not withdraw it; input
  stays frozen until the person answers (approves nothing) or it times out, and that timeout is logged by the sweep
  (`outcome: timeout, no_retry, voided: true`). Gate exit logs `void` (why=gate_exit) for requests not already voided.
  A conflicting ask_help form (`{choice: "hint", done: true}`) stays a refusal, not "another way"; a yes/no desktop
  dialog's output that is none of its buttons is a refusal.
- ask_help "hint" / "try another way" is an answer (`human_outcome: accepted`, `human_choice: another_way`), not
  `denied`: gui-pi turned `denied` into "the person declined, do not try another way", the opposite of the hint.
- Desktop dialogs: the default button is the refusing one (zenity `--default-cancel`, osascript, GTK, WinForms); a typed
  answer that reads "timeout" / "allow" is that answer (text channel kept apart from the buttons); a failed helper is
  `channel_unavailable`, not a denial; an open dialog is killed when the gate exits.
- Legacy clients: a JSON-RPC error answer to `elicitation/create` is `channel_unavailable`, not a timeout.
- `request_human` over MRTR counts one ask (was two); its no-answer check uses the outcome, never the answer text.
- Takeover without a person channel logs `human_outcome: channel_unavailable` like approve does.
- Agent context: a reviewer refusal no longer quotes the reviewer's reason (it stays in the log); `gate_report.reviewer`
  is `configured | none` instead of the reviewer's protocol and model; app / page / terminal / command text lines that
  start like the gate's own messages (`[gate]`, `[human via`, ...) are prefixed `(app text) `.

Value model v2 and offline evaluation (branch v2-val, docs/value-model-v2.md). The default decision path is unchanged
(golden test of 189 decisions generated on c3893bc).
- `GUI_GATE_VALUE_MODEL` defaults to `legacy`; `auto` (the 0.10.1 default: treatment whenever its weights were loaded)
  is opt-in. New value `v2`: commands are not scored by the GUI predictor (`tier: command`, `p_direct: null`), GUI
  history features skip commands, click/scroll/role features (`features::V2_FEATURES`); the treatment model only with
  `treatment_v2` weights.
- A treatment model with fewer than 20 bootstrap replicates has an unknown interval (was: zero-width at the point
  estimate) and is not used: the legacy prior decides and the value log says `fallback`.
- Trainers: a collect-mode shadow probe is no longer copied into the features; the propensity model never sees probe
  results; IPW coverage = min(n, ESS) and 0 below `--min-support` (0.2); `run_command` rows and later sessions of a log
  are excluded by default (`--commands include`, `--all-sessions`); `--features v2`. `gui-gate replay` counts commands
  apart from GUI interventions.
- bench/offline_v2.py (offline evaluation with an exclusion audit, GUI vs command calibration, failure-prediction
  budgets, costs from gate + Pi logs; never claims avoided failures) and bench/online_plan.py (frozen online plan,
  runs nothing). Results: results/offline-v2-2026-10-26/.

Fixes from the gui-pi-desktop 0.3.0 live test on a Linux desktop (2026-10-25).
- Person results: a declined / cancelled form now keeps the client's reason (`_meta["io.github.cszf1/gui-gate"].reason`
  or a top-level `reason`): `timed_out` -> `human_outcome: "timeout"`, `expired` (the client never showed it: another
  question was open) -> `"expired"`, `closed` -> `"window_closed"`, `unavailable` -> `"channel_unavailable"`. Before,
  every timeout was recorded as `declined`. `expired` is a no-answer outcome (never a training label, not a refusal
  by the person). `request_human` says "No answer: the person did not answer within N s ..." instead of "cancelled".
- Takeover / ask_help texts no longer name buttons ("press Allow / Deny") that the client's popup does not have.
- `run_actions`: before every step after the first the gate re-reads the step's window. While it shows a loading cue
  or the step's target is disabled the batch waits up to 3 s, then stops there (`N. tool: NOT RUN - ...`); a
  window/dialog that appeared after the previous step also stops it. Tokens of the batch are re-bound to the fresh
  read by label. Before, a batch clicked disabled buttons during a load and reported every step "ok".

## 0.10.1 (2026-10-25)

Follow-up to the external review of 2026-10-25 (security boundary, person requests, value model, evaluation) and the
interface the gui-pi desktop client (design 0.3) needs. Includes everything from 0.10.0 below.

### Security (docs/security-model.md)
- Read-only driver tools are an explicit allowlist; every other tool (`launch_app`, `bring_to_front`, any tool a newer
  driver adds) is an action and passes the single final dispatch check. Pass-through actions are never sent raw.
- `launch_app` risk: URLs, terminals/interpreters with a command (classified like `run_command`), file managers on a path.
- Input is frozen while a person is answering (entry and dispatch point); the gate's own dialog windows/pids are never a target.
- A task that names only a file name covers only the file in the working directory.
- `--task-file` trusted channel: `gui-pi-task/1 version=N` with `--- user <i> <UTC> initial request|later message`
  sections; re-read on change, versions logged by hash; permits and "not asked again" are bound to the task version;
  a newer operator version may supersede an older denial only when it explicitly covers the effect.
- MRTR `requestState` is signed per gate process (`gg1.<instance>.<nonce>.<HMAC>`) and invalid after a restart.
- Strict person result types (approve / answer / takeover / ask_help with `did_it|hint|try_other`); an answer never
  approves; takeover/help done → `status: "done_by_person"`, never re-executed; a request that got no answer is not
  asked again for the same effect within a task version (`human_outcome: "not_asked_again"`).
- `gate_reset` voids permits, approvals, pending person requests and cached observations / snapshot ids.
- Known secret values and common key shapes are redacted from every tool result and log line.

### Desktop client interface (docs/mcp-2026-07-28.md)
- Client-only tools `gate_next_step` (advisory pre-step decision: observe none/tree/tree+screenshot, extra_reason)
  and `gate_reset`; not listed to the model, usable unmarked.
- `_meta["gui_pi/usage"] = {input, output, cache_read, same_message?}` per tool call: logged, summed in
  `gate_report.model_usage`, used by `gui-gate costs`.
- Person request `_meta["io.github.cszf1/gui-gate"]`: approve `object/scope/consequence/target/effect`, clarify
  `options[{label,value,effect}]` + `allow_other`, help `tried[]`, takeover `steps[]`.
- Status `done_by_person`; `next_action: "rethink"` with `reasoning_budget_tokens: 1024` on extra_reason.

### Value model (docs/value-model.md)
- `extra_reason` is a rethink of the acting model (no new observation, 1024-token hint, authorizes nothing);
  `review` is not extra reasoning.
- `train --model treatment --ipw [--ipw-clip]`: inverse-propensity weights from the logged policy with ESS overall
  and per option; `--report` compares interaction, separate-per-option and boosted-stumps variants on the same split.
- Legacy prior remains the default (no treatment weights ship). Safety asks are never valued; no-answer outcomes are
  never training labels.
- Person burden (asks, repeat ratio, wait/active seconds, takeover steps, no-answer rate, not-asked-again) in
  `gate_report`, `person_burden` log events and `gui-gate report LOGS`.

### Roles
- `planner`, `search`, `compact` are off by default even when configured (`GUI_GATE_ROLE_<NAME>_ENABLE=1` or
  `GUI_GATE_ROLES_ENABLE=...` to turn on).

### Evaluation (merged from fix-bench; docs/eval-protocol.md, bench/README.md)
- Evaluation protocol tooling: frozen task lists, planned and valid denominators, Wilson CIs, paired task-level
  bootstrap, McNemar, permutation, repeats, budget curves, human-pilot analysis and counterbalanced assignment.
- `bench/human_pilot.py from-logs` converts the desktop client's `logs/human.jsonl` (`gui-pi-human/1`) and the gate's
  `person_burden` events into pilot sessions.
- Results wording: "custom environment based on OSWorld tasks and evaluators, not comparable to the leaderboard";
  v0.9.9 numbers match the committed data: valid 137/256 = 53.5 % (Wilson 47.4–59.5 %), planned 137/361 = 38.0 %
  (33.1–43.1 %).

### Docs
- README: reviewer is "one necessary line of defence, not sufficient on its own"; JPEG q60 showed no significant
  difference from PNG in our tests but is not proven equivalent; the ~3x first-token speedup holds only for the
  tested model and API.

## 0.10.0 (2026-10-25)

- Single final dispatch check: every GUI action the gate sends (direct, focus/occlusion re-sends, heals, batch
  sub-steps, terminal line clearing) is rebuilt on its final arguments and needs a one-shot permit bound to its
  structured effect.
- Structured effect keys `{tool, hazard, resources, app, window, payload}`; denial records (stable scope, any tool)
  separated from one-shot approval permits (30 s, used once).
- No agent self-approval: `gate_confirm` removed, `gate_task` is context only; authorization only from the
  launcher's task text, the independent reviewer (medium risk) and a person.
- Result status `unknown` (never re-sent; read the window first) and `input_required`; `human_outcome` on every
  person request.
- Structured tree diffs with `base_snapshot_id`.
- Treatment-interaction value model (legacy prior stays the default), measured option costs, `check` as value of
  information, fixed fair-baseline policies (`GUI_GATE_VALUE_POLICY`), counterfactual harness.
