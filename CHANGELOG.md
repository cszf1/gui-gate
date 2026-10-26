# Changelog

## Unreleased (goes into 0.10.2)

v2 person hand-off protocol and agent context (branch v2-hum; design: docs/gate-design-v2.md).
- Every parked person request now ends in one `human_outcome` log record carrying its request id (`req`): answered,
  late, expired without a retry (`no_retry`), voided by `gate_reset` / gate exit (`void`), or answered but not used by the
  retried call (`unused: frozen | effect_changed | not_needed`). Before, a request whose retry never came, a reset, a gate
  exit, or an answer the retried call did not reach left no outcome at all.
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
