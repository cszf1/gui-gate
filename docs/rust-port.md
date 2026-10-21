# 0.6.0: the gate in Rust

The gate was rewritten from Python (~3,700 lines) to Rust (`src/`), behaviour for behaviour. From now on only
the Rust version is maintained. Cua Driver stays a separate, official package (`cua-driver>=0.37.0,<1`).

## How the port was checked

* The 61 MCP-over-stdio tests in `tests/` (fake driver, real JSON-RPC) now run against the Rust binary
  (`GUI_GATE_BIN`). They were written for the Python gate, so they act as a conformance suite: all pass.
* The module-level Python tests (side-effect checks, authorization, risk rules, reviewer parsing, nearest-row
  re-binding, version handling) were ported to `cargo test` (23 tests). All pass.
* `gui-gate compare` gives byte-identical JSON to the old Python `gui-gate-compare` on a real run's logs.
* A wheel built with maturin, installed into a fresh venv, pulled `cua-driver 0.37.0` from PyPI by itself;
  `gui-gate doctor` found the bundled driver (66 tools) and the server answered `initialize` / `tools/list`
  (19 core tools) through the real driver. No desktop task was run.

## What got better

Measured locally with the fake driver (best of 3, 200 actions):

| | Python 0.5 | Rust 0.6 |
|---|---|---|
| start + initialize | 97 ms | 20 ms |
| memory (RSS) | 12.4 MB | 3.5 MB |
| one action round trip | 0.21 ms | 0.15 ms |
| runtime needed | Python 3.10+ | none (one ~4 MB binary) |

Per-action time is dominated by the driver and the model, not the gate, so task time will not change much;
start-up, memory and "nothing to install" are the real gains.

Other changes made during the port:

* the child-driver reader never takes the gate's lock, so a gated call waiting on the driver can never block
  the driver's replies (the Python version relied on a re-entrant lock for this);
* desktop approval dialogs use zenity / kdialog when present, GTK via python3 only as a fallback;
* `gui-gate train` / `gui-gate compare` are subcommands of the same binary.

## Predictor on real data

`gui-gate train results/osworld-2026-10-21/gate` (1,948 executed-direct actions, 118 sessions, 35 held out):

| | AUC | ECE | Brier |
|---|---|---|---|
| hand-set prior (default) | 0.50 | 0.096 | 0.160 |
| fitted | 0.66 | 0.037 | 0.126 |
| fitted + Platt | 0.66 | 0.051 | 0.126 |

The hand-set prior does not rank actions better than chance on real tasks. The fitted weights are in
`weights/osworld-2026-10-21.json` (opt-in with `--weights`); they are not the default yet because labels are
proxies (driver error / stale token / immediate repeat) from mixed gate versions, and no benchmark has been run
with them.
