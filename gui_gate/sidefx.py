"""Side-effect checks the gate can do because it runs on the controlled machine.

A model sees pixels and a tree; it does not see the file system. Before a shell command typed into a
terminal is submitted, the gate parses it and looks at the disk: would `cp`/`mv`/`>` replace a file that
already exists, would a recursive chmod strip the execute bit from directories, would `rm` delete files.
It never runs the command; `find` is the only thing it runs, read-only (its actions are stripped), to know
which files a `find ... -exec cp` would touch.

Also: overwrite-confirmation dialogs ("... already exists. Do you want to replace it?") turn a plain
"Yes"/"Replace" click (or Enter) into a high-risk action.
"""
from __future__ import annotations

import glob
import os
import re
import shlex
import subprocess
from typing import Optional

OVERWRITE_CTX = re.compile(r"(already exists|do you want to (replace|overwrite)|replace the existing|overwrite the existing"
                           r"|file exists|已存在|是否(替换|覆盖)|替换现有)", re.I)
CONFIRM_LABELS = {"yes", "ok", "replace", "overwrite", "save", "continue", "use", "keep both", "&yes", "_yes",
                  "是", "确定", "替换", "覆盖", "保存", "继续"}
_TERMINAL_COMM = re.compile(r"(term|konsole|tilix|alacritty|kitty|wezterm|guake|terminator|foot|sakura)", re.I)
_SHELLS = {"bash", "sh", "zsh", "fish", "dash", "ksh"}


def is_terminal(pid) -> bool:
    try:
        comm = open(f"/proc/{int(pid)}/comm").read().strip()
        cmd = open(f"/proc/{int(pid)}/cmdline").read().replace("\0", " ")
    except (OSError, ValueError, TypeError):
        return False
    return bool(_TERMINAL_COMM.search(comm) or _TERMINAL_COMM.search(cmd.split(" ")[0] if cmd else ""))


def shell_cwd(term_pid) -> str:
    """Working directory of the newest shell running under a terminal emulator."""
    home = os.path.expanduser("~")
    try:
        term_pid = int(term_pid)
    except (TypeError, ValueError):
        return home
    kids: dict[int, list[int]] = {}
    comm: dict[int, str] = {}
    for d in os.listdir("/proc"):
        if not d.isdigit():
            continue
        try:
            st = open(f"/proc/{d}/stat").read()
            rp = st.rfind(")")
            ppid = int(st[rp + 2:].split()[1])
            kids.setdefault(ppid, []).append(int(d))
            comm[int(d)] = st[st.find("(") + 1:rp]
        except (OSError, ValueError, IndexError):
            continue
    best, stack = None, [term_pid]
    while stack:
        p = stack.pop()
        for k in kids.get(p, []):
            if comm.get(k) in _SHELLS:
                best = k if best is None else max(best, k)
            stack.append(k)
    if best is None:
        return home
    try:
        return os.readlink(f"/proc/{best}/cwd")
    except OSError:
        return home


def _p(path: str, cwd: str) -> str:
    path = os.path.expandvars(os.path.expanduser(path))
    return os.path.normpath(path if os.path.isabs(path) else os.path.join(cwd, path))


def _nonempty_file(p: str) -> bool:
    try:
        return os.path.isfile(p) and os.path.getsize(p) > 0
    except OSError:
        return False


def _expand(args: list[str], cwd: str) -> list[str]:
    out = []
    for a in args:
        if any(c in a for c in "*?["):
            hits = sorted(glob.glob(_p(a, cwd)))
            out.extend(hits or [_p(a, cwd)])
        else:
            out.append(_p(a, cwd))
    return out


