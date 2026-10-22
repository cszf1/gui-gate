"""Context simulation for rounds 6-9 on the m3 transcripts: characters the new extras would add per run.
Assumptions measured on gui-pi2 (no model): full visible-cell block ~1.9 KB (58-row demo sheet, 129 cells; real task
sheets vary), post-action cell diff ~0.1-0.3 KB, terminal text ~0.3-0.7 KB (30-line cap ~2 KB). Removed: the round-2
grid/terminal notes (~430 / ~330 chars, once per window)."""
import glob, os, re, collections, statistics as st
HERE = os.path.dirname(os.path.abspath(__file__))
TX = os.path.join(HERE, "..", "analysis", "tx")
FULL_CELLS, DIFF_CELLS, TERM_READ, TERM_TYPED = 1900, 200, 600, 600
per = collections.defaultdict(list)
for f in glob.glob(os.path.join(TX, "*.txt")):
    cond = "gate8" if "-gate8-" in f else "direct"
    lines = open(f, errors="replace").read().split("\n")
    add = 0; base = 0
    term = set(m.group(1) for l in lines for m in re.finditer(r'pid=(\d+) "Terminal', l))
    prev = ""
    for l in lines:
        if l.startswith("   -> "):
            base += len(l)
            if "cell grid" in l and "CALL get_window_state" in prev:
                add += FULL_CELLS
            elif "cell grid" in l and "Window right after the action" in l:
                add += DIFF_CELLS
            pm = re.search(r'"pid": (\d+)', prev)
            if pm and pm.group(1) in term:
                if "CALL get_window_state" in prev:
                    add += TERM_READ
                elif "CALL type_text" in prev and "\\n" in prev or "CALL press_key" in prev and re.search(r"(?i)return|enter", prev):
                    add += TERM_TYPED
        elif l.startswith("T"):
            prev = l
    # real tool-result size from the Pi log (the transcripts cut each result)
    pi = os.path.join(HERE, "..", "pi", os.path.basename(f)[:-4] + ".jsonl")
    base = 0
    import json
    for pl in open(pi, errors="replace"):
        e = json.loads(pl)
        if e.get("type") == "tool_execution_end":
            for c_ in (e.get("result") or {}).get("content") or []:
                base += len(c_.get("text") or "")
    per[cond].append((base, add))
for c, v in sorted(per.items()):
    b = [x[0] for x in v]; a = [x[1] for x in v]
    print(f"{c}: runs {len(v)}; tool-result text per run median {st.median(b)/1000:.1f} KB (mean {st.mean(b)/1000:.1f});"
          f" added by rounds 6-9 median {st.median(a)/1000:.1f} KB, mean {st.mean(a)/1000:.1f} KB"
          f" (+{100*sum(a)/sum(b):.0f}% of tool-result text); max {max(a)/1000:.1f} KB")
