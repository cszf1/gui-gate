"""P1 counterfactual harness: branch the SAME step under every treatment from a recoverable checkpoint.

Protocol (README.md): after the controller has produced its candidate action, freeze the environment, the candidate,
the controller's full state (its belief = its model context, its sampling state), the remaining budget and - in
replay mode - the gate's history. From that checkpoint, restore once per (treatment, repeat) in a shuffled order,
verify the restore by state hash, apply the treatment, execute, then continue the task with a FIXED control policy
pi inside the remaining budget. Record step success, task success, the treatment's cost and the restore check.

Treatments (identical names to the gate's options where they exist):
  direct        execute the candidate
  look_light    fresh tree read of the main window (no dialog, no image), the same controller re-chooses
  look_full     fresh read with the screenshot (dialogs visible), re-choose
  check         the gate's cheap probe of the candidate's element; pass -> execute, fail -> look_light path
  extra_reason  one more think of the controller with NO new observation (may change the candidate, never authorises)
  matched_wait  wait as long as look_light costs, no new information, then execute the original candidate
  ask           a scripted person names the right element (person seconds are recorded)

Backends: SimBackend (the fixture twin in-process) and GateBackend (the same twin behind tests-style MCP, driven
through the real gui-gate binary). A checkpoint restores by snapshot (exact state load) or by replay (fresh env from
the seed + the logged action prefix, through the gate when GateBackend is used); either way the state hash after
restore is compared with the checkpoint's hash and `restored_ok` records the result.
"""
import copy
import json
import os
import random
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from fixture_env import FixtureEnv  # noqa: E402

TREATMENTS = ["direct", "look_light", "look_full", "check", "extra_reason", "matched_wait", "ask"]
# virtual seconds / tokens (the controller is scripted: these are accounting, not sleeps)
COST = {"turn_s": 7.0, "image_s": 2.0, "act_s": 0.3, "probe_s": 0.2, "person_s": 20.0,
        "tree_tok": 300, "image_tok": 1100, "reason_tok": 400}
FAMILIES = {  # hazard profiles = task families (all checkpoints of one family stay on one side of a split)
    "relabel": dict(p_relabel=0.5, p_loading=0.1, p_popup=0.05),
    "loading": dict(p_relabel=0.1, p_loading=0.5, p_popup=0.05, loading_s=12.0),  # slow loads: waiting itself can help
    "popup": dict(p_relabel=0.1, p_loading=0.1, p_popup=0.35),
    "mixed": dict(p_relabel=0.25, p_loading=0.2, p_popup=0.12),
}



def env_kwargs(family):
    """FixtureEnv kwargs of a family: a hazard profile name, or "layout:<id>" = a held-out layout variant
    (layouts.py) with its own hazard profile. Every layout variant is its own family."""
    if family.startswith("layout:"):
        import layouts
        lay = layouts.get(family.split(":", 1)[1])
        return dict(FAMILIES[lay["hazards"]], layout=lay)
    return dict(FAMILIES[family])


# ---------------------------------------------------------------- the scripted controller (stand-in acting model)

class Controller:
    """Acts from its own belief (its last read), like sim_agent.py: optimistic about its own clicks."""

    def __init__(self, seq, seed=0):
        self.b = {"labels": None, "k": 0, "popup": False, "since_read": 0, "seq": list(seq)}
        self.rng = random.Random(seed)

    def dump(self):
        v, internal, g = self.rng.getstate()
        return {"b": copy.deepcopy(self.b), "rng": [v, list(internal), g]}

    @classmethod
    def load(cls, d):
        c = cls.__new__(cls)
        c.b = copy.deepcopy(d["b"])
        v, internal, g = d["rng"]
        c.rng = random.Random()
        c.rng.setstate((v, tuple(internal), g))
        return c

    def reseed(self, seed):
        """model sampling noise of one repeat (the env's own randomness is part of the checkpoint)"""
        self.rng = random.Random(seed)

    def observe(self, o):
        old = self.b["labels"] or ["?"] * len(o["labels"])
        self.b["labels"] = [old[i] if lab == "Loading..." else lab for i, lab in enumerate(o["labels"])]
        self.b["k"] = o["k"]
        self.b["popup"] = o["popup"]
        self.b["since_read"] = 0

    def candidate(self):
        b = self.b
        if b["popup"]:
            return "ok"
        if b["k"] >= len(b["seq"]) or not b["labels"]:
            return 0
        want = b["seq"][b["k"]]
        return b["labels"].index(want) if want in b["labels"] else self.rng.randrange(len(b["labels"]))

    def rethink(self, cand):
        """extra_reason without new information: doubts its belief after several blind actions and may re-guess"""
        if cand == "ok" or self.b["since_read"] < 1:
            return cand
        return cand if self.rng.random() < 0.6 else self.rng.randrange(len(self.b["labels"] or [0] * 8))

    def acted(self, cand):
        if cand == "ok":
            self.b["popup"] = False
        else:
            self.b["k"] += 1  # optimistic
        self.b["since_read"] += 1


