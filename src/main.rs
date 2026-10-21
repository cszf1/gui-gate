//! gui-gate: a pre-action value gate for GUI agents, served as one MCP server in front of Cua Driver.
//!
//!   gui-gate                 serve MCP on stdio (what MCP clients run)
//!   gui-gate setup           add it to an MCP client and check the machine
//!   gui-gate doctor          check the machine end to end
//!   gui-gate update          look for / install a newer Cua Driver and gui-gate (only when you run it)
//!   gui-gate train | compare offline tools over the gate's logs
#![allow(dead_code, unused_assignments)]
mod authz;
mod batch;
mod decide;
mod features;
mod human;
mod install;
mod log;
mod reviewer;
mod rpc;
mod server;
mod sidefx;
mod state;
mod tools;
mod updates;
mod util;

use decide::GateConfig;
use util::*;

const HELP: &str = "usage: gui-gate [setup|doctor|update|train|compare|replay] [options]

Cua Driver behind a pre-action value gate (MCP stdio server).

options:
  --policy {gate,direct,light,full}   gate (default) or a baseline
  --lambda X                          cost weight (higher = fewer observations)
  --collect                           shadow-label every action for training
  --no-risk-confirm                   do not stop risky actions
  --risk-mode {auto,reviewer,human}   auto: high risk -> person, medium -> the gate's reviewer
                                      (reviewer = GUI_GATE_REVIEWER_URL/_MODEL/_KEY, the only LLM in the gate;
                                      the acting agent is never asked to approve its own step)
  --human {auto,elicit,desktop,chat,deny}  how to reach a person
  --human-timeout S                   seconds to wait for a person (default 120)
  --delivery {auto,background,foreground}  input delivery
  --toolset {core,all}                core (default): only the tools a GUI task needs
  --no-auto-observe                   do not return the fresh window tree after each action
  --task TEXT | --task-file PATH      the user's request, verbatim (or GUI_GATE_TASK / GUI_GATE_TASK_FILE)
  --mode {both,single}                both (default): single-step tools + run_actions; single: one action per call
  --weights PATH|prior_v1             trained weights JSON (default: built-in osworld_v2)
  --log PATH|none                     JSONL log (default ~/.gui-gate/logs/<time>.jsonl)
  --driver PATH                       path to cua-driver (default: the bundled one)
  --driver-arg ARG                    extra args for `cua-driver mcp` (repeatable)
  --version";

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if let Some(cmd) = argv.first() {
        let rest = &argv[1..];
        let code = match cmd.as_str() {
            "setup" => Some(install::setup_main(rest)),
            "doctor" => Some(install::doctor_main(rest)),
            "update" => Some(updates::update_main(rest)),
            "train" => Some(tools::train_main(rest)),
            "compare" => Some(tools::compare_main(rest)),
            "replay" => Some(tools::replay_main(rest)),
            _ => None,
        };
        if let Some(c) = code {
            std::process::exit(c);
        }
    }
    let mut cfg = GateConfig::from_env();
    let (mut mode, mut task_file, mut weights, mut log, mut driver, mut child) = (None, None, None, None, None, None);
    let mut driver_args: Vec<String> = vec![];
    let mut i = 0;
    let choices = |name: &str, v: &str, ok: &[&str]| {
        if !ok.contains(&v) {
            eprintln!("gui-gate: argument {name}: invalid choice: '{v}' (choose from {})", ok.join(", "));
            std::process::exit(2);
        }
    };
    while i < argv.len() {
        let (key, inline) = match argv[i].split_once('=') {
            Some((k, v)) if argv[i].starts_with("--") => (k.to_string(), Some(v.to_string())),
            _ => (argv[i].clone(), None),
        };
        let mut val = || -> String {
            if let Some(v) = inline.clone() {
                return v;
            }
            i += 1;
            argv.get(i).cloned().unwrap_or_else(|| {
                eprintln!("gui-gate: argument {key}: expected one argument");
                std::process::exit(2);
            })
        };
        match key.as_str() {
            "-h" | "--help" => { println!("{HELP}"); return; }
            "--version" => { println!("{}", server::VERSION); return; }
            "--policy" => { let v = val(); choices("--policy", &v, &["gate", "direct", "light", "full"]); cfg.policy = v; }
            "--lambda" => { cfg.lam = val().parse().unwrap_or(cfg.lam); }
            "--collect" => cfg.collect = true,
            "--no-risk-confirm" => cfg.risk_confirm = false,
            "--decider" => { let _ = val(); } // removed in 0.8 (the agent no longer settles anything); accepted for old configs
            "--risk-mode" => {
                let v = val();
                choices("--risk-mode", &v, &["auto", "reviewer", "model", "human"]);
                cfg.risk_mode = if v == "model" { "reviewer".into() } else { v };
            }
            "--human" => { let v = val(); choices("--human", &v, &["auto", "elicit", "desktop", "chat", "deny"]); cfg.human_channel = v; }
            "--human-timeout" => { cfg.human_timeout = val().parse().unwrap_or(cfg.human_timeout); }
            "--delivery" => { let v = val(); choices("--delivery", &v, &["auto", "background", "foreground"]); cfg.delivery = v; }
            "--toolset" => { let v = val(); choices("--toolset", &v, &["core", "all"]); cfg.toolset = v; }
            "--no-auto-observe" => cfg.auto_observe = false,
            "--task" => cfg.task = val(),
            "--task-file" => task_file = Some(val()),
            "--mode" => { let v = val(); choices("--mode", &v, &["both", "single", "batch"]); mode = Some(v); }
            "--batch" => { let v = val(); choices("--batch", &v, &["auto", "actions", "off"]); cfg.batch = v; }
            "--weights" => weights = Some(val()),
            "--log" => log = Some(val()),
            "--driver" => driver = Some(val()),
            "--driver-arg" => driver_args.push(val()),
            "--child" => child = Some(val()), // test hook: full command of a fake MCP server
            other => {
                eprintln!("gui-gate: unrecognized argument: {other}\n{HELP}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    if mode.as_deref() == Some("single") {
        cfg.batch = "off".into();
    }
    if cfg.task.is_empty() {
        if let Some(tf) = task_file {
            if let Ok(t) = std::fs::read_to_string(tf) {
                cfg.task = t.trim().to_string();
            }
        }
    }
    if weights.as_deref() == Some("prior_v1") {
        cfg.weights = decide::prior_v1_weights();
        cfg.calib = None;
        cfg.model_name = "prior_v1".into();
    } else if let Some(w) = weights {
        if let Err(e) = cfg.load_weights(&w) {
            log_err(&format!("cannot load weights {w}: {e}"));
        }
    }
    let cmd: Vec<String> = match child {
        Some(c) => c.split_whitespace().map(String::from).collect(),
        None => {
            let Some(drv) = driver.or_else(install::find_driver) else {
                log_err("Cua Driver was not found. Install it with gui-gate's installer (it installs the latest Cua Driver too): curl -fsSL https://github.com/cszf1/gui-gate/releases/latest/download/install.sh | bash  (Windows: irm https://github.com/cszf1/gui-gate/releases/latest/download/install.ps1 | iex), then run `gui-gate doctor`.");
                std::process::exit(2);
            };
            let mut c = vec![drv, "mcp".into()];
            c.extend(driver_args);
            c
        }
    };
    let log_path = match log.as_deref().map(|l| l.to_lowercase()) {
        Some(l) if l == "none" => None,
        _ => Some(log.or_else(|| env("GUI_GATE_LOG")).unwrap_or_else(|| "auto".into())),
    };
    let g = server::Gate::new(cmd, cfg, log_path);
    g.check_updates();
    g.serve();
}
