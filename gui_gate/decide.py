"""The value gate: hard rules, then a calibrated light predictor that compares
"expected benefit increment" against cost, with a cheap probe in the grey zone.

Options
  direct      execute now with what the agent already has
  check       gate probes the target cheaply (verify_state, ~20 ms, no model)
  heal        stale element token: gate re-reads the tree (no screenshot) and
              re-binds the token to the unique element with the same role+label
  wait        UI looks busy: wait until it settles, then act
  look_light  agent must re-read the tree (no screenshot) -> one model turn
  look_full   agent must re-read with a screenshot -> model turn + image
  reason      agent must double-check which target it means -> model turn
  risk_review high-risk action: the main model must think it over and re-send with gate_ack
  ask_human   high-risk action: a person approves it (dialog / MCP elicitation / via the agent)
  handoff     a person must do this step (secret entry, captcha) before the agent goes on
  deny        risky and not authorized (reviewer said no, a person said no, or the same effect was already
              refused): not executed, the agent gets a safe alternative and goes on (deny-and-continue)

Deciders (who settles the grey zone after the rules):
  formula  rules + calibrated logistic predictor + tree probe (no model calls)
  model    formula first; what it cannot settle cheaply is handed to the main model
           (model_review: it re-reads or re-sends with gate_ack); risk -> main model
"""
from __future__ import annotations

import json
import math
import os
from dataclasses import dataclass, field
from typing import Optional

from .features import FEATURE_NAMES, ActionCtx

OPTIONS = ["direct", "check", "heal", "wait", "look_light", "look_full", "reason", "risk_review", "model_review", "ask_human",
           "handoff", "deny"]

# Prior weights for P(direct succeeds). Hand-set, documented as a prior only:
# replaced by `gui-gate-train` once logs exist. Logit scale.
PRIOR_WEIGHTS = {
    "bias": 2.6, "is_pixel": -0.9, "is_token": 0.4, "is_name": 0.3, "is_keyboard": 0.2,
    "no_obs": -2.5, "token_stale": -3.2, "log_obs_age": -0.15, "acts_since_obs": -0.55,
    "obs_had_shot": 0.1, "last_err": -0.9, "last_unverif": -0.3, "last_popup": -1.2,
    "repeat": -0.8, "edge_close": -1.0, "small_target": -0.6, "label_empty": -0.4,
    "label_dup": -0.9, "loading_cue": -1.3, "window_changed": -1.2, "window_added": -1.5, "elem_unknown": -0.7,
    "check_pass": 1.6, "check_fail": -3.5,
}

# Fraction of the remaining failure probability each option removes
# (benefit increment). Priors; learnable from logs.
PRIOR_GAINS = {
    "check": 0.45, "heal": 0.65, "wait": 0.35, "look_light": 0.75, "look_full": 0.92,
    "reason": 0.55,
}
PIXEL_GAINS = {"look_light": 0.35, "look_full": 0.92, "check": 0.0}

# Cost in seconds-equivalent (latency + money folded into one unit).
DEFAULT_COSTS = {
    "direct": 0.0, "check": 0.05, "heal": 0.15, "wait": 0.8, "look_light": 3.0,
    "look_full": 5.5, "reason": 4.0, "ask_human": 30.0,
}


