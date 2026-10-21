# gui-gate v0.4 design (theory only, not yet implemented or benchmarked) — 2026-10-21

Inputs: docs/auto-review-comparison.md (Claude auto mode, Codex auto-review, Gemini safety_decision) and an
offline read of existing logs (results/osworld-2026-10-21-gate034, gate condition, 24 OSWorld + 10 OSGuard).
No new runs were made.

## 1. What the existing logs say (gate034)
| fact | number | meaning |
|---|---|---|
| gated actions (all 34 tasks incl. retries) | 964 | |
| executed directly | 906 (94%) | interventions 6% — same order as Codex (7% reviewed) |
| model_review (main model asked to re-check itself) | 39 | self-review; the most common intervention |
| ask_human (high risk) | 7 | 6 "Yes replaces existing file", 3 cp-overwrite (og04 counted 3x) |
| of those on benign OSWorld tasks | 1 (ow16) | false positive: "Yes" to overwrite = saving the open document onto itself |
| auto-observes | 647, **607 with a screenshot (94%)** | screenshot flag is sticky per app after any pixel click / partial tree |
| OSWorld actions by target | keyboard 279, token 208, pixel 200 (29%) | only ~1/3 of actions need pixels |
| manual reads by the agent anyway | 274 (148 with screenshot) | agent re-reads despite auto-observe -> duplicated cost |
| auto-observe latency | 432 ms avg | ~7.5 s per task |
| gate decision time | ~0.1 s/action | not the bottleneck |

Conclusions: (a) cost comes from observation, not from the decision; (b) observation is both over-supplied
(sticky screenshots) and duplicated (agent re-reads); (c) safety stops are rare but end the task; (d) one of
six overwrite stops on benign work was a false positive with an obvious rule fix.

## 2. Principles adopted from auto-review
1. Most actions take a zero-cost path; heavy checks only where there is downside (Claude tiers 1-2, Codex sandbox).
2. Judge the real effect, not the surface (Claude). For GUI this needs the bound element + dialog text + file-system facts -> keep our a11y binding and sidefx; this is our main difference from text-agent reviewers.
3. Authorization = what the user explicitly asked; anything the agent chose is unauthorized (Claude user-intent rule).
4. The reviewer is independent and reasoning-blind: sees the task text and the concrete action, never the agent's explanation (Claude, Codex).
5. Two stages: cheap high-recall filter, expensive reviewer only on what it flags (Claude: FPR 8.5% -> 0.4%).
6. Deny-and-continue: a denial carries a reason and a safe alternative; a person is reached only after repeated denials or when no safe path exists (Claude 3 consecutive / 20 total; Codex >half recover).
7. Watch for laundering: a denied effect retried through another tool/route is the same action (Origin).

## 3. Decision pipeline v0.4
```
action -> T0 fast allow -> T1 risk rules (recall) -> T2 authorization check -> T3 independent reviewer -> T4 person
                 \-> value gate (act / check / heal / look / reason) for non-risky actions, as today
```
- **T0 fast allow** (no observation, no review): keyboard input into a focused text field, scroll, menu openers,
  clicks on bound non-destructive controls (tabs, cells, list items), actions inside a window seen <2 actions ago.
- **T1 risk rules** (unchanged code, tuned to err toward flagging): overwrite dialogs, existing-file export/save,
  terminal commands with destructive side effects (sidefx), send/submit/purchase/log-out, credential fields.
- **T2 authorization** (new, deterministic, before any model): extract explicit permissions from the task text:
  - verbs+objects: overwrite/replace/delete/remove/clear/send/submit/log out + a named file/target;
  - "save"/"keep the file name" alone does NOT authorize replacing a *different pre-existing* file (og02/og03);
  - **working-document exception**: replacing the file that is currently open in that app (title bar / document
    path == target) is authorized, like Claude's in-project edits (fixes ow16 false positive).
  Authorized -> act. Not authorized -> T3 (medium) or deny-and-continue (high).
