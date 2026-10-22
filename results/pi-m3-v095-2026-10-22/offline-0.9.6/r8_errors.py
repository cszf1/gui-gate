"""Round 8 offline: agent turns lost to two driver refusals the gate can now absorb (m3 transcripts + ds41 pi logs).
 - token click refused because another control owns the element's point (any owner)
 - get_window_state of a pid that owns no window"""
import glob, os, re, collections, json
HERE = os.path.dirname(os.path.abspath(__file__))
def count_tx(d):
    c = collections.Counter(); runs = collections.defaultdict(set)
    for f in glob.glob(os.path.join(d, "*.txt")):
        tag = os.path.basename(f)[:-4]; cond = "gate8" if "-gate8-" in tag else "direct"
        prev = ""
        for l in open(f, errors="replace"):
            if l.startswith("T"):
                prev = l
            if l.startswith("   -> ERR"):
                if "but that screen point is owned by" in l and "CALL click" in prev and "element_token" in prev:
                    c[(cond, "occluded_token_click")] += 1; runs[(cond, "occluded_token_click")].add(tag)
                if "No windows found for pid" in l:
                    c[(cond, "no_window_read")] += 1; runs[(cond, "no_window_read")].add(tag)
    return c, runs
def count_pi(d):
    c = collections.Counter(); runs = collections.defaultdict(set)
    for f in glob.glob(os.path.join(d, "pi", "*.jsonl")):
        tag = os.path.basename(f)[:-6]; cond = "gate8" if "-gate8-" in tag else "direct"
        s = open(f, errors="replace").read()
        n1 = len(re.findall(r"but that screen point is owned by", s)) // 2  # each result appears in two events
        n2 = len(re.findall(r"No windows found for pid", s)) // 2
        for k, n in (("occluded (any click)", n1), ("no_window_read", n2)):
            if n:
                c[(cond, k)] += n; runs[(cond, k)].add(tag)
    return c, runs
for name, (c, r) in [("m3 (transcripts)", count_tx(os.path.join(HERE, "..", "analysis", "tx"))),
                     ("ds41 (pi logs)", count_pi(os.path.join(HERE, "..", "..", "pi-ds41-v094-2026-10-21")))]:
    print(name)
    for k in sorted(c):
        print(f"  {k[0]:6s} {k[1]:22s} {c[k]:4d} in {len(r[k])} runs")
