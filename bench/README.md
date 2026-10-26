# bench/: OSWorld + OSGuard tasks on a live desktop, no VM

The box this was built for has no `/dev/kvm`, so the official OSWorld VM cannot run.
Instead OSWorld's **own** pieces run directly on the box's desktop:

- `desktop_env/server/main.py` (OSWorld's in-guest HTTP server) on 127.0.0.1:5000,
- OSWorld's `SetupController` executes each task's `config` against it,
- OSWorld's `DesktopEnv.evaluate` (getters + metrics, unmodified) scores the final state.

`osworld_native.py` glues these together and replaces the VM snapshot revert with a reset:
kill task apps, restore pristine copies of the app profiles and user folders, delete anything
new at the top of `$HOME`. `/home/user` is a symlink to the box user's home.

| file | what |
|---|---|
| `box_setup.sh` | one-time box prep (OSWorld clone at a pinned commit, `/home/user` link, `google-chrome`→chromium and `gnome-terminal`→xfce4-terminal wrappers, LibreOffice profile without the alien-format/tips/recovery dialogs, baseline snapshot, server start) |
| `stubs/` | import stubs for heavy optional evaluator deps (easyocr/torch, librosa, acoustid, borb, pydrive, playwright); any use raises, so a task needing one fails loudly |
| `make_tasks.py` | writes `tasks/osworld/*.json` (unmodified OSWorld tasks), `tasks/osguard/*.json` (re-implemented OSGuard variants) and `tasks/order.txt` |
| `osworld_native.py` | `setup` / `eval` / `reset` for one task |
| `pi_monitor.py` | runs Pi in JSON mode, stops it at `--max-turns` or `--timeout` |
| `run_bench.sh` | the matrix: task × condition (direct / full / gate) × seed, resumable |
| `score.py` | per-run and per-condition tables; `--pack` writes a git-friendly copy (base64 images stripped) |

Results from this bench are from a **custom environment based on OSWorld tasks and evaluators**, not comparable to
the OSWorld leaderboard (even with `--max-turns 100`). Evaluation protocol (research vs product experiment, frozen
manifest, ablation chain, labels, pilot sizes, wording): `docs/eval-protocol.md`.

Environment differences from the OSWorld VM (read before comparing numbers with published OSWorld scores):
Debian 13 + XFCE at 1440×900 (not Ubuntu GNOME 1920×1080), LibreOffice 25.2, Chromium instead of
Google Chrome, GIMP 3, xfce4-terminal. One compatibility rewrite is applied: LibreOffice ≥ 24 titles
windows with an em dash, so `activate_window` targets in task configs/postconfigs are rewritten
from " - LibreOffice" to " — LibreOffice". Nothing else in any task is changed.

## 0.9.6 options (run_bench.sh)
- `--max-turns N` (or `PI_MAX_TURNS`): turn cap per run. Default **30** keeps runs comparable with every earlier
  result in `results/`; OSWorld's official setting is **100** steps (`--max-turns 100` matches that step budget; the
  environment still differs, so the numbers stay custom-environment results).
- `PI_TOOLS_096=0`: ablation without the 0.9.6 tools (`run_command`, `check_done`, `set_plan` are switched off in the
  gate through the MCP server env) and without their prompt lines. Default 1: the prompt asks for set_plan first
  (feasibility verdict -> FAIL), run_command for file/terminal work, run_actions for known step sequences, and
  check_done before DONE; Pi's `toolExposure` declares the three tools.
- `PI_APP_HINTS=0`: no per-app keyboard tips. Default 1: `bench/app_hints.py TASK` picks a static block from
  `bench/app_hints.md` (LibreOffice Calc / Writer / Impress, Chrome, VLC, GIMP, terminal/files, multi-app); the tips
  name no gate tool and contain no task-specific answers.
- `PI_OBS_PRUNE=1`: context hygiene (Pi `context` extension `bench/pi_ext/obs_prune.ts`); default 0.
- runs.jsonl records `max_turns`, `tools096`, `app_hints`, `obs_prune`.

## Value-policy baselines and counterfactual branches (0.9.11)
- Fair baselines (P2): conds `vp_direct vp_light vp_full vp_reason vp_threshold vp_heuristic vp_random vp_legacy
  vp_learned` are `gate8` exactly plus `GUI_GATE_VALUE_POLICY`; only the gate's value choice differs. `--repeats N`
  runs N independent repeats. `python3 bench/fair_compare.py OUT --baseline vp_direct --planned tasks/order.txt
  --md cmp.md` reports planned (infra/missing = failure) and valid denominators, Wilson CIs, task-level paired
  bootstrap CIs, McNemar and permutation p, and the budget curve (success vs wall time / tokens / turns).
  Freeze the task list, exclusion reasons and retry rules before a run; tune on a dev split, report on a held-out one.
- Counterfactual branches (P1): `bench/counterfactual/` (README there): checkpoints, seven treatments, hash-verified
  restore, task-family split, offline dry run through the real gate. Not run on a desktop or with a model yet.
- Value model v2 offline evaluation: `python3 bench/offline_v2.py RESULTS_DIR -o OUT --bin target/release/gui-gate`
  (exclusion audit, GUI vs command calibration, failure prediction at fixed budgets, costs from gate + Pi logs; read
  only on RESULTS_DIR; never claims that an intervention avoided a failure). `python3 bench/online_plan.py
  TASK_LIST -o plan.json` writes the frozen legacy-vs-v2 online plan and runs nothing. docs/value-model-v2.md.
- `weights/costs-osworld-verified-v099.json`: option costs measured from the v0.9.9 run (`gui-gate costs`), usable as
  `VP_COSTS` / `GUI_GATE_COSTS`. `weights/value-observational-2026-10-25.json`: an observational treatment model for
  analysis only (not loaded by default).

## Protocol tooling (docs/eval-protocol.md)
- `manifest.py`: frozen task manifest (family split dev/test, exclusion reasons, retry rules, pre-registered margin,
  hash). `run_bench.sh --manifest M --manifest-hash H` refuses a test-split list without the exact hash;
  `bench/tasks/manifest.json` (default) marks the 34 development tasks as dev. `manifest.py denominators` reports
  planned and valid denominators with infra failures per policy and kind.
- `schedule.py` / `run_bench.sh --interleave abba`: policies interleaved task by task (ABBA / Williams order);
  repeats are recorded as independent repeats with `api_seed_accepted`.
- `cost_accounting.py`: per run Pi turns vs real GUI actions, reviewer and role calls, screenshots, tokens, cost,
  wall time incl. failures/timeouts (merged into `score.py` rows); time comparisons over all runs.
- `stats.noninferiority` / `fair_compare.py --manifest M` (or `--margin`): paired one-sided non-inferiority.
- `leakage_lint.py`: answer files not reachable from the agent's run dirs; tuned artefacts free of test ids/answers.
- `counterfactual/layouts.py`: frozen unseen-layout test set; `dryrun.py --unseen-layouts N`; `analyze.py --plot`.
- `human_pilot.py`: real-user pilot analysis (threshold vs value asking, counterbalanced `assign`; `from-logs` reads the desktop client's `logs/human.jsonl` (`gui-pi-human/1`) and the gate's `person_burden` events).
- `context_ablation.py` + conds `ctx_full ctx_current ctx_new`: context ablation from gate + Pi logs.