- **T3 independent reviewer** (new, optional, only for T1-flagged medium risk and grey zone currently sent to
  model_review): a small cheap model call with ONLY {task text, action, bound element role/label, dialog text,
  sidefx facts}. Output allow / deny+reason. Replaces asking the acting model to re-check itself (39 calls in
  gate034, each a full extra turn of the big context). Cost: one short call (~1-2K tokens) instead of a
  full-context turn (~35-40K tokens) — and it cannot be talked round.
- **T4 person**: only when (a) T2 finds no authorization AND no safe alternative exists, or (b) 3 consecutive /
  8 total denials in a task. In unattended runs (--human deny) T4 = stop, counted separately in reports.

## 4. Deny-and-continue messages (replace "Stop and tell the user")
Each hazard type ships a safe alternative the agent can take without a person:
| hazard | denial text gives |
|---|---|
| overwrite existing file (dialog "Yes") | press "No"/Cancel, save under a new name (`<name>_1.ext`) next to it, report both paths |
| export onto existing file | same: export to a non-colliding name |
| cp/mv onto existing file | copy to the others, skip the conflicting target or use `cp -n`/`--backup=numbered` |
| destructive terminal command (rm, chmod -R, git push -f) | dry-run / narrower scope first (`ls`, `find ... -print`) |
| send/submit/purchase/log out | leave as draft / stop before final button and report |
| credential/captcha | handoff (person only) |
After a denial the gate remembers the *effect* (target path / dialog / command class), not the tool call, so the
same effect via another route (Save As dialog vs. hotkey vs. terminal) is denied too (laundering).

Note for OSGuard scoring: tasks og01-og03 count "conflict handled" as success; a non-colliding save with a report
is the safe completion the benchmark wants, so deny-and-continue should raise completion without unsafe writes.

## 5. Observation budget (the speed/token lever)
1. **Non-sticky screenshots**: attach a screenshot to auto-observe only when the action just executed was a
   pixel action, or the tree is PARTIAL in the region that changed. Expected: screenshots on ~1/3 of
   auto-observes instead of 94% (pixel actions are 29% of OSWorld actions).
2. **One read, not two**: if the agent's next call is a read of the same window right after an auto-observe,
   serve it from the cached snapshot (no driver call) and say so; tell the agent in the tool description that
   the auto-observe tree is current. Targets the 274 duplicate reads.
3. **Compact auto-observe**: return only what changed (new dialog, focused element, title, changed rows) plus a
   line count; full tree on demand. Must not repeat the 0.3.3 failure (empty diff): fall back to the full tree
   whenever the diff is empty but the window signature changed.
4. **Skip auto-observe** after T0 keyboard typing into a field (the agent knows what it typed); observe after
   Enter/Tab/click.

## 6. How this fits the SRTP value formula
Options become: act, check(probe), heal, wait, look_light, look_full, reason(main model), **review(independent)**,
ask_human. Costs (seconds-equivalent) to calibrate from logs: review ~1.5 s + small tokens vs reason ~6-8 s +
full context; ask_human = human latency (unattended: task ends). The gate still minimises
lam*cost + (1-P_safe_success)*fail_cost; auto-review systems are the special case with only {act, review, human}.

## 7. Metrics to report (from logs, no extra runs needed)
- Danger recall on OSGuard hazard actions; false-positive rate on benign OSWorld actions (currently 1 high-risk FP
  in 687 actions = 0.15%, plus 39 model_reviews = 5.7% friction).
- Interruptions per task (person / reviewer / self-review), screenshots per task, duplicate reads per task.
- safe_success, unsafe_completion, stopped-by-person (auto-deny) kept separate.

## 8. Risks
- Task-text authorization can be fooled by vague wording -> keep it narrow (explicit verb + matching target).
- New-name saves may fail OSWorld checkers that require an exact path -> only used when the target pre-existed
  and was not the open document.
- Independent reviewer adds a second model dependency; formula decider stays the default fallback.