@dataclass
class GateConfig:
    policy: str = "gate"          # gate | direct | light | full
    lam: float = 1.0              # weight on cost (higher = thriftier)
    fail_cost: float = 12.0       # seconds-equivalent lost by a failed action
    grey_lo: float = 0.55         # P(direct) band where a cheap probe is worth it
    grey_hi: float = 0.9
    risk_confirm: bool = True     # high-risk actions are stopped (see risk_mode)
    risk_mode: str = "auto"       # auto: high risk -> person, medium -> main model re-thinks (gate_ack)
                                  # model: all risk -> main model | human: all risk -> person
    decider: str = "formula"      # formula | model  (who settles the grey zone)
    collect: bool = False         # shadow-label every action
    human_channel: str = "auto"   # auto | elicit | desktop | chat | deny  (how a person is reached)
    human_timeout: float = 120.0  # seconds to wait for a person before treating it as "no"
    toolset: str = "core"         # core: expose only the tools a GUI task needs | all
    auto_observe: bool = True     # after an action, return the window's fresh tree (no extra model turn)
    delivery: str = "auto"        # auto (background, sticky fallback to foreground) | background | foreground
    task: str = ""                # the user's request, verbatim (authorizes risky actions it explicitly asks for)
    batch: str = "auto"           # auto: run_actions + gui_script (if quickjs is installed) | actions | off
    deny_consecutive: int = 3     # refusals in a row before the agent is told to stop and report
    deny_total: int = 8           # refusals per session before the same
    costs: dict = field(default_factory=lambda: dict(DEFAULT_COSTS))
    weights: dict = field(default_factory=lambda: dict(PRIOR_WEIGHTS))
    gains: dict = field(default_factory=lambda: dict(PRIOR_GAINS))
    calib: Optional[dict] = None  # {"a":..,"b":..} Platt on the logit
    model_name: str = "prior_v0"

    @classmethod
    def from_env(cls, **over) -> "GateConfig":
        c = cls()
        c.policy = os.environ.get("GUI_GATE_POLICY", c.policy)
        c.lam = float(os.environ.get("GUI_GATE_LAMBDA", c.lam))
        c.collect = os.environ.get("GUI_GATE_COLLECT", "0") in ("1", "true", "yes")
        c.risk_confirm = os.environ.get("GUI_GATE_RISK_CONFIRM", "1") not in ("0", "false", "no")
        c.risk_mode = os.environ.get("GUI_GATE_RISK_MODE", c.risk_mode)
        c.decider = os.environ.get("GUI_GATE_DECIDER", c.decider)
        c.human_channel = os.environ.get("GUI_GATE_HUMAN", c.human_channel)
        c.toolset = os.environ.get("GUI_GATE_TOOLSET", c.toolset)
        c.auto_observe = os.environ.get("GUI_GATE_AUTO_OBSERVE", "1") not in ("0", "false", "no")
        c.delivery = os.environ.get("GUI_GATE_DELIVERY", c.delivery)
        c.task = os.environ.get("GUI_GATE_TASK", c.task)
        tf = os.environ.get("GUI_GATE_TASK_FILE")
        if not c.task and tf and os.path.exists(tf):
            c.task = open(tf, encoding="utf-8").read().strip()
        c.batch = os.environ.get("GUI_GATE_BATCH", c.batch)
        for k, v in over.items():
            if v is not None:
                setattr(c, k, v)
        wpath = os.environ.get("GUI_GATE_WEIGHTS")
        if wpath and os.path.exists(wpath):
            c.load_weights(wpath)
        return c

    def load_weights(self, path: str) -> None:
        d = json.load(open(path))
        self.weights.update(d.get("weights", {}))
        self.gains.update(d.get("gains", {}))
        self.calib = d.get("calib")
        self.model_name = d.get("name", os.path.basename(path))


def sigmoid(z: float) -> float:
    if z >= 0:
        return 1 / (1 + math.exp(-z))
    e = math.exp(z)
    return e / (1 + e)


def p_direct(f: dict, cfg: GateConfig) -> float:
    z = sum(cfg.weights.get(k, 0.0) * f.get(k, 0.0) for k in FEATURE_NAMES)
    if cfg.calib:
        z = cfg.calib.get("a", 1.0) * z + cfg.calib.get("b", 0.0)
    return sigmoid(z)


@dataclass
class Decision:
    option: str
    p_direct: float
    reason: str
    tier: str  # rule | model | grey | policy
    scores: dict = field(default_factory=dict)  # option -> expected cost


