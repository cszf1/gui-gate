> UPDATE 2026-10-21 (0.5.0): the optional QuickJS `gui_script` was dropped so the package has no extra
> dependencies. Batch mode is `run_actions` with a small JSON program (action steps + find / for_each / if /
> read), interpreted in pure Python. Cua Driver now comes with the package (official `cua-driver==0.37.0`
> wheels), so `pip install gui-gate` is the whole install.

# Codemode inside gui-gate (design, theory only) — 2026-10-21

Problem: Pi's codemode (MIT, earendil-works/pi, packages/coding-agent) runs on the client. Any other MCP client
(Claude Code, Codex, Cursor, a plain SDK loop) connecting to gui-gate gets no batching, so it pays one model turn
per GUI action. Measured: the model turn (6-8 s incl. ~3.1 s relay) is the bottleneck, not the gate (~0.1 s).

Decision: put batching on the SERVER side of gui-gate, so every client gets it. Pi's TypeScript cannot be
embedded as-is (gui-gate is zero-dependency Python; Pi's codemode is TS on Node/Bun + QuickJS workers), so we
re-implement its script API in Python and credit Pi (MIT) for the API and docs wording.

## Two tools, one engine
1. `run_actions` — always available, zero dependencies. JSON list of steps:
   `{"steps":[{"tool":"click","args":{...}}, {"tool":"type_text","args":{...}, "expect":{"title_contains":"Save"}}], "stop_on_new_window":true}`
   Works for weak models and every client; covers the common case (open menu -> pick item -> type -> Enter).
2. `run_script` — optional (`pip install gui-gate[codemode]`, uses the `quickjs` package: MIT, x86_64/macOS/
   Windows wheels; on other platforms it builds from source or the tool is simply not listed). Same globals as Pi
   so Pi-style prompts transfer: `tools.<name>(args)`, `text()`, `image()`, `console.log`, top-level `return`,
   `store/load` (per gate session), `ALL_TOOLS`. No timers, file system or network; host calls are synchronous.
   Limits: timeout (default 60 s), memory cap, max 15 GUI actions per script, output cap.

## Gate semantics inside a batch (the part Pi cannot give us)
- Every inner call goes through `gated()` exactly as a single call: same risk rules, authorization check,
  value decision. A refusal throws inside the script / stops the action list, returning what ran, what was refused
  and why (deny-and-continue text from design-v0.4). Calls already executed are not undone (as in Pi).
- No auto-observe between inner actions; ONE observation at the end of the batch (or at the stop point). This is
  where the saving is: today 94% of auto-observes carry a screenshot, one per action.
- Blind-batch guard: stop the batch when the gate sees an unexpected new window/dialog, a stale token, a driver
  error, or a failed `expect`; return the fresh observation so the model re-plans. (Smoke data: gate_cm halved
  turns on a Chrome task, 9 vs 19, but hit the 40-turn cap on a Calc task where the tree is partial — batches
  must stop early instead of continuing blind.)
- Pixel actions inside a batch are allowed only on the first step (coordinates come from the last screenshot);
  later pixel steps must use element tokens/names, otherwise the batch stops — coordinates go stale after the
  first action changes the screen.
- Risky actions are never batched past: a T1-flagged step ends the batch before execution unless T2 authorizes it.
- Log each inner call as a normal action row with `batch_id` + step index, so scoring stays comparable.

## Client interplay
- If the client already has codemode (Pi), either works; Pi codemode calling single gated tools still gets every
  safety check. `--batch off|actions|script` lets benchmarks pin one.
- Tool descriptions say: "use run_actions for several steps whose targets you can already see; the gate stops
  the batch if the screen changes unexpectedly."

## Expected effect (to verify later, not measured)
- Turns: a batch of k actions costs 1 turn instead of k (+ fewer duplicate reads). For 33-37 turns/task this is
  the largest available speed lever for every client.
- Tokens: one observation per batch instead of per action.
- Safety: unchanged per action; stricter than client-side batching because the gate also stops on surprise.

## Risks
- `quickjs` PyPI wheels are x86_64-only; keep `run_actions` as the portable path.
- Models may over-batch; cap steps and stop on surprise.
- A script can loop on tool calls; count inner calls toward the denial budget (3 consecutive / 8 total).
