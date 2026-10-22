"""Round 9 offline: type_text calls that commit a line/cell (\\n or \\t) into a terminal or LibreOffice Calc, and how
often the agent's very next call was a read of that window (the turn the gate's after-typing look replaces)."""
import glob, os, re, collections
HERE = os.path.dirname(os.path.abspath(__file__))
TX = os.path.join(HERE, "..", "analysis", "tx")
c = collections.Counter()
for f in glob.glob(os.path.join(TX, "*.txt")):
    cond = "gate8" if "-gate8-" in f else "direct"
    calls = []
    term, calc = set(), set()
    for l in open(f, errors="replace"):
        for m in re.finditer(r'pid=(\d+) "([^"]*)"[^/]*?app=([\w.-]+)', l):
            if "terminal" in m.group(3).lower():
                term.add(m.group(1))
            if "calc" in m.group(3).lower() or "LibreOffice Calc" in m.group(2):
                calc.add(m.group(1))
        m = re.match(r"T(\d+) CALL (\w+) (.*)", l)
        if m:
            calls.append((m.group(2), m.group(3)))
    for i, (tool, a) in enumerate(calls):
        if tool != "type_text":
            continue
        pm = re.search(r'"pid": (\d+)', a)
        if not pm:
            continue
        kind = "terminal" if pm.group(1) in term else "calc" if pm.group(1) in calc else None
        if not kind:
            continue
        nxt = calls[i + 1] if i + 1 < len(calls) else ("", "")
        commit = "\\n" in a or "\\t" in a
        enter_next = nxt[0] == "press_key" and re.search(r"(?i)return|enter|tab", nxt[1])
        c[(cond, kind, "commit" if commit else ("then Enter/Tab key" if enter_next else "no commit"))] += 1
        if commit:
            c[(cond, kind, "commit -> next call is a read")] += nxt[0] == "get_window_state"
for k in sorted(c):
    print(f"{k[0]:6s} {k[1]:8s} {k[2]:32s} {c[k]}")
