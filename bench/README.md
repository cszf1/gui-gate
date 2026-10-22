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

Environment differences from the OSWorld VM (read before comparing numbers with published OSWorld scores):
Debian 13 + XFCE at 1440×900 (not Ubuntu GNOME 1920×1080), LibreOffice 25.2, Chromium instead of
Google Chrome, GIMP 3, xfce4-terminal. One compatibility rewrite is applied: LibreOffice ≥ 24 titles
windows with an em dash, so `activate_window` targets in task configs/postconfigs are rewritten
from " - LibreOffice" to " — LibreOffice". Nothing else in any task is changed.

## 0.9.6 options (run_bench.sh)
- `--max-turns N` (or `PI_MAX_TURNS`): turn cap per run. Default **30** keeps runs comparable with every earlier
  result in `results/`; OSWorld's official setting is **100** steps (`--max-turns 100` for leaderboard-comparable runs).
- `PI_TOOLS_096=0`: ablation without the 0.9.6 tools (`run_command`, `check_done`, `set_plan` are switched off in the
  gate through the MCP server env) and without their prompt lines. Default 1: the prompt asks for set_plan first
  (feasibility verdict -> FAIL), run_command for file/terminal work, run_actions for known step sequences, and
  check_done before DONE; Pi's `toolExposure` declares the three tools.
- `PI_APP_HINTS=0`: no per-app keyboard tips. Default 1: `bench/app_hints.py TASK` picks a static block from
  `bench/app_hints.md` (LibreOffice Calc / Writer / Impress, Chrome, VLC, GIMP, terminal/files, multi-app); the tips
  name no gate tool and contain no task-specific answers.
- `PI_OBS_PRUNE=1`: context hygiene (Pi `context` extension `bench/pi_ext/obs_prune.ts`); default 0.
- runs.jsonl records `max_turns`, `tools096`, `app_hints`, `obs_prune`.
