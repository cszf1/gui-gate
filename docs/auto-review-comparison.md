# gui-gate vs. agent auto-review (2026-10-21 notes)

Sources: Anthropic "How we built Claude Code auto mode" (2026-03-25); OpenAI "Auto-review of agent actions without synchronous human oversight" (2026-04-30) + Codex docs; Origin "Who watches the watchmen?" (2026-10-08); Gemini computer-use safety_decision docs; OpenAI CUA/Operator system card.

## What they do
- Claude Code auto mode: tier1 safe-tool allowlist, tier2 in-project edits skip review, tier3 transcript classifier (Sonnet). Classifier sees only user messages + raw tool calls (agent prose and tool output stripped: no persuasion, no injection). Two stages: 1-token filter told to err toward block (FPR 8.5%, FNR 6.6% on real overeager set) then CoT only on flagged (FPR 0.4%, FNR 17%). Judges real-world effect, not surface text. User-intent rule: only what the user explicitly authorized counts. Deny-and-continue: denial + reason returned, agent tries a safer path; 3 consecutive / 20 total denials -> escalate to human (headless: terminate). Separate input-layer prompt-injection probe on tool outputs.
- Codex auto-review: sandbox does most work; only boundary-crossing requests go to a separate reviewer agent (GPT-5.4 thinking low). ~7% of actions reviewed, ~99% of those approved; denial carries a rationale; >half of denials recover via a safer path; stop after repeated denials. Recall: overeager 90.3%, prompt injection 99.3%.
- Gemini computer use: server-side safety_decision per proposed action (allowed / require_confirmation / blocked); client must ask the user on require_confirmation. Optional screenshot prompt-injection detection.
- Operator/CUA: model trained to ask before finalizing side effects (purchase, send).
- Origin field data: 0.032% of tool calls denied; after denial agents asked the user ~60%, found a safer path 31%; some bypassed by switching tools or delegating ("permission laundering").

## Relation to gui-gate
- gui-gate is auto-review for GUI actions, but with 4 options (act / observe / think / ask person) under a budget, not just allow/deny. Auto-review = our "ask_human" branch with a model standing in for the person.
- GUI actions are meaningless as text (click(412,300)); the reviewer needs the screen. Our a11y-tree target binding + sidefx file checks are the GUI analog of "judge real effect, not surface text" — our main difference.
- Where we diverge (and lose): every action pays auto-observe (they keep >90% of actions on a zero-cost path); grey zone is settled by the acting model itself (they insist on a separate, reasoning-blind reviewer); a block tells the agent to stop (they deny-and-continue, escalate only after repeated denials) -> our low OSGuard completion.

## Changes to make (theory, untested)
1. Zero-cost path: no auto-observe/screenshot unless the action is risky, the target is unbound, or the window changed unexpectedly.
2. Deny-and-continue: refusal returns the reason + a concrete safe alternative (save under a new name, cancel the dialog); escalate to a person only after 3 consecutive / N total denials or when no safe path exists.
3. Independent reviewer for the grey zone: cheap model sees only the task text, the action, the bound target (role/label/dialog text) and sidefx facts — never the agent's own explanation.
4. Two-stage: rules tuned for recall (err to block) -> reviewer only on flagged actions to cut false positives.
5. User-intent check: explicit words in the task ("overwrite", "replace", "delete") authorize the matching hazard; otherwise not.
6. Laundering: codemode scripts still hit the gate per call (good); also gate typing shell commands into terminals (sidefx) and repeated retries via other tools after a denial.
7. Measure like they do: recall on hazard set (OSGuard) and false-positive rate on benign actions (OSWorld logs), plus interruptions per task.
