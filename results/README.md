# All test data (kept from the very first run)

| folder | when (CST) | what | agent |
|---|---|---|---|
| pilot-2026-10-20/ | 2026-10-20 ~21:00 | First pilot on GateFixture: gate vs screenshot-every-step, training data for the light predictor. `box-raw/` = everything the box wrote (logs, logs2, train_logs, STATUS files). | scripted agent (sim_agent.py), NOT an LLM |
| pi-2026-10-20/ | 2026-10-20 22:45 | First real-model run: direct / full / formula / model deciders × 3 seeds. | Pi + deepseek-v4-flash |
| pi-2026-10-20b/ | 2026-10-20 23:20 | gui-gate 0.2.0 (core toolset, auto-observe, risk levels): direct / full / gate × 3 seeds. | Pi + deepseek-v4-flash |
| osworld-2026-10-20-aborted-keyboard/ | 2026-10-20 | First OSWorld/OSGuard try, aborted: background keyboard delivery failed and a VNC viewer shrank the screen to 979x550. Invalid, kept for the record. | Pi + deepseek-v4-flash |
| osworld-2026-10-21/ | 2026-10-21 00:55-08:00 | Full matrix: 24 OSWorld-Verified tasks (cua-speedrun subset) + 10 OSGuard-style risk tasks × direct / full / gate, seed 0, think medium, 40 turns / 480 s. direct and full rows are the baselines. The gate was improved WHILE this ran (0.2.x → 0.3.0 at 01:26, 0.3.1 at 01:48, 0.3.2 at 02:28 CST), so its gate rows mix versions (intermediate data); runs.jsonl times tell which. | Pi + deepseek-v4-flash |
| smoke-033/ | 2026-10-21 08:05 | Smoke test of 0.3.3 (auto-observe diff read). Found the agent got an empty tree after each action; diff read dropped in 0.3.4. og03 run killed mid-way. | Pi + deepseek-v4-flash |
| osworld-2026-10-21-gate034/ | 2026-10-21 08:11- | Gate-only pass over the same 34 tasks with the frozen gui-gate 0.3.4, compared against the direct/full baselines of osworld-2026-10-21. | Pi + deepseek-v4-flash |

Conventions: screenshots are stripped from Pi event streams (`"data":"<image stripped>"`) to keep the repo small; everything else is raw. Runs that died from the AgentRouter relay error (400 `content[].thinking must be passed back`) are infrastructure failures and are not counted against the gate.