def person_answer(env_view, rng, acc=0.9):
    """scripted person: looks at the real screen and names the right element (acc of the time)"""
    if env_view["popup"]:
        return "ok"
    if env_view["k"] >= len(env_view["seq"]):
        return 0
    want = env_view["seq"][env_view["k"]]
    return env_view["true_labels"].index(want) if rng.random() < acc else rng.randrange(len(env_view["true_labels"]))


# ---------------------------------------------------------------- backends

class SimBackend:
    name = "sim"

    def __init__(self, env):
        self.env = env

    @classmethod
    def from_state(cls, state):
        return cls(FixtureEnv.load(state))

    @classmethod
    def fresh(cls, seed, length, family):
        return cls(FixtureEnv(seed=seed, length=length, **env_kwargs(family)))

    def observe(self, full):
        return self.env.observe(popup_visible=full)

    def truth(self):
        o = self.env.observe(True)
        o["true_labels"] = list(self.env.st["labels"])
        return o

    def probe(self, index, caption):
        return self.env.probe(index, caption)

    def click(self, index):
        ev = self.env.click(index)
        ev["executed"] = True
        return ev

    def advance(self, s):
        self.env.advance(s)

    def dump(self):
        return self.env.dump()

    def state_hash(self):
        return self.env.state_hash()

    def done(self):
        return self.env.done

    def close(self):
        pass


class GateBackend:
    """The fixture twin behind fixture_driver.py, every GUI action sent through the real gui-gate binary.
    The gate runs with its normal engineering base (--policy gate) and GUI_GATE_VALUE_POLICY=direct, so it never adds
    a value intervention of its own on top of the treatment; its hard rules (stale-token heal, ...) still apply."""
    name = "gate"
    PID, WID = 5151, 901

    def __init__(self, gate_bin, state=None, seed=0, length=10, family="mixed", log=None):
        st = state or FixtureEnv(seed=seed, length=length, **env_kwargs(family)).dump()
        self.state_file = tempfile.NamedTemporaryFile("w", suffix=".json", delete=False)
        json.dump(st, self.state_file)
        self.state_file.close()
        self.log = log or tempfile.NamedTemporaryFile(suffix=".jsonl", delete=False).name
        drv = f"{sys.executable} {os.path.join(HERE, 'fixture_driver.py')}"
        env = dict(os.environ, FIXTURE_STATE=self.state_file.name, GUI_GATE_VALUE_POLICY="direct", GUI_GATE_NO_UPDATE_CHECK="1",
                   GUI_GATE_CHILD_ENV="FIXTURE_*,PYTHONPATH", PYTHONPATH=HERE,
                   GUI_GATE_REVIEWER_URL="http://127.0.0.1:9/v1/chat/completions", GUI_GATE_REVIEWER_MODEL="none",
                   GUI_GATE_REVIEWER_KEY="offline", GUI_GATE_REVIEWER_TIMEOUT="1",
                   GUI_GATE_REVIEWER_ENV=os.path.join(HERE, "no-such-reviewer.env"))
        self.p = subprocess.Popen([gate_bin, "--child", drv, "--log", self.log, "--policy", "gate", "--human", "deny",
                                   "--no-auto-observe", "--toolset", "all"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  text=True, env=env)
        self.i = 0
        self.rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "cf-harness", "version": "0"}})
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        self.p.stdin.flush()
        self.snap = None

    def rpc(self, method, params):
        self.i += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.i, "method": method, "params": params}) + "\n")
        self.p.stdin.flush()
        while True:
            m = json.loads(self.p.stdout.readline())
            if m.get("id") == self.i and "method" not in m:
                return m.get("result") or {"isError": True, "content": [{"type": "text", "text": str(m.get("error"))}]}

    def call(self, name, **a):
        return self.rpc("tools/call", {"name": name, "arguments": a})

    @staticmethod
    def text(r):
        return "\n".join(c.get("text", "") for c in r.get("content", []) if c.get("type") == "text")

    def _dump(self):
        t = self.text(self.call("fixture_dump"))
        return json.loads(t[t.index("{"):])

    def observe(self, full):
        r = self.call("get_window_state", pid=self.PID, window_id=self.WID, include_screenshot=full)
        sc = r.get("structuredContent") or {}
        self.snap = sc.get("snapshot_id")
        md = sc.get("tree_markdown") or self.text(r)
        import re
        rows = re.findall(r'\[(\d+)\] button "([^"]*)"', md)
        labels = [lab for i, lab in rows if int(i) < 8]
        k = int(re.search(r"Progress (\d+)/", md).group(1))
        seq = re.search(r'Sequence: ([^"]*)"', md).group(1).split(", ")
        return {"labels": labels, "k": k, "seq": seq, "popup": len(rows) > 8}

    def truth(self):
        st = self._dump()["state"]
        busy = st["t"] < st["busy_until"]
        return {"labels": ["Loading..." if busy else x for x in st["labels"]], "true_labels": st["labels"], "k": st["k"],
                "seq": st["seq"], "popup": st["popup"]}

    def probe(self, index, caption):
        t = self.text(self.call("fixture_probe", index=index, caption=caption))
        return json.loads(t)["pass"]

    def click(self, index):
        if self.snap is None:
            self.observe(True)
        idx = 8 if index == "ok" else index
        r = self.call("click", element_token=f"{self.snap}:{idx}")
        sc = r.get("structuredContent") or {}
        ev = dict(sc.get("fixture") or {"ev": "not_executed", "progress": False, "useful": False})
        ev["executed"] = "fixture" in sc
        if not ev["executed"]:
            ev["gate"] = self.text(r)[:200]
        return ev

    def advance(self, s):
        self.call("fixture_advance", s=s)

    def dump(self):
        return self._dump()["state"]

    def state_hash(self):
        return self._dump()["hash"]

    def done(self):
        st = self.dump()
        return st["k"] >= st["length"]

    def close(self):
        try:
            self.p.stdin.close()
            self.p.wait(timeout=10)
        except Exception:
            self.p.kill()
        os.unlink(self.state_file.name)