def hard_rules(ctx: ActionCtx, f: dict, cfg: GateConfig, confirmed: bool, ack: bool = False) -> Optional[Decision]:
    """Tier 0: zero-cost rules. Most come from field lessons in the Codex
    computer-use reports (stale indices, off-screen clamps, unknown results)."""
    if ctx.handoff and cfg.risk_confirm and not confirmed:
        return Decision("handoff", 0.0, ctx.handoff, "rule")
    if ctx.risk and cfg.risk_confirm and not confirmed:
        level = ctx.risk_level or "high"
        to_human = cfg.risk_mode == "human" or (cfg.risk_mode == "auto" and level == "high")
        if to_human:
            return Decision("ask_human", 0.0, f"{level}-risk action: {ctx.risk}", "rule")
        if not ack:
            return Decision("risk_review", 0.0, f"{level}-risk action: {ctx.risk}", "rule")
    if f["token_stale"]:
        return Decision("heal", 0.0, "element token is from an older snapshot of this window", "rule")
    if f["no_obs"] and ctx.target.kind in ("token", "index", "pixel"):
        return Decision("look_full" if ctx.target.kind == "pixel" else "look_light", 0.0,
                        "no observation of this window has passed through the gate", "rule")
    if f["last_err"] and f["repeat"] >= 1:
        return Decision("look_full", 0.0,
                        "the identical action just failed; its effect is unknown, so it must not be re-sent blind",
                        "rule")
    if f["repeat"] >= 2 and f["last_unverif"]:
        return Decision("reason", 0.0, "same action sent repeatedly without a confirmed effect (possible loop)",
                        "rule")
    if ctx.target.kind == "pixel" and (f["window_changed"] or f["last_popup"]) and f["acts_since_obs"] >= 1:
        return Decision("look_full", 0.0, "pixel target but windows/popups changed since the screenshot", "rule")
    if f["window_added"] and f["acts_since_obs"] >= 1 and ctx.target.kind != "keyboard":
        return Decision("look_light", 0.0, "a new window or dialog of this app appeared since your last read "
                        "(list_windows, then read it)", "rule")
    return None


def choose(ctx: ActionCtx, f: dict, cfg: GateConfig, check_available: bool, check_done: bool) -> Decision:
    """Tier 1 (+ grey zone): minimise lam*cost + (1-P)*fail_cost."""
    p = p_direct(f, cfg)
    pixel = ctx.target.kind == "pixel"
    keyboard = ctx.target.kind in ("keyboard", "none")
    avail = ["direct", "look_light", "look_full", "reason"]
    if check_available and not check_done:
        avail.append("check")
    if f["loading_cue"] or f["last_popup"]:
        avail.append("wait")
    if keyboard:
        avail = [o for o in avail if o not in ("reason",)]
    scores = {}
    for o in avail:
        if o == "direct":
            po = p
        else:
            g = (PIXEL_GAINS.get(o) if pixel and o in PIXEL_GAINS else cfg.gains.get(o, 0.0))
            po = p + g * (1 - p)
        scores[o] = cfg.lam * cfg.costs.get(o, 0.0) + (1 - po) * cfg.fail_cost
    best = min(scores, key=scores.get)
    tier = "model"
    # Grey zone: before paying for a model turn, spend ~20 ms on a probe.
    if (check_available and not check_done and best != "direct" and best in ("look_light", "look_full", "reason")):
        best, tier = "check", "grey"
    elif check_available and not check_done and best == "direct" and cfg.grey_lo <= p < cfg.grey_hi:
        best, tier = "check", "grey"
    reason = f"P(direct)={p:.2f}; expected cost " + ", ".join(f"{k}={v:.2f}" for k, v in sorted(scores.items(), key=lambda kv: kv[1])[:3])
    return Decision(best, p, reason, tier, scores)


def policy_decision(ctx: ActionCtx, f: dict, cfg: GateConfig) -> Optional[Decision]:
    """Baselines. direct: never intervene. light/full: require a fresh read
    (tree / screenshot) of the window after the last action, every step."""
    if cfg.policy == "direct":
        return Decision("direct", 1.0, "baseline: always direct", "policy")
    if cfg.policy in ("light", "full"):
        if ctx.target.kind in ("keyboard", "none") and f["no_obs"]:
            return Decision("direct", 1.0, "baseline: nothing to observe", "policy")
        need_shot = cfg.policy == "full"
        fresh = (not f["no_obs"]) and f["acts_since_obs"] == 0 and (f["obs_had_shot"] or not need_shot) \
            and not f["token_stale"]
        if fresh:
            return Decision("direct", 1.0, f"baseline {cfg.policy}: fresh observation", "policy")
        return Decision("look_full" if need_shot else "look_light", 0.0,
                        f"baseline {cfg.policy}: observe before every action", "policy")
    return None