def _copy_clobbers(srcs: list[str], dest: str, recursive: bool, parents: bool, cwd: str) -> list[str]:
    """Existing files a cp/mv of srcs -> dest would replace."""
    hit = []
    dest_is_dir = os.path.isdir(dest)
    for s in srcs:
        if parents:
            rel = os.path.relpath(s, cwd)
            tgt = os.path.join(dest, rel)
        elif dest_is_dir:
            tgt = os.path.join(dest, os.path.basename(s.rstrip("/")))
        else:
            tgt = dest
        if os.path.isdir(s) and recursive:
            if os.path.isdir(tgt):
                for root, _dirs, files in os.walk(s):
                    for f in files:
                        t2 = os.path.join(tgt, os.path.relpath(os.path.join(root, f), s))
                        if _nonempty_file(t2):
                            hit.append(t2)
        elif _nonempty_file(tgt) and os.path.realpath(tgt) != os.path.realpath(s):
            hit.append(tgt)
    return hit


def _cp_like(argv: list[str], cwd: str) -> Optional[tuple[str, str]]:
    tool = os.path.basename(argv[0])
    flags, pos, target_dir = set(), [], None
    it = iter(range(1, len(argv)))
    i = 1
    while i < len(argv):
        a = argv[i]
        if a in ("-t", "--target-directory"):
            target_dir = argv[i + 1] if i + 1 < len(argv) else None
            i += 2
            continue
        if a.startswith("--target-directory="):
            target_dir = a.split("=", 1)[1]
        elif a.startswith("--"):
            flags.add(a)
        elif a.startswith("-") and len(a) > 1:
            flags.update("-" + c for c in a[1:])
        else:
            pos.append(a)
        i += 1
    del it
    if flags & {"-n", "--no-clobber", "-i", "--interactive", "--update=none", "-u", "--update"}:
        return None
    if tool == "rsync" and "--ignore-existing" in flags:
        return None
    if target_dir:
        srcs, dest = pos, target_dir
    elif len(pos) >= 2:
        srcs, dest = pos[:-1], pos[-1]
    else:
        return None
    recursive = bool(flags & {"-r", "-R", "-a", "--recursive", "--archive"}) or tool in ("mv", "rsync")
    parents = "--parents" in flags or "-R" in flags and tool == "rsync"
    clob = _copy_clobbers(_expand(srcs, cwd), _p(dest, cwd), recursive, parents, cwd)
    if clob:
        names = ", ".join(os.path.relpath(c, cwd) for c in clob[:3]) + (" ..." if len(clob) > 3 else "")
        return "high", f"`{tool}` would overwrite existing file(s): {names}"
    return None


def _find_exec(argv: list[str], cwd: str) -> Optional[tuple[str, str]]:
    """find ... -exec cp/mv {} DEST \\; : list the files read-only, then check the copies."""
    try:
        k = next(i for i, a in enumerate(argv) if a in ("-exec", "-execdir", "-ok", "-okdir"))
    except StopIteration:
        return ("high", "`find -delete` deletes files") if "-delete" in argv else None
    inner = argv[k + 1:]
    end = next((i for i, a in enumerate(inner) if a in (";", "\\;", "+")), len(inner))
    inner = inner[:end]
    if not inner or os.path.basename(inner[0]) not in ("cp", "mv", "rsync", "install"):
        if inner and os.path.basename(inner[0]) == "rm":
            return "high", "`find -exec rm` deletes files"
        if inner and os.path.basename(inner[0]) == "chmod":
            return _chmod(inner, cwd, via_find=True)
        return None
    if any(a in ("-fprint", "-fprint0", "-fprintf", "-fls", "-delete") for a in argv[:k]):
        return None  # not read-only: do not run it
    try:
        out = subprocess.run(argv[:k] + ["-print0"], cwd=cwd, capture_output=True, timeout=3).stdout
    except (OSError, subprocess.SubprocessError):
        return None
    files = [f.decode(errors="replace") for f in out.split(b"\0") if f][:2000]
    hits = []
    for f in files:
        av = [f if a == "{}" else a for a in inner]
        r = _cp_like(av, cwd)
        if r:
            hits.append(r[1])
    if hits:
        return "high", hits[0] + (f" (and {len(hits) - 1} more)" if len(hits) > 1 else "")
    return None