# ---------------------------------------------------------------- running, checkpoints, branches

def act(be, ctl, cand, cost):
    ev = be.click(cand)
    be.advance(COST["act_s"])
    cost["wall_s"] += COST["act_s"]
    ctl.acted(cand)
    return ev


def continue_task(be, ctl, budget_s, read_every=3, max_actions=60):
    """the fixed control policy pi: a full read every `read_every` actions, otherwise act from belief"""
    spent, n = 0.0, 0
    while not be.done() and spent < budget_s and n < max_actions:
        if ctl.b["since_read"] >= read_every or ctl.b["labels"] is None:
            be.advance(COST["turn_s"] + COST["image_s"])
            spent += COST["turn_s"] + COST["image_s"]
            ctl.observe(be.observe(True))
        cand = ctl.candidate()
        be.advance(COST["turn_s"])  # one model turn per action
        spent += COST["turn_s"]
        c = {"wall_s": 0.0}
        act(be, ctl, cand, c)
        spent += c["wall_s"]
        n += 1
    return {"task_success": be.done(), "continuation_s": round(spent, 3), "continuation_actions": n}


def make_checkpoint(make_backend, seed, family, length, prefix_actions, budget_s, read_every=3):
    """Run pi from a fresh env for `prefix_actions` actions, then stop right after the next candidate is generated."""
    be = make_backend(None, seed, length, family)
    ctl = Controller(be.observe(True)["seq"], seed=seed)
    log = [("observe", True)]
    ctl.observe(be.observe(True))
    spent = COST["turn_s"] + COST["image_s"]
    be.advance(spent)
    log.append(("advance", spent))
    for _ in range(prefix_actions):
        if be.done():
            break
        if ctl.b["since_read"] >= read_every:
            be.advance(COST["turn_s"] + COST["image_s"])
            log.append(("advance", COST["turn_s"] + COST["image_s"]))
            ctl.observe(be.observe(True))
            log.append(("observe", True))
            spent += COST["turn_s"] + COST["image_s"]
        cand = ctl.candidate()
        be.advance(COST["turn_s"])
        log.append(("advance", COST["turn_s"]))
        be.click(cand)
        log.append(("click", cand))
        be.advance(COST["act_s"])
        log.append(("advance", COST["act_s"]))
        ctl.acted(cand)
        spent += COST["turn_s"] + COST["act_s"]
    cand = ctl.candidate()
    be.advance(COST["turn_s"])  # the turn that produced the candidate
    log.append(("advance", COST["turn_s"]))
    spent += COST["turn_s"]
    cp = {"id": f"{family}-s{seed}-a{prefix_actions}", "family": family, "seed": seed, "length": length,
          "env": be.dump(), "hash": be.state_hash(), "controller": ctl.dump(), "candidate": cand,
          "budget_left_s": max(0.0, budget_s - spent), "prefix": log, "done": be.done()}
    be.close()
    return cp


