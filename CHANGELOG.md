# Changelog

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
