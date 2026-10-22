# Decision v2 (gui-gate 0.7.0 / 0.8.0): trained predictor + one expected-cost formula + one reviewer model

Every GUI action passes three stages before it reaches Cua Driver.

## 1. Hard rules (no probabilities)
Unchanged from 0.6: risky effects (delete / send / overwrite / purchase …) go to a person or are refused with a safe
alternative; a stale element token is re-bound; an action that just failed is not re-sent blind; pixel clicks after
the window changed need a fresh screenshot.

## 2. The predictor: P(direct) = P(this action succeeds if sent now)
A logistic regression, `P = sigmoid(a · Σ wᵢxᵢ + b)`, over 31 features of the current state. It runs inside the gate
in microseconds; it is **not** an LLM.

* v1 features (24): target kind (pixel / element / keyboard), observation age and actions since the last read,
  stale token, last action errored / had a popup, repeats, label empty or duplicated, loading cue, window changed or
  a new window appeared, probe result.
* **new in v2 (7)**: what this session has already shown — `prev_none` (last action had no visible effect),
  `sess_fail` (shrunk log-odds of failure so far: ln((fails+1)/(successes+4))), `consec_fail` (consecutive known
  failures, capped at 3), `sig_failed` (this exact action failed before) — and the element kind (`role_menu`,
  `role_cell`, `role_toggle`). A failure counts as known only when the live gate already knows it at decision time
  (driver error, stale-token error, no effect, or a repeat already flagged), so training and runtime see the same
  numbers (`features::history_feats` is shared by both).
* Weights: `gui-gate train` on 2,934 executed actions from 169 sessions (OSWorld + OSGuard runs of 2026-10-21),
  L2-regularised towards the hand-set prior (so rare signals such as stale tokens keep their prior weight), Platt
  calibrated. Label: the action did not error, was not a stale-token error and was not immediately repeated.
  Built in as the default (`osworld_v2`); `--weights prior_v1` restores the old hand-set weights.

Held-out quality (session split, so a model is never scored on a run it was trained on):

| | AUC (5 seeds, 30 % sessions held out) | Brier |
|---|---|---|
| hand-set prior (v1) | 0.50 – 0.58 | 0.13 – 0.15 |
| fitted, v1 features | 0.67 – 0.75 (mean 0.71) | 0.10 – 0.12 |
| fitted, v2 features | 0.73 – 0.80 (mean 0.76) | 0.10 – 0.12 |

(A separate 5-fold cross-validation with fully converged fits gives 0.716 → 0.772.)

## 3. The formula: pick the option with the lowest expected cost, in seconds

    E[cost(o)] = λ · cost(o) + (1 − P_o) · C_fail

* `cost(o)`: what the option itself costs. Gate-side options are cheap: probe 0.2 s, re-bind 0.3 s, wait ~1 s.
  Handing the step back to the model costs a whole model turn: re-read 7 s, re-read with screenshot 9 s,
  re-think 8 s (≈ 7 s per turn measured with Pi + relay). A person: 45 s.
* `P_o`: the **same predictor** scored on the state the option leaves behind (`decide::after`): a fresh read zeroes
  the staleness features, waiting clears the loading cue, re-thinking clears duplicate-label and repeat doubts; plus
  a small residual gain for what the features cannot see. So the predictor itself says how much looking again helps.
* `C_fail`: noticing and redoing a failed step takes about two turns plus clean-up (2·7 + 5 = 19 s), times how hard it
  is to undo (scroll / arrow keys ×0.5, typing ×1.5, Enter or OK / Save / Send / Delete ×2, risky ×3) and times
  (1 + consecutive failures)^e, e = 1.0 since 0.9.5 (was 1.5; env GUI_GATE_FAIL_EXP) — the more stuck the run, the more a failure costs.
* `ask_help` (a person does or explains the step) enters only when a person is reachable (not `--human deny`) and
  the run has failed twice in a row; with the escalation above it becomes the cheapest option once the run is
  clearly stuck. With a desktop / elicitation channel the gate asks directly ("please do this step, then Allow");
  in chat mode the agent is told to ask its user or try a clearly different way.