def _chmod(argv: list[str], cwd: str, via_find: bool = False) -> Optional[tuple[str, str]]:
    rec = any(a in ("-R", "--recursive") or (a.startswith("-") and "R" in a and not a.startswith("--")) for a in argv[1:])
    mode = next((a for a in argv[1:] if not a.startswith("-")), "")
    if via_find or not rec:
        return None
    if re.fullmatch(r"0?[0-7]{3,4}", mode) and int(mode[-3], 8) % 2 == 0:
        return "high", f"`chmod -R {mode}` also hits directories and removes their execute bit (they become unopenable)"
    return "medium", f"`chmod -R {mode}` changes directories too, not only files"


def _commands(cmdline: str) -> list[tuple[list[str], list[str]]]:
    """[(argv, overwrite-redirect targets)] for each simple command; quotes and escapes respected."""
    src = (cmdline or "").replace("\\;", " __ESC_SEMI__ ").replace("\n", " ; ")
    lx = shlex.shlex(src, posix=True, punctuation_chars=True)
    lx.whitespace_split = True
    lx.commenters = "#"
    try:
        toks = list(lx)
    except ValueError:
        return []
    out, argv, redir, i = [], [], [], 0
    while i < len(toks):
        t = toks[i]
        if t in (";", "&&", "||", "|", "&", "(", ")"):
            if argv:
                out.append((argv, redir))
            argv, redir = [], []
        elif t in (">", ">|", "&>"):
            if i + 1 < len(toks):
                redir.append(toks[i + 1])
            i += 1
        elif t in (">>", "<", "<<", "2>", ">&"):
            i += 1
        else:
            argv.append(";" if t == "__ESC_SEMI__" else t)
        i += 1
    if argv:
        out.append((argv, redir))
    return out


def shell_effects(cmdline: str, cwd: str) -> Optional[tuple[str, str]]:
    """(level, reason) for the worst file-system effect of a typed shell command, or None."""
    worst = None
    for argv, redir in _commands(cmdline):
        r = None
        for tgt0 in redir:
            tgt = _p(tgt0, cwd)
            if tgt0 != "/dev/null" and _nonempty_file(tgt):
                r = ("high", f"`>` would overwrite existing file {os.path.relpath(tgt, cwd)}")
        while argv and (argv[0] in ("sudo", "env", "nohup", "time", "do", "then") or
                        ("=" in argv[0] and not argv[0].startswith("-"))):
            argv = argv[1:]
        tool = os.path.basename(argv[0]) if argv else ""
        try:
            if r:
                pass
            elif tool in ("cp", "mv", "rsync", "install"):
                r = _cp_like(argv, cwd)
            elif tool == "find":
                r = _find_exec(argv, cwd)
            elif tool == "chmod":
                r = _chmod(argv, cwd)
            elif tool == "rm":
                tg = [a for a in argv[1:] if not a.startswith("-")]
                if any(os.path.exists(x) for x in _expand(tg, cwd)):
                    r = ("high" if any("r" in a.lower() for a in argv[1:] if a.startswith("-")) else "medium",
                         f"`rm` deletes {' '.join(tg[:3])}")
        except Exception:  # a check must never break the action
            r = None
        if r and (worst is None or (r[0] == "high" and worst[0] != "high")):
            worst = r
    return worst


def overwrite_dialog(blob: str, label: str, keys: set) -> bool:
    """True when the visible window asks to replace an existing file and this action says yes."""
    if not blob or not OVERWRITE_CTX.search(blob):
        return False
    lab = (label or "").strip().lower().replace("_", "").replace("&", "")
    return lab in CONFIRM_LABELS or bool(keys & {"return", "enter", "kp_enter", "y", "alt+y"})
