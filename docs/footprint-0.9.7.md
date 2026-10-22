# gui-gate 0.9.7 – 0.9.9 footprint: everything it writes, starts, changes or downloads, and how it is cleaned

Audit of 2026-10-22, file by file, for installing on a real personal machine. Every file in `src/` (all `.rs` and
`skills.md`), `scripts/install.sh`, `scripts/install.ps1`, the new `scripts/fetch_driver.py` / `scripts/smoke_session.py`,
`.github/workflows/release.yml` and the README install section were read; side-effect call sites were then
cross-checked mechanically (every `fs::write/create/remove/rename/copy`, `OpenOptions`, `create_dir*`, `temp_dir`,
`Command::new/spawn/output/status`, `TcpStream`/`UnixStream`, `ureq`, `set_var`, registry/PowerShell calls).
"Gate dirs" below means the paths gui-gate owns: the binary, `<bin>/../lib/gui-gate/` (Windows
`%LOCALAPPDATA%\Programs\gui-gate\`), `~/.config/gui-gate/`, `~/.gui-gate/`, `~/.cache/gui-gate/` and
`$TMPDIR/gui-gate-*`.

## 1. What ends up on the machine

| What | Where (default) | Created by | Removed by `gui-gate uninstall` |
|---|---|---|---|
| gate binary | `~/.local/bin/gui-gate` · Win `%LOCALAPPDATA%\Programs\gui-gate\bin\gui-gate.exe` | installer | yes (Windows: renamed to `gui-gate.exe.old`, a detached hidden `cmd` deletes it and the empty dirs ~3 s later) |
| bundled Cua Driver 0.38.0 (official, unmodified, MIT) | Linux `~/.local/lib/gui-gate/cua-driver/` (cua-driver, cua-cursor-theme, wayland-helper/, LICENSE, THIRD_PARTY_NOTICES.md, VERSION) · Win `%LOCALAPPDATA%\Programs\gui-gate\cua-driver\` (cua-driver.exe, cua-driver-uia.exe, cua-cursor-theme.exe, LICENSE, ...) · macOS `~/.local/lib/gui-gate/cua-driver/` (LICENSE, VERSION only) | installer | yes |
| macOS: `CuaDriver.app` (Cua's signed app, byte-for-byte, so Accessibility / Screen Recording work as with Cua's installer) | `/Applications/CuaDriver.app` (`~/Applications/` when /Applications is not writable) | installer, **only if no CuaDriver.app is there yet** | yes, only when the manifest says gui-gate installed it: every process running from the app is stopped and verified gone first (`cua-driver stop` with Cua telemetry off, then SIGTERM, then SIGKILL; uninstall fails and keeps the app if one survives), `lsregister -u`, delete, `tccutil reset Accessibility/ScreenCapture com.trycua.driver`; plus `~/.cua-driver`, `~/Library/Caches/cua-driver`, `~/Library/Application Support/Cua Driver` if they did not exist before the install; `~/Library/Caches` and `~/Library` themselves are recorded as `created_dir` only when missing at install time (a throwaway HOME) and removed once empty |
| install manifest | `<lib>/install-manifest` (`created_dir=`, `created_file=`, `app=`, `owned_dir=` lines) | installer | yes (it is what uninstall reads) |
| shell PATH line | `# gui-gate` + `export PATH="<bin>:$PATH"` in exactly one file: `~/.zshrc` (zsh) / `~/.bashrc` (Linux bash) / macOS bash: the first existing of `~/.bash_profile`, `~/.bash_login`, `~/.profile` (what a login shell reads), else `~/.bash_profile` / `~/.profile` (other shells); only if `<bin>` was not on PATH; `--no-modify-path` skips it. A file that did not exist is created with only these two lines and recorded as `created_file=` (0.9.9) | install.sh | yes, every `# gui-gate` block (the blank line before it too); a `created_file` is deleted when nothing but our lines was in it |
| Windows user Path entry | `HKCU\Environment\Path` += `%LOCALAPPDATA%\Programs\gui-gate\bin` | install.ps1 | yes |
| reviewer settings | `~/.config/gui-gate/reviewer.env` (600), `reviewer.key` (600; Windows ACL owner-only) | installer / user | yes (`--keep-config` keeps) |
| role key files, experience memory | anything under `~/.config/gui-gate/` (e.g. `memory.jsonl` from `gui-gate memory add`) | user | yes (`--keep-config` keeps) |
| session logs (JSONL, contain task text, commands, window titles) | `~/.gui-gate/logs/<time>-<id>.jsonl` (`--log PATH|none`, `GUI_GATE_LOG_DIR`) | gate at runtime | yes (`--keep-logs` keeps). Pruned at every gate start: > 30 days (`GUI_GATE_LOG_KEEP_DAYS`) and oldest beyond 200 MB (`GUI_GATE_LOG_MAX_MB`) |
| driver update-check cache | `~/.cache/gui-gate/driver-latest.json` — **only with an external driver**; never with the bundled one | gate | yes |
| MCP client entries | `~/.claude.json` (via `claude mcp add`), `~/.pi/agent/mcp.json` (via `pi mcp add`), `~/.cursor/mcp.json`, `~/.codex/config.toml` — only for clients you name (`--client`, `gui-gate setup --client`) | `gui-gate setup` | yes: only entries whose command is gui-gate; other entries untouched. No `.bak` is written (atomic temp + rename); an old gui-gate-made `.bak` that differs only by our entry is deleted by `cleanup` |
| dirs the installer had to create | e.g. `~/.local/bin`, `~/.local/lib`, `~/.local`, `~/.config`, `~/.cache` (fresh accounts only) | installer (recorded in the manifest only if missing before) | yes, if empty afterwards |
| temp files | `$TMPDIR/gui-gate-job-<pid>-<n>.log` (unix: unlinked right after the job starts, only the open handle remains), `$TMPDIR/gui-gate-lo-<pid>/` (throwaway LibreOffice profile run_command suggests), Windows installer `%TEMP%\gui-gate-<guid>` (deleted in `finally`), install.sh `mktemp -d` (trap `rm -rf`) | gate / installer | gate exit deletes its own; next gate start and `cleanup` delete those of dead gates; `uninstall` deletes all |
| processes | `gui-gate` (started by your MCP client), its child `cua-driver mcp`, agent commands (`run_command`, jobs) in their own process groups, desktop dialogs (zenity/kdialog/python3 GTK) while asking a person | MCP client / gate | stdin EOF or SIGTERM/SIGINT/SIGHUP: jobs and still-running commands killed (whole groups), driver closed (it exits on EOF itself). macOS: `cua-driver mcp` proxies to the CuaDriver.app daemon (Cua's design for TCC); since 0.9.8 the gate (and `doctor`) stop that daemon at exit when it was not already running before and no other gui-gate uses it; uninstall stops it too |

Nothing else: no services, scheduled tasks, LaunchAgents, systemd units, registry keys besides the user Path,
browser profile changes, or files outside the paths above. Cua's own installer is **not** run any more, so its
side effects (Windows scheduled task `cua-driver-serve` running elevated at logon, directory junctions,
`~/.cua-driver/packages`, `# Added by cua-driver-rs installer` PATH line, telemetry install event) do not happen.

### Network

| Call | When | Off switch |
|---|---|---|
| GitHub release download (archive + SHA256SUMS), checksum-verified | install / `update --self` | — |
| your reviewer model's API (risky steps; check at install with `check-reviewer`) | when a step needs review | required by design |
| optional role models (`GUI_GATE_ROLE_*`) | only if configured | unset them |
| cua.ai `_install-rust.sh` / PyPI: latest driver version, ≤ 1/day, 3 s | **only with an external driver** | `GUI_GATE_NO_UPDATE_CHECK=1` |
| GitHub API latest gui-gate release | only when you run `gui-gate update` | — |
| `127.0.0.1:<GUI_GATE_CDP_PORT=9222>` | `inspect kind=page/env` (0.9.7, default off): connect only if a Chrome debugging port is already open; never launches or configures Chrome; only localhost DevTools URLs are followed; one fixed read-only `Runtime.evaluate` (password fields masked) | `GUI_GATE_INSPECT=0` (default) |
| Cua Driver telemetry / update check | bundled driver: **off** (gate sets `CUA_DRIVER_RS_TELEMETRY_ENABLED=0 CUA_TELEMETRY_ENABLED=0 DO_NOT_TRACK=1 CUA_DRIVER_RS_UPDATE_CHECK=0`; verified: it then writes nothing to `$HOME`) | `GUI_GATE_DRIVER_TELEMETRY=1` restores Cua's defaults |

## 2. Per-file audit

| File | Side effects (outside memory) | How cleaned / bounded |
|---|---|---|
| `src/main.rs` | dispatches subcommands; server mode calls `cleanup::startup()`, reads `--task-file`/weights | — |
| `src/server.rs` | spawns the driver (via rpc.rs), respawns it if it dies; threads per tool call; writes the session log; runs `run_command`/jobs; **new `shutdown()`** on stdin EOF kills jobs and running commands, removes `gui-gate-lo-<pid>`, closes the driver; installs signal handlers; reads `/proc/<pid>/{comm,cmdline}` (Linux, read-only) | shutdown + proc.rs handlers |
| `src/proc.rs` (new) | process-group registry; `kill(2)`/`taskkill /T /F`; SIGTERM/SIGINT/SIGHUP handler (kill registered groups, TERM the driver, re-raise); `run_argv` (direct exec with timeout, no shell; Windows `CREATE_NO_WINDOW`) | — |
| `src/rpc.rs` | spawns `cua-driver mcp` with piped stdio (stderr to null unless `GUI_GATE_DEBUG`); bundled driver gets telemetry/update-check off; Windows no console window | `close()`: stdin closed, wait 3 s, kill |
| `src/command.rs` | `run_command`: `bash -c` in its own process group, env PAGER/TERM etc., timeout kill of the whole group; scans cwd for changed files (read-only); **Windows: tool not offered** (risk rules are bash-only) | group registered while running, killed on timeout / gate exit; a program it backgrounds on purpose (`app &`) is left running |
| `src/jobs.rs` | `run_command mode=start` (GUI_GATE_JOBS, default off): background bash in own group; log in `$TMPDIR/gui-gate-job-<pid>-<n>.log` | unix: log unlinked at start; stop/poll/Drop/shutdown kill the group; Windows: not available |
| `src/cleanup.rs` | `cleanup`: removes old gui-gate copies, `pip uninstall` of the 0.2-0.5 Python package (and the pip Cua Driver only it pulled in), duplicate/stale `# gui-gate` profile lines and Windows Path entries, repoints stale MCP entries, removes our `.bak`, old cache files, our temp files, old logs. `uninstall`: everything in section 1; `--with-driver` runs Cua's own uninstaller (`uninstall.sh --purge [--reset-permissions]` / `uninstall.ps1` with FORCE+PURGE) and removes the PATH line / Path entry Cua's installer added | only gui-gate's own things; files needing admin rights are reported, not touched |
| `src/install.rs` | finds the driver (bundled first; `GUI_GATE_DRIVER=PATH|external`), runs `cua-driver --version` (external only), `setup`: writes MCP configs atomically (`<file>.gui-gate-tmp` → rename) or runs `claude/pi mcp add`; `doctor` starts the driver once | temp file removed on failure |
| `src/updates.rs` | bundled driver: no network; external: daily version check (cache `~/.cache/gui-gate`), `update` runs Cua's installer or pip only when asked; `update --self` re-runs gui-gate's installer | see network table |
| `src/reviewer.rs` | reads `~/.config/gui-gate/reviewer.env` / key file; HTTPS to the configured reviewer (honours `HTTPS_PROXY`) | — |
| `src/roles.rs` | optional role models (HTTPS); read-only probes via `command::run` restricted to an allow-list. **Hardened**: `sort -o`, `uniq IN OUT`, `xxd -r`/`xxd IN OUT`, `tree -o`, `file -C`, writing `exiftool`, `date -s`, `hostname NAME`, `less/more` now refused; Python probe filter also catches `.open('w')`, `mode='w'`, `makedirs`, `symlink`, `truncate`, `os.open`, `ctypes`, `importlib`, `getattr`, `.copy*(`, `utime`, `.kill(`, `signal.` | refused probes never run |
| `src/inspect.rs` | reads files the agent names (never the gate's own key/env files — **fixed for Windows paths**); PDF/office/OCR helpers now exec `pdftotext`/`python3 -c`/`tesseract` directly with a timeout (no bash, works on every OS); `kind=page` connects to an already-open localhost DevTools port only (**now refuses non-local ws URLs, connect/write timeouts**); `kind=env` reads `/sys`, `/dev`, `/proc` (read-only) | test temp dir now removed |
| `src/memory.rs` | `gui-gate memory add` appends to `~/.config/gui-gate/memory.jsonl` (or `--out`) | uninstall removes `~/.config/gui-gate` |
| `src/log.rs` | creates `~/.gui-gate/logs/`, appends one JSONL per session | pruned at start (age + size cap), uninstall |
| `src/human.rs` | zenity / kdialog / `python3` GTK dialog while asking a person (killed on timeout) | child killed on timeout |
| `src/atspi.rs` | connects to the session D-Bus / AT-SPI bus (unix socket) and calls only `Get*` methods (read-only) | — |
| `src/done.rs`, `src/plancheck.rs` | read-only stat/walks of paths the task names (bounded budgets); `/proc` scan for `app_running` (Linux). **ctime now via proc::changed_at (compiles on Windows)** | — |
| `src/sidefx.rs` | read-only filesystem checks; runs `find` (without -exec/-delete, 3 s timeout) to preview what a `find -exec` would touch | child killed on timeout |
| `src/tools.rs` | `train` writes the weights file you name (`--out`); `compare/replay` read logs | user-named output only |
| `src/shot.rs`, `src/decide.rs`, `src/features.rs`, `src/authz.rs`, `src/state.rs`, `src/batch.rs`, `src/loopbreak.rs`, `src/v097.rs`, `src/util.rs` | none (pure logic; test-only temp files are removed by their tests or live under the test's own temp dir) | — |
| `src/skills.md` | none (text compiled into the binary) | — |
| `scripts/install.sh` | downloads + verifies one archive; installs gate + bundled driver (macOS app with `ditto`, `codesign --verify --deep --strict`, `lsregister -f`, rollback on failure; an existing CuaDriver.app not installed by gui-gate is left alone); PATH line; reviewer settings; runs `gui-gate cleanup`, `check-reviewer` (one reviewer API call), `doctor` (starts the driver once); writes the manifest | `trap rm -rf` for its temp dir |
| `scripts/install.ps1` | same on Windows (no Cua installer, no scheduled task, no junctions); user Path; reviewer key ACL via `icacls` | `finally` removes `%TEMP%\gui-gate-<guid>` |
| `scripts/fetch_driver.py` | CI only: downloads the pinned Cua assets, verifies sha256 + Cua SHA256SUMS + Sigstore (cosign) | its temp dir is removed |
| `scripts/smoke_session.py` | CI / manual: one MCP session, asserts no gui-gate / cua-driver / job process is left | — |
| `.github/workflows/release.yml` | CI: builds, packs, releases; install-smoke on 4 OS incl. upgrade-from-0.8.3 and a throwaway-HOME zero-residue diff after uninstall | runs on GitHub runners only |

## 3. Issues found and fixed in this audit

1. Windows build broke: `std::os::unix` used unconditionally in jobs.rs, command.rs, done.rs, plancheck.rs → cfg-gated through proc.rs; `cargo check` clean for x86_64-pc-windows-msvc, x86_64-apple-darwin, aarch64-apple-darwin (+ CI check on windows-gnu).
2. Background jobs survived the gate (own process group, never killed) → killed on stdin EOF, SIGTERM/SIGINT/SIGHUP and Drop; in-flight `run_command` groups too. Tests: `jobs::no_orphans_no_logs`, `proc::kill_all_takes_grandchildren`, `JobTests.test_gate_exit_kills_jobs_and_leaves_no_temp` (EOF and SIGTERM).
3. Job logs stayed in `/tmp` forever → unlinked at start (unix), swept by gate start / cleanup / uninstall.
4. run_command's description told the agent to use `-env:UserInstallation=file:///tmp/lo_cmd` → a LibreOffice profile left in `/tmp` → now `$TMPDIR/gui-gate-lo-<pid>`, removed at gate exit.
5. `uninstall` left `~/.config/gui-gate` (reviewer key!) → removed by default, `--keep-config` to keep.
6. `uninstall` kept the PATH line whenever `~/.local/bin` held anything else → our marked line is always removed.
7. Installer-created dirs (`~/.local/bin`, `~/.config`, Windows `Programs\gui-gate\bin` and its parent) stayed → recorded in a manifest, removed when empty.
8. Logs grew without bound between installs → pruned at every start (30 days / 200 MB).
9. `inspect` secret-file guard compared `/`-joined strings → on Windows the reviewer key was readable through `inspect kind=file` → component-wise canonical comparison, also covers `GUI_GATE_REVIEWER_ENV`.
10. `inspect` PDF/office/OCR and the env fact sheet went through `bash` heredocs (broken on Windows) → direct exec with timeout.
11. DevTools reader followed any ws URL from `/json/list` and had no connect timeout → localhost only, timeouts.
12. Read-only probe allow-list let `sort -o`, `uniq IN OUT`, `xxd -r`, `tree -o`, `file -C`, `exiftool -tag=…`, `date -s`, `hostname NAME`, pagers through; Python filter missed `.open('w')`/`mode='w'` and friends → refused.
13. Windows uninstall flashed a console and left `Programs\gui-gate\bin` → hidden detached cleanup that also removes the empty dirs.
14. The installer ran Cua's installer (Windows: elevated scheduled task at logon; all: telemetry install event, `~/.cua-driver`, an extra PATH line, network at runtime) → replaced by the bundled driver (below).
15. Unused imports / a stray doc comment produced warnings → build is warning-free on Linux, Windows and macOS targets.

## 4. The bundled Cua Driver (since 0.9.7)

* `scripts/cua-driver.lock` pins Cua Driver 0.38.0 (`cua-driver-rs-v0.38.0`; 0.9.7 shipped 0.37.0) and the sha256 of each platform asset.
  At release time `fetch_driver.py` checks the pin, Cua's SHA256SUMS and the Sigstore bundle issued to
  `trycua/cua/.github/workflows/cd-rust-cua-driver.yml@refs/tags/cua-driver-rs-v0.38.0`, then packs it into the
  gui-gate archive; on macOS CI also runs `codesign --verify --deep --strict` on the packed app.
* Not embedded in the gate binary and never extracted at runtime: installed once, next to the gate, by the installer.
* `find_driver`: `GUI_GATE_DRIVER=PATH` > bundled > (or `GUI_GATE_DRIVER=external`) pip / PATH / Cua's locations.
  Installing with `--skip-driver` (`GUI_GATE_SKIP_DRIVER=1`) leaves the bundled driver out.
* Updates: the bundled driver changes only with a gui-gate release. `gui-gate update` reports whether a newer
  gui-gate exists; `gui-gate update --self` installs it. No background driver check with the bundled driver.
* A Cua Driver installed with Cua's own installer is never touched (install notes it; uninstall leaves it unless
  `--with-driver`). On macOS an existing `CuaDriver.app` that gui-gate did not install is used as is.
* License: Cua's MIT LICENSE and THIRD_PARTY_NOTICES ship in the archive (`cua-driver/`) and in
  `third_party/cua-driver/`; see NOTICE.

## 5. Removing Cua Driver installed by Cua's own installer (not gui-gate's)

`gui-gate uninstall --with-driver` (opt-in) runs Cua's documented uninstaller and then removes the PATH line / user
Path entry Cua's installer added (its uninstaller leaves them). Manually:

* macOS / Linux: `bash -c "$(curl -fsSL https://cua.ai/driver/uninstall.sh)" -- --purge` (macOS: add
  `--reset-permissions` to revoke Accessibility / Screen Recording), then delete the
  `# Added by cua-driver-rs installer` line and the `export PATH=...` after it from `~/.bashrc` / `~/.zshrc`.
* Windows: `$env:CUA_DRIVER_RS_UNINSTALL_PURGE='1'; irm https://cua.ai/driver/uninstall.ps1 | iex` (removes the
  `cua-driver-serve` scheduled task, may ask for elevation), then remove
  `%LOCALAPPDATA%\Programs\Cua\cua-driver\bin` from the user Path.

## 6. Only verifiable on the real machine

* macOS: the permission prompts and that the TCC grants stick to `/Applications/CuaDriver.app` (CI has no GUI
  session); whether the CuaDriver daemon keeps running after the client closes; an existing CuaDriver.app is reused.
* Windows: GUI actions without Cua's elevated `cua-driver serve` scheduled task (Cua's docs say `cua-driver mcp`
  owns its runtime on Windows and Linux; CI only checks that the driver starts and lists its tools); the hidden
  delayed deletion of `gui-gate.exe.old`; SmartScreen / antivirus reactions to the two unsigned exes.
* Linux: GNOME-on-Wayland users only get window geometry / the agent cursor after running
  `~/.local/lib/gui-gate/cua-driver/wayland-helper/install.sh` (not run by the installer; it writes a GNOME Shell
  extension into `~/.local/share/gnome-shell/extensions/winrects@cua` and changes a gsettings key, which uninstall
  does not undo).
* Processes your MCP client keeps alive: uninstall reports a still-running gui-gate; it ends when the client closes it.

## 7. 0.9.8: Cua Driver 0.38.0

* `scripts/cua-driver.lock` moved to `cua-driver-rs-v0.38.0` (GitHub release of 2026-10-21; cua.ai's installer and
  PyPI still served 0.37.0 that day). All four platform assets exist with Sigstore bundles; the Linux arm64 asset was
  verified here (sha256 pin + SHA256SUMS + cosign), the others are verified by CI before packing.
* Tool-list diff 0.37.0 → 0.38.0 (Linux arm64 binaries, `cua-driver mcp` → `tools/list`): 66 → 66 tools, none added or
  removed, every input schema identical; only `get_accessibility_tree`'s description text changed. All 32 driver tools
  the gate refers to (incl. REQUIRED_DRIVER_TOOLS get_window_state, list_windows, click, type_text, press_key) are
  unchanged. With telemetry and update check off, 0.38.0 also writes nothing to `$HOME`.
* Release notes: 1 feature (cua-sdk / proto contract) and 25 fixes, no breaking change listed; behaviour changes the
  gate may notice: Windows list_windows omits the driver's own overlay windows, Windows tree completeness reported,
  foreground Document text insertion verified, DOM click effects classified with readback, native Wayland windows in
  desktop snapshots, unknown tools reported accurately. Output formats were not re-checked against a live desktop.
* macOS: the CuaDriver.app daemon a session (or `doctor`) started is stopped at gate exit.

## 8. 0.9.9: zero residue on macOS (release CI of v0.9.8, install-smoke macos-14)

* **Empty `~/.bash_profile` left in a throwaway HOME.** Two bugs together: (a) on macOS bash the installer appended to
  `~/.bash_profile` even when that file did not exist, creating it without recording it; (b) right after, the
  installer's `gui-gate cleanup` compared the PATH line's dir (`/var/folders/.../.local/bin`, as written) with the
  running binary's canonical dir (`/private/var/folders/...`: `/var` is a symlink on macOS) and dropped gui-gate's own
  fresh line as "stale" - so PATH was not set at all and uninstall found no line, leaving the empty file. Fixed: the
  cleanup compares dirs after resolving symlinks; the PATH line goes to exactly one file (table above); a profile file
  the installer creates is recorded as `created_file=` and deleted by uninstall when only our lines were in it. Why
  `~/.bash_profile` and not the existing `~/.bashrc`: macOS Terminal starts login shells, which read the first of
  `~/.bash_profile`, `~/.bash_login`, `~/.profile` and never `~/.bashrc`; a line only in `~/.bashrc` would not work.
* **`~/Library`, `~/Library/Caches` left in a throwaway HOME.** The CuaDriver.app daemon (`cua-driver serve`) puts its
  socket and pid file in `$HOME/Library/Caches/cua-driver/` (cua-driver 0.38.0 `cua-driver-core/src/daemon.rs`
  `socket_path_for_namespace`, `serve.rs` `default_pid_file_path`; hard-coded, no env var: `--socket` moves only the
  socket, the pid file stays; `CUA_DRIVER_RS_HOME` covers only `~/.cua-driver` state). Uninstall already removed
  `cua-driver/` (owned_dir); now the installer also records `~/Library/Caches` / `~/Library` as `created_dir` when
  they do not exist at install time, so uninstall removes them once empty. On a real Mac both always exist: no change.
* **A CuaDriver.app process still running after uninstall** (`/Applications/CuaDriver.app/Contents/MacOS/cua-driver`,
  no arguments). It was Cua's detached telemetry worker: `cua-driver stop` is a "finite command" that Cua wraps in a
  child and follows with a worker (the same binary, no arguments, configured by env) unless telemetry is off -
  uninstall (and the gate's exit-time stop) ran `stop` without the telemetry-off env the gate uses for `mcp`, so the
  worker started, re-created `~/.cua-driver` and outlived the uninstall. Fixed: every `cua-driver stop` gui-gate runs
  carries the telemetry/update-check-off env; uninstall stops every process running from the app it installed
  (`stop`, wait 3 s, SIGTERM, wait 5 s, SIGKILL, verify), refuses to delete the app if one survives, and checks again
  after deleting it. There is no LaunchAgent/launchd job to remove: the daemon is started by `open -n -g ... --args
  serve` (LaunchServices) and Cua's autostart is opt-in and not implemented for macOS in 0.38.0.
* **CI**: the `! grep` / `! pgrep` checks never failed a step (bash's `set -e` ignores a negated command unless it is
  the last one) - all are now explicit `if ...; then exit 1; fi`; leftover processes are checked right after
  uninstall and again 3 s later; the throwaway-HOME run also asserts the PATH line is in exactly one profile after
  install and that a created profile file is in the manifest.
