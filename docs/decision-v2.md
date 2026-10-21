# Decision v2 (gui-gate 0.7.0): trained predictor + one expected-cost formula

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
  (1 + consecutive failures)^1.5 — the more stuck the run, the more a failure costs.
* `ask_help` (a person does or explains the step) enters only when a person is reachable (not `--human deny`) and
  the run has failed twice in a row; with the escalation above it becomes the cheapest option once the run is
  clearly stuck. With a desktop / elicitation channel the gate asks directly ("please do this step, then Allow");
  in chat mode the agent is told to ask its user or try a clearly different way.
* Before handing a step back to the model, the gate tries its cheap probe once. The old fixed "grey band"
  (always probe when 0.55 ≤ P < 0.9) is gone: in the logs 330 of 332 probes passed and they flagged only 2 of 82
  later failures, at ~0.2 s each.

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
