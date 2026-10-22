"""Rounds 6-7 offline: in the runs whose primary failure cause was 'tree missing' (spreadsheet grid / terminal output),
when would the new AT-SPI exposure first have shown the missing content, and how many turns came after it?
Reads the transcripts (analysis/tx) + taxonomy.csv. Not task success: only where the information gap was."""
import csv, glob, os, re, statistics as st, sys
HERE = os.path.dirname(os.path.abspath(__file__))
AN = os.path.join(HERE, "..", "analysis")
tax = {r["tag"]: r for r in csv.DictReader(open(os.path.join(AN, "taxonomy.csv")))}
def calls(tag):
    out = []
    cur = None
    for l in open(os.path.join(AN, "tx", tag + ".txt"), errors="replace"):
        m = re.match(r"T(\d+) CALL (\w+) (.*)", l)
        if m:
            cur = {"t": int(m.group(1)), "tool": m.group(2), "args": m.group(3), "res": ""}
            out.append(cur)
        elif l.startswith("   -> ") and cur is not None:
            cur["res"] += l
    return out
def summarize(cause, first_fn):
    tags = [t for t, r in tax.items() if r["primary"] == cause]
    rows = []
    for t in tags:
        cs = calls(t)
        last = max([c["t"] for c in cs] or [0])
        f = first_fn(cs)
        n = sum(1 for c in cs if c["tool"] == "get_window_state" and ("cell grid" in c["res"] or "Terminal" in c["res"] and "terminal" in c["res"]))
        rows.append((t, tax[t]["cond"], f, last, n))
    for cond in ("direct", "gate8"):
        r = [x for x in rows if x[1] == cond]
        hit = [x for x in r if x[2] is not None]
        after = [x[3] - x[2] for x in hit]
        print(f"{cause} {cond}: runs {len(r)}, content would show at turn (median) {st.median([x[2] for x in hit]) if hit else '-'}"
              f" in {len(hit)}/{len(r)} runs; turns after that: median {st.median(after) if after else '-'}, total {sum(after)};"
              f" reads that returned the gap: {sum(x[4] for x in r)}")
    return rows
def first_grid(cs):
    for c in cs:
        if "cell grid" in c["res"]:
            return c["t"]
    return None
def first_term_cmd(cs):
    # first command typed into a terminal and run (text with a newline, or Enter right after)
    term_pids = set()
    for c in cs:
        for m in re.finditer(r'pid=(\d+) "Terminal', c["res"]):
            term_pids.add(m.group(1))
    for i, c in enumerate(cs):
        if c["tool"] == "type_text" and any(f'"pid": {p}' in c["args"] for p in term_pids):
            if "\\n" in c["args"] or (i + 1 < len(cs) and cs[i + 1]["tool"] == "press_key" and re.search(r"(?i)return|enter", cs[i + 1]["args"])):
                return c["t"]
    return None
g = summarize("tree_missing:spreadsheet_grid", first_grid)
t = summarize("tree_missing:terminal_output", first_term_cmd)