* Before handing a step back to the model, the gate tries its cheap probe once. The old fixed "grey band"
  (always probe when 0.55 ≤ P < 0.9) is gone: in the logs 330 of 332 probes passed and they flagged only 2 of 82
  later failures, at ~0.2 s each.

## 0.8.0: one language model in the gate, and it is not the agent's
The acting agent's model (e.g. Pi's) is never asked to judge its own step any more: `gate_ack` is gone (accepted and
ignored), the `decider=model` mode is removed. The only language model inside the gate is the independent reviewer
(`GUI_GATE_REVIEWER_URL` / `_MODEL` / `_KEY`, protocols chat | responses | anthropic | gemini | systemone (System One decision models such as Jev or decision-model-preview); reasoning-blind: it sees the user's
request, the concrete action and the window tree, never the agent's explanation). Since 0.9.2 it is only ever an
independent decision model reached over its API; the 0.9.0 `agent` mode (reusing the agent's own model via MCP sampling
or a one-shot agent CLI run) and the `auto` fallback to it are removed.

| situation | 0.6 / 0.7 | 0.8 |
|---|---|---|
| medium risk not authorized by the user's words | agent re-thinks, may retry with gate_ack | reviewer allows / denies; no reviewer API configured → refused (fail closed, 0.9.2); reviewer did not answer → a person if reachable, else refused with the safe way on |
| doubtful target, possible loop (option `review`, ~3 s) | agent double-checks, may retry with gate_ack | reviewer checks the target against the tree; no reviewer → a person when stuck, else "try a clearly different way" |
| high risk | person | person (unchanged) |
| re-read needed (`look_light`) | agent calls get_window_state, then retries | the gate reads the window itself and attaches the fresh tree to its refusal |
| re-aim needed (`look_full`, pixel targets) | agent calls get_window_state with a screenshot, then retries | the gate attaches a fresh screenshot to its refusal |

Since 0.9.3 a reviewer model is required: the gate refuses to start without one (`GUI_GATE_REVIEWER=off` is gone),
so "no reviewer" above now only means "the configured reviewer did not answer" (API error or timeout): a person
when one is reachable, otherwise the step is refused with a safe way on.

So a step the gate stops costs the agent one turn (re-plan with what is attached) instead of two (read, then retry),
and the agent is never asked "are you sure?". `review` is not offered for pixel targets: the reviewer reads the
element tree, not the screen. Baseline policies (`light`, `full`, `direct`) are unchanged so earlier runs stay
comparable.

Replay with a reviewer configured (`gui-gate replay ... --reviewer`): 12 of 2,934 steps go to the reviewer (10 of them
really failed); the 139 pixel re-aims remain, now with the screenshot attached.

## Offline replay on the same logs (`gui-gate replay`)
Each logged executed action re-decided by the current rules + formula, compared with what really happened to it
(437 of 2,934 failed). "Flagged" = the gate would not have sent it blind; whether the chosen option would then have
prevented the failure cannot be measured offline.

| | intervene | of which gate-side | failures flagged | precision |
|---|---|---|---|---|
| v2 formula + hand-set prior | 14.5 % | 5.6 % | 33.9 % | 34.9 % |
| v2 formula + trained v2 predictor | 13.9 % | 8.5 % | 41.0 % | 44.0 % |

The replay is in-sample for the trained predictor (it was fitted on these logs); the held-out AUC above is the
honest number for the predictor alone.

## Transport
* After an action that errored, the gate now also returns the fresh window tree (and fresh tokens) in the same
  result, so the agent does not spend a turn on a re-read before retrying.
* Not done, on evidence: retrying "no visible effect" clicks with foreground input. All 231 such cases in the logs
  already ran in foreground mode, so a delivery retry would not have helped.

## Not yet verified
No new benchmark run with 0.7.0 (stopped on request). Next step when allowed: the same 34-task matrix, gate 0.7.0
vs direct, think medium, plus a run with a person reachable to count ask_help.
