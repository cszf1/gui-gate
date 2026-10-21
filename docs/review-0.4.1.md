# 0.4.0 → 0.4.1 self-review (theory, unit tests only, no benchmark) — 2026-10-21

Holes found by reasoning about how an agent would actually use 0.4.0, and what was changed.

| # | Hole in 0.4.0 | Why it matters | Fix in 0.4.1 |
|---|---|---|---|
| 1 | `run_actions` could only click with tokens from the snapshot taken before the batch | the most common batch ("open File → Save As → type name → Enter") needs items that appear only after step 1; every such batch would stop after one step | steps may name a target `{"label", "role"}`; the gate reads the screen at that moment (newest windows first) and turns it into a fresh token; ambiguous / missing → stop with what is on screen |
| 2 | any new window stopped the batch, including the popup menu the batch itself opened | same as 1 | a new window whose elements are all menu/list entries counts as an expected menu; any other new window (a dialog) still stops unless `expect.new_window` |
| 3 | after a menu opened, an old token could still be sent | it would point into the window behind the menu | refused: "name the item with target:{label}" |
| 4 | `gate_task` could be called at any time | an agent facing an overwrite dialog could "remember" a request that authorizes it (permission laundering through the gate's own tool) | accepted only before the first action; a request relayed by the agent can never authorize send / purchase / permissions / power / install / logout / shell commands (only `--task` from the operator can) |
| 5 | with a reviewer configured, `gate_ack:true` still let the acting model approve its own medium-risk step | defeats the point of a reasoning-blind reviewer | with a reviewer, medium risk always goes to it; only when the reviewer is unreachable does the old self-check apply (fail-soft, logged as `reviewer_down_selfack`) |
| 6 | an authorized risky step skipped the remaining zero-cost rules | an authorized click with a stale token or on an unseen window would go straight to the driver | after authorization the remaining rules (stale token, no observation, repeat-after-failure) run as usual |
| 7 | reviewer HTTP call went through the system proxy even for a local endpoint | local reviewers (e.g. a small model on the same box) would silently fail | loopback endpoints bypass the proxy |

Unit tests: 73 (box, quickjs installed) — new tests for each row above. No benchmark was run.
