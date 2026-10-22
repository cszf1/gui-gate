"""Context hygiene for GUI runs: keep only the newest window observation in full, older ones summarised.
prune() is the same rule as bench/pi_ext/obs_prune.ts (the Pi `context` extension, opt-in with PI_OBS_PRUNE=1);
tests/test_bench.py checks both give identical output. `python3 bench/context_hygiene.py RUN_DIR...` replays the rule
on packed Pi logs and prints the input-context bytes per run with and without it."""
import json, os, re, sys
MARKERS = ["[gate] Window right after the action", "[gate] Window now (nothing was executed)"]
KEEP_FULL = 1
HEAD_KEEP = 600

def text_of(m):
    return "\n".join(c.get("text", "") for c in m.get("content") or [] if isinstance(c, dict) and c.get("type") == "text")

def is_obs(m):
    if not m or m.get("role") != "toolResult":
        return False
    if str(m.get("toolName", "")).endswith("get_window_state"):
        return True
    t = text_of(m)
    return any(k in t for k in MARKERS)

def summary(text):
    t = re.search(r'(?:frame|dialog|window) = "([^"]{0,80})', text)
    s = re.search(r"snapshot_id=(\S+)", text) or re.search(r"\(snapshot (s[0-9a-f]+)", text)
    rows = len(re.findall(r"^\s*- \[\d+\]", text, re.M))
    return f'[client: older window read summarised - "{t.group(1) if t else "?"}", snapshot {s.group(1) if s else "?"}, {rows} rows; the newest read is current]'

def prune_text(text, name):
    for k in MARKERS:
        i = text.find(k)
        if i >= 0:
            head = text[:i].rstrip()
            return (head[:HEAD_KEEP] + " ..." if len(head) > HEAD_KEEP else head) + ("\n" if head else "") + summary(text[i:])
    if str(name or "").endswith("get_window_state"):
        return summary(text)
    return text

def prune(messages):
    idx = [i for i, m in enumerate(messages) if is_obs(m)]
    old = set(idx[:max(0, len(idx) - KEEP_FULL)])
    out = []
    for i, m in enumerate(messages):
        if i not in old:
            out.append(m); continue
        content = []
        for c in m.get("content") or []:
            if not isinstance(c, dict) or c.get("type") == "image":
                continue
            content.append(dict(c, text=prune_text(c.get("text", ""), m.get("toolName"))) if c.get("type") == "text" else c)
        out.append(dict(m, content=content))
    return out

def run_messages(path):
    """Tool results of a Pi --mode json log as toolResult messages, in call order."""
    msgs = []
    for l in open(path):
        try:
            e = json.loads(l)
        except Exception:
            continue
        if e.get("type") == "tool_execution_end":
            r = e.get("result") or {}
            msgs.append({"role": "toolResult", "toolName": e.get("toolName", ""), "content": r.get("content") or [],
                         "images": sum(1 for c in r.get("content") or [] if isinstance(c, dict) and c.get("type") == "image")})
    return msgs

def size(m):
    return len(text_of(m).encode())

def replay(path):
    """(bytes resent without pruning, with pruning, images resent without, with): every LLM call resends all earlier
    tool results; the call after result k sees results 0..k."""
    msgs = run_messages(path)
    full = pr = imf = imp = 0
    for k in range(1, len(msgs) + 1):
        cur = msgs[:k]
        p = prune(cur)
        full += sum(size(m) for m in cur); pr += sum(size(m) for m in p)
        imf += sum(m.get("images", 0) for m in cur)
        imp += sum(m.get("images", 0) for m, q in zip(cur, p) if q is m)
    last_full = sum(size(m) for m in msgs)
    last_pr = sum(size(m) for m in prune(msgs))
    return full, pr, imf, imp, len(msgs), last_full, last_pr

if __name__ == "__main__":
    for d in sys.argv[1:]:
        tot = [0, 0, 0, 0, 0]; n = 0
        per = {}
        for f in sorted(os.listdir(os.path.join(d, "pi"))):
            if not f.endswith(".jsonl"):
                continue
            r = replay(os.path.join(d, "pi", f))
            cond = "gate8" if "-gate8-" in f else "direct" if "-direct-" in f else "other"
            per.setdefault(cond, [0] * 8)
            for i, v in enumerate(r):
                per[cond][i] += v
            per[cond][7] += 1
        print(f"== {os.path.basename(d.rstrip('/'))}")
        for cond, v in sorted(per.items()):
            full, pr, imf, imp, calls, lf, lp, runs = v
            print(f"{cond}: runs {runs}; tool-result text resent over all model calls {full/runs/1e6:.2f} MB/run -> {pr/runs/1e6:.2f} MB/run "
                  f"({100*(1-pr/max(full,1)):.0f}% less); screenshots resent {imf/runs:.0f} -> {imp/runs:.0f} per run; tool-result text in "
                  f"the last call's context {lf/runs/1e3:.0f} KB -> {lp/runs/1e3:.0f} KB per run")
