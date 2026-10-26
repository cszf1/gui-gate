# Changelog

## Unreleased (goes into 0.10.2)

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
