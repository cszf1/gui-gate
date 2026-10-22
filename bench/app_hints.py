"""Pick the per-app hint block for a task: `python3 bench/app_hints.py TASKNAME` (e.g. ow01-libreoffice_calc-7efeb4b1,
og04-os-copy-dir2-conflict, og08-writer-decoy-front). Static text from bench/app_hints.md; prints nothing if no app matches."""
import os, re, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ALIASES = [("libreoffice_calc", "libreoffice_calc"), ("calc", "libreoffice_calc"), ("libreoffice_writer", "libreoffice_writer"),
           ("writer", "libreoffice_writer"), ("libreoffice_impress", "libreoffice_impress"), ("impress", "libreoffice_impress"),
           ("chrome", "chrome"), ("vlc", "vlc"), ("gimp", "gimp"), ("multi_apps", "multi_apps"), ("-os-", "os"),
           ("clone", "os")]

def sections():
    out, cur = {}, None
    for line in open(os.path.join(HERE, "app_hints.md")):
        m = re.match(r"^## (\S+)", line)
        if m:
            cur = m.group(1); out[cur] = []
        elif cur and not line.startswith("#"):
            out[cur].append(line.rstrip("\n"))
    return {k: " ".join(x.strip() for x in v if x.strip()) for k, v in out.items()}

def hint_for(task):
    sec = sections()
    for key, name in ALIASES:
        if key in task:
            return sec.get(name, "")
    return ""

if __name__ == "__main__":
    print(hint_for(sys.argv[1] if len(sys.argv) > 1 else ""))
