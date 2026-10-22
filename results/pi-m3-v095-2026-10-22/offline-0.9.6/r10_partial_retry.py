"""Round 10 offline: reads that followed the driver's PARTIAL TREE advice ("retry with a larger timeout_ms or narrow
with query / max_depth") on a spreadsheet grid - re-reads of the same pid with timeout_ms / max_elements / query /
max_depth after a grid read, before any action."""
import glob, os, re, collections
HERE = os.path.dirname(os.path.abspath(__file__))
TX = os.path.join(HERE, "..", "analysis", "tx")
c = collections.Counter(); runs = collections.defaultdict(set)
for f in glob.glob(os.path.join(TX, "*.txt")):
    cond = "gate8" if "-gate8-" in f else "direct"
    grid_pids = set(); calls = []
    cur = None
    for l in open(f, errors="replace"):
        m = re.match(r"T(\d+) CALL (\w+) (.*)", l)
        if m:
            cur = [m.group(2), m.group(3), ""]; calls.append(cur)
        elif l.startswith("   -> ") and cur:
            cur[2] += l
    seen_grid = set()
    for tool, a, r in calls:
        pm = re.search(r'"pid": (\d+)', a); p = pm.group(1) if pm else None
        if tool == "get_window_state" and p in seen_grid and re.search(r'"(timeout_ms|max_elements|query|max_depth)"', a):
            c[cond] += 1; runs[cond].add(f)
        if tool == "get_window_state" and "cell grid" in r and p:
            seen_grid.add(p)
for k in sorted(c):
    print(f"{k}: {c[k]} re-reads with timeout_ms/max_elements/query/max_depth after a grid read, in {len(runs[k])} runs")
