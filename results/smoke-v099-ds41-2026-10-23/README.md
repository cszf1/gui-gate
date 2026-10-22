# Smoke before the full OSWorld-Verified run (gui-gate v0.9.9, all 0.9.7+ features, 7 DeepSeek roles), box gui-pi3, 2026-10-22/23 CST

Same configuration as results/osworld-verified-v099-ds41-2026-10-23/ (see its README).

- rolecheck/: direct MCP session against the installed gate (no LLM agent) on a LibreOffice Writer window with every
  switch on: feasibility + planner (task briefing), search (inspect howto), grounder (inspect locate, native JPEG image),
  judge (run_actions pick_one), compact (digest every 2 steps), verifier (check_done, correctly FAILED because the file
  was not saved). All 7 roles answered without error. Screenshot returned as JPEG q60 at the window's native size.
- smoke-v099-ds41/ (3 tasks, Pi + deepseek, before two harness fixes):
  - chrome da46d875: setup_failed. Real Google Chrome 156 was tried first; Chrome >= 136 ignores
    --remote-debugging-port on its default profile dir (which OSWorld getters read), so the box went back to Chromium.
  - thunderbird 15c3b339: success (score 1.0), 9 turns, 268 s. Two check_done calls hit Pi's 60 s MCP timeout while the
    verifier role was probing -> run_bench now sets the MCP server timeout (PI_MCP_TIMEOUT=600).
  - libreoffice_calc 4e6fcf72: eval_error "Employee age reference date was not initialized during setup" (the
    host_reference_date state lived only in the setup process) -> osworld_native.py now carries it from setup to eval.
- smoke2-v099-ds41/: chrome da46d875 again with Chromium + fixes; setup OK, agent working on the live MBTA booking
  site; stopped by the operator after ~8 min (no score) to start the full run. Playwright CDP getter path checked by hand.
- Reviewer: `gui-gate check-reviewer` 2/2 correct (System One, 0.7 s / 1.7 s); no risky step came up in the smoke.
