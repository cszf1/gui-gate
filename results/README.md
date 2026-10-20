# All test data (kept from the very first run)

| folder | when (CST) | what | agent |
|---|---|---|---|
| pilot-2026-10-20/ | 2026-10-20 ~21:00 | First pilot on GateFixture: gate vs screenshot-every-step, training data for the light predictor. `box-raw/` = everything the box wrote (logs, logs2, train_logs, STATUS files). | scripted agent (sim_agent.py), NOT an LLM |
| pi-2026-10-20/ | 2026-10-20 22:45 | First real-model run: direct / full / formula / model deciders × 3 seeds. | Pi + deepseek-v4-flash |
| pi-2026-10-20b/ | 2026-10-20 23:20 | gui-gate 0.2.0 (core toolset, auto-observe, risk levels): direct / full / gate × 3 seeds. | Pi + deepseek-v4-flash |
| osworld-*/ | in progress | OSWorld-Verified subset + OSGuard risk tasks. | Pi + deepseek-v4-flash |

Conventions: screenshots are stripped from Pi event streams (`"data":"<image stripped>"`) to keep the repo small; everything else is raw. Runs that died from the AgentRouter relay error (400 `content[].thinking must be passed back`) are infrastructure failures and are not counted against the gate.