def restore(cp, make_backend, mode):
    if mode == "snapshot":
        be = make_backend(cp["env"], cp["seed"], cp["length"], cp["family"])
    else:  # replay: fresh env from the seed + the logged prefix (through the gate for GateBackend)
        be = make_backend(None, cp["seed"], cp["length"], cp["family"])
        for kind, arg in cp["prefix"]:
            if kind == "advance":
                be.advance(arg)
            elif kind == "observe":
                be.observe(arg)
            elif kind == "click":
                be.click(arg)  # exactly as logged: no extra reads (a read changes what the gate's rules see)
    h = be.state_hash()
    return be, Controller.load(cp["controller"]), h == cp["hash"], h


def apply_treatment(t, be, ctl, cand, rng):
    cost = {"wall_s": 0.0, "tokens": 0, "person_s": 0.0}
    info = {}
    if t == "direct":
        ev = act(be, ctl, cand, cost)
    elif t in ("look_light", "look_full"):
        full = t == "look_full"
        o = be.observe(full)
        s = COST["turn_s"] + (COST["image_s"] if full else 0.0)
        be.advance(s)
        cost["wall_s"] += s
        cost["tokens"] += COST["tree_tok"] + (COST["image_tok"] if full else 0)
        ctl.observe(o)
        c2 = ctl.candidate()
        info["changed"] = c2 != cand
        ev = act(be, ctl, c2, cost)
    elif t == "check":
        be.advance(COST["probe_s"])
        cost["wall_s"] += COST["probe_s"]
        cap = ctl.b["labels"][cand] if cand != "ok" and ctl.b["labels"] else None
        ok = cand == "ok" or (cap is not None and be.probe(cand, cap))
        info["probe_pass"] = ok
        if ok:
            ev = act(be, ctl, cand, cost)
        else:
            ev2, c2 = apply_treatment("look_light", be, ctl, cand, rng)
            for k in cost:
                cost[k] += c2[k]
            return ev2, cost
    elif t == "extra_reason":
        be.advance(COST["turn_s"])
        cost["wall_s"] += COST["turn_s"]
        cost["tokens"] += COST["reason_tok"]
        c2 = ctl.rethink(cand)
        info["changed"] = c2 != cand
        ev = act(be, ctl, c2, cost)
    elif t == "matched_wait":
        be.advance(COST["turn_s"])  # same wall time as look_light, no information
        cost["wall_s"] += COST["turn_s"]
        ev = act(be, ctl, cand, cost)
    elif t == "ask":
        tr = be.truth()
        be.advance(COST["person_s"])
        cost["wall_s"] += COST["person_s"]
        cost["person_s"] += COST["person_s"]
        c2 = person_answer(tr, rng)
        ctl.observe({"labels": tr["true_labels"], "k": tr["k"], "seq": tr["seq"], "popup": tr["popup"]})
        info["changed"] = c2 != cand
        ev = act(be, ctl, c2, cost)
    else:
        raise ValueError(t)
    ev.update(info)
    return ev, cost


def run_branches(checkpoints, make_backend, treatments=TREATMENTS, repeats=3, seed=0, restore_mode="snapshot", out=None):
    rng = random.Random(seed)
    recs = []
    for cp in checkpoints:
        if cp["done"]:
            continue
        plan = [(t, r) for t in treatments for r in range(repeats)]
        rng.shuffle(plan)  # randomised branch order
        for pos, (t, r) in enumerate(plan):
            be, ctl, ok, h = restore(cp, make_backend, restore_mode)
            ctl.reseed(hash((cp["id"], r)) & 0xFFFFFFFF)
            ev, cost = apply_treatment(t, be, ctl, cp["candidate"], random.Random(r * 7919 + pos))
            left = cp["budget_left_s"] - cost["wall_s"]
            cont = continue_task(be, ctl, left)
            recs.append({"checkpoint": cp["id"], "family": cp["family"], "treatment": t, "repeat": r, "order": pos,
                         "backend": getattr(be, "name", "?"), "restore": restore_mode, "restored_ok": ok, "state_hash": h,
                         "expected_hash": cp["hash"], "gate_history_restored": restore_mode == "replay" and getattr(be, "name", "") == "gate",
                         "executed": ev.get("executed", True), "step_success": bool(ev.get("useful")), "event": ev.get("ev"),
                         "changed_candidate": ev.get("changed"), "cost_s": round(cost["wall_s"], 3), "tokens": cost["tokens"],
                         "person_s": cost["person_s"], **cont})
            be.close()
    if out:
        with open(out, "w") as f:
            for r in recs:
                f.write(json.dumps(r) + "\n")
    return recs


def sim_backend(state, seed, length, family):
    return SimBackend.from_state(state) if state is not None else SimBackend.fresh(seed, length, family)


def gate_backend(gate_bin):
    def mk(state, seed, length, family):
        return GateBackend(gate_bin, state=state, seed=seed, length=length, family=family)
    return mk
