#!/usr/bin/env bash
# gui-gate installer (macOS / Linux): ONE archive from this repo's GitHub release holds the gate binary and the
# Cua Driver build it was tested with (official, unmodified, MIT; verified against Cua's SHA256SUMS + Sigstore when
# the release was made). Cua's own installer is never run.
#   gui-gate      -> ~/.local/bin/gui-gate
#   Cua Driver    -> ~/.local/lib/gui-gate/cua-driver/   (Linux)
#                    /Applications/CuaDriver.app         (macOS; Cua's signed app, so its Accessibility / Screen
#                    Recording permissions work as with Cua's installer; an existing CuaDriver.app is left alone)
#   uninstall: gui-gate uninstall  (removes all of the above that this installer created)
#
#   curl -fsSL https://github.com/cszf1/gui-gate/releases/latest/download/install.sh | bash
#   curl -fsSL .../install.sh | bash -s -- --client claude      # also add it to Claude Code
#
# Flags (or env):
#   --client NAME      claude|codex|cursor|pi (repeatable; GUI_GATE_CLIENT=claude,codex)
#   --version X.Y.Z    pin a gui-gate release              (GUI_GATE_VERSION)
#   --bin-dir PATH     where gui-gate goes, default ~/.local/bin (GUI_GATE_INSTALL_DIR)
#   --lib-dir PATH     where the bundled Cua Driver goes, default <bin-dir>/../lib/gui-gate (GUI_GATE_LIB_DIR)
#   --skip-driver      do not install the bundled Cua Driver; use one installed separately (GUI_GATE_SKIP_DRIVER=1)
#   --no-modify-path   do not add the bin dir to your shell profile
#   GUI_GATE_BASE_URL  download gui-gate from this URL instead of GitHub (a mirror)
# The gate needs a reviewer model (it refuses to start without one). Unless one is already configured the
# installer asks for it on a terminal, or takes it from these (saved to ~/.config/gui-gate/reviewer.env):
#   --reviewer-url URL          base URL or full endpoint          (GUI_GATE_REVIEWER_URL)
#   --reviewer-model NAME       model name                         (GUI_GATE_REVIEWER_MODEL)
#   --reviewer-key-file PATH    file holding only the API key      (GUI_GATE_REVIEWER_KEY_FILE;
#                               or GUI_GATE_REVIEWER_KEY=..., saved to ~/.config/gui-gate/reviewer.key, mode 600)
#   --reviewer-protocol P       auto|chat|responses|anthropic|gemini|systemone (GUI_GATE_REVIEWER_PROTOCOL, default auto)
#   --no-prompt                 never ask on the terminal          (GUI_GATE_NO_PROMPT=1)
set -euo pipefail

REPO="cszf1/gui-gate"
BIN_DIR="${GUI_GATE_INSTALL_DIR:-$HOME/.local/bin}"
LIB_DIR="${GUI_GATE_LIB_DIR:-}"
VERSION="${GUI_GATE_VERSION:-}"
SKIP_DRIVER="${GUI_GATE_SKIP_DRIVER:-0}"
NO_MODIFY_PATH=0
NO_PROMPT="${GUI_GATE_NO_PROMPT:-0}"
[[ -n "${CI:-}" ]] && NO_PROMPT=1   # never wait for a terminal answer in CI
R_URL="${GUI_GATE_REVIEWER_URL:-}"
R_MODEL="${GUI_GATE_REVIEWER_MODEL:-}"
R_KEY_FILE="${GUI_GATE_REVIEWER_KEY_FILE:-}"
R_PROTOCOL="${GUI_GATE_REVIEWER_PROTOCOL:-}"
CLIENTS=()
if [[ -n "${GUI_GATE_CLIENT:-}" ]]; then IFS=',' read -r -a CLIENTS <<< "$GUI_GATE_CLIENT"; fi

while [[ $# -gt 0 ]]; do
    case "$1" in
        --client) CLIENTS+=("$2"); shift 2 ;;
        --client=*) CLIENTS+=("${1#*=}"); shift ;;
        --version) VERSION="$2"; shift 2 ;;
        --version=*) VERSION="${1#*=}"; shift ;;
        --bin-dir) BIN_DIR="$2"; shift 2 ;;
        --bin-dir=*) BIN_DIR="${1#*=}"; shift ;;
        --lib-dir) LIB_DIR="$2"; shift 2 ;;
        --lib-dir=*) LIB_DIR="${1#*=}"; shift ;;
        --skip-driver) SKIP_DRIVER=1; shift ;;
        --no-modify-path) NO_MODIFY_PATH=1; shift ;;
        --reviewer-url) R_URL="$2"; shift 2 ;;
        --reviewer-url=*) R_URL="${1#*=}"; shift ;;
        --reviewer-model) R_MODEL="$2"; shift 2 ;;
        --reviewer-model=*) R_MODEL="${1#*=}"; shift ;;
        --reviewer-key-file) R_KEY_FILE="$2"; shift 2 ;;
        --reviewer-key-file=*) R_KEY_FILE="${1#*=}"; shift ;;
        --reviewer-protocol) R_PROTOCOL="$2"; shift 2 ;;
        --reviewer-protocol=*) R_PROTOCOL="${1#*=}"; shift ;;
        --no-prompt) NO_PROMPT=1; shift ;;
        -h|--help) sed -n '2,32p' "$0" 2>/dev/null || true; exit 0 ;;
        *) echo "gui-gate install: unknown argument $1" >&2; exit 2 ;;
    esac
done

say() { printf '\033[1m==> %s\033[0m\n' "$*" >&2; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }
command -v curl >/dev/null || die "curl is required"
command -v tar >/dev/null || die "tar is required"

os="$(uname -s)"; arch="$(uname -m)"
case "$os" in
    Linux)
        case "$arch" in
            x86_64|amd64) asset="gui-gate-linux-x86_64.tar.gz" ;;
            aarch64|arm64) asset="gui-gate-linux-aarch64.tar.gz" ;;
            *) die "no prebuilt gui-gate for Linux $arch" ;;
        esac ;;
    Darwin) asset="gui-gate-macos-universal.tar.gz" ;;
    MINGW*|MSYS*|CYGWIN*) die "on Windows use PowerShell: irm https://github.com/$REPO/releases/latest/download/install.ps1 | iex" ;;
    *) die "unsupported OS $os" ;;
esac

if [[ -n "${GUI_GATE_BASE_URL:-}" ]]; then base="$GUI_GATE_BASE_URL"   # mirror / local test
elif [[ -n "$VERSION" ]]; then base="https://github.com/$REPO/releases/download/v${VERSION#v}"
else base="https://github.com/$REPO/releases/latest/download"; fi

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
say "downloading $asset"
curl -fsSL --retry 3 -o "$tmp/$asset" "$base/$asset" || die "download failed: $base/$asset"
curl -fsSL --retry 3 -o "$tmp/SHA256SUMS" "$base/SHA256SUMS" || die "download failed: $base/SHA256SUMS"
want="$(awk -v f="$asset" '$2==f || $2=="*"f {print $1}' "$tmp/SHA256SUMS")"
[[ -n "$want" ]] || die "$asset is not listed in SHA256SUMS"
if command -v sha256sum >/dev/null; then got="$(sha256sum "$tmp/$asset" | awk '{print $1}')"
else got="$(shasum -a 256 "$tmp/$asset" | awk '{print $1}')"; fi
[[ "$want" == "$got" ]] || die "checksum mismatch for $asset"

tar -xzf "$tmp/$asset" -C "$tmp"
[[ -z "$LIB_DIR" ]] && LIB_DIR="$(dirname "$BIN_DIR")/lib/gui-gate"
MANIFEST="$LIB_DIR/install-manifest"
# remember which dirs this installer creates (only under $HOME, only if missing now), so `gui-gate uninstall` can
# remove exactly those once they are empty; an upgrade keeps what the first install recorded
created=()
if [[ -f "$MANIFEST" ]]; then while IFS= read -r l; do [[ "$l" == created_dir=* ]] && created+=("${l#created_dir=}"); done < "$MANIFEST"; fi
note_dir() {
    local d="$1"
    while [[ -n "$d" && "$d" != "/" && "$d" != "$HOME" && "$d" == "$HOME"/* ]]; do
        if [[ ! -e "$d" ]]; then created+=("$d"); fi
        d="$(dirname "$d")"
    done
}
note_dir "$BIN_DIR"; note_dir "$LIB_DIR"; note_dir "$HOME/.config/gui-gate"; note_dir "$HOME/.cache/gui-gate"
app_line=""
owned=()
if [[ -f "$MANIFEST" ]]; then
    app_line="$(grep '^app=' "$MANIFEST" | tail -n 1 || true)"
    while IFS= read -r l; do [[ "$l" == owned_dir=* ]] && owned+=("${l#owned_dir=}"); done < "$MANIFEST"
fi

mkdir -p "$BIN_DIR"
install -m 0755 "$tmp/gui-gate" "$BIN_DIR/.gui-gate.new"
mv -f "$BIN_DIR/.gui-gate.new" "$BIN_DIR/gui-gate"   # atomic swap; a running gate keeps its old copy
say "installed gui-gate $("$BIN_DIR/gui-gate" --version 2>/dev/null || true) -> $BIN_DIR/gui-gate"

# ------------------------------------------------------------------ the bundled Cua Driver
install_app() {  # macOS: Cua's signed CuaDriver.app, copied byte for byte (ditto) and verified, never re-signed
    local src="$tmp/CuaDriver.app" dest="$1" ver have backup=""
    ver="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$src/Contents/Info.plist" 2>/dev/null || echo "?")"
    codesign --verify --deep --strict "$src" 2>/dev/null || die "the bundled CuaDriver.app failed its signature check; nothing was changed"
    if [[ -e "$dest" ]]; then
        have="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$dest/Contents/Info.plist" 2>/dev/null || echo "")"
        if [[ "$have" == "$ver" ]] && codesign --verify --deep --strict "$dest" 2>/dev/null; then
            say "Cua Driver $ver is already in place at $dest"; return 0
        fi
        "$dest/Contents/MacOS/cua-driver" stop >/dev/null 2>&1 || true
        backup="$dest.gui-gate-backup.$$"; mv "$dest" "$backup"
    fi
    mkdir -p "$(dirname "$dest")"
    if ditto "$src" "$dest" && codesign --verify --deep --strict "$dest" 2>/dev/null; then
        if [[ -n "${backup:-}" ]]; then rm -rf "$backup"; fi
    else
        rm -rf "$dest"; if [[ -n "${backup:-}" ]]; then mv "$backup" "$dest"; fi
        die "could not install CuaDriver.app at $dest (rolled back)"
    fi
    /System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister -f "$dest" >/dev/null 2>&1 || true
    say "installed Cua Driver $ver -> $dest"
}
if [[ "$SKIP_DRIVER" != "1" && -d "$tmp/cua-driver" ]]; then
    mkdir -p "$LIB_DIR"
    rm -rf "$LIB_DIR/.cua-driver.new" "$LIB_DIR/cua-driver.old"
    cp -R "$tmp/cua-driver" "$LIB_DIR/.cua-driver.new"
    if [[ -e "$LIB_DIR/cua-driver" ]]; then mv "$LIB_DIR/cua-driver" "$LIB_DIR/cua-driver.old"; fi
    mv "$LIB_DIR/.cua-driver.new" "$LIB_DIR/cua-driver"
    rm -rf "$LIB_DIR/cua-driver.old"
    drv_ver="$(cat "$LIB_DIR/cua-driver/VERSION" 2>/dev/null || echo "?")"
    if [[ "$os" == "Darwin" ]]; then
        app_dest="/Applications/CuaDriver.app"
        [[ -w /Applications ]] || app_dest="$HOME/Applications/CuaDriver.app"
        if [[ -n "$app_line" && "${app_line#app=}" != "$app_dest" && -e "${app_line#app=}" ]]; then app_dest="${app_line#app=}"; fi
        if [[ -e "$app_dest" && "$app_line" != "app=$app_dest" ]]; then
            say "a Cua Driver app is already installed at $app_dest (not by gui-gate): left untouched and used as is"
            app_line=""
        else
            if [[ "$app_dest" == "$HOME/Applications/CuaDriver.app" ]]; then note_dir "$HOME/Applications"; fi
            # the app keeps its daemon socket / pid in ~/Library/Caches/cua-driver (Cua has no setting to move it);
            # record ~/Library/Caches and ~/Library too when they do not exist yet (a fresh or throwaway HOME), so
            # uninstall removes them once empty; on a real Mac both always exist and stay
            note_dir "$HOME/Library/Caches"
            # state the app creates on first use; removed by `gui-gate uninstall` only if it is not there yet
            for d in "$HOME/.cua-driver" "$HOME/Library/Caches/cua-driver" "$HOME/Library/Application Support/Cua Driver"; do
                [[ -e "$d" ]] || owned+=("$d")
            done
            install_app "$app_dest"
            app_line="app=$app_dest"
        fi
    else
        say "installed Cua Driver $drv_ver (bundled) -> $LIB_DIR/cua-driver"
    fi
    for c in "$HOME/.local/bin/cua-driver" "$HOME/.cua-driver/packages/current/cua-driver"; do
        if [[ -e "$c" ]]; then say "note: a Cua Driver installed by Cua's own installer is at $c; it is left untouched (gui-gate uses its bundled one; GUI_GATE_DRIVER=external selects yours)"; break; fi
    done
elif [[ "$SKIP_DRIVER" == "1" ]]; then
    say "--skip-driver: not installing the bundled Cua Driver; gui-gate looks for one installed separately"
fi
# PATH: exactly one shell profile gets the line - the one this shell reads at login/startup:
#   zsh -> ~/.zshrc; bash on Linux -> ~/.bashrc; bash on macOS (Terminal opens login shells, which read the first of
#   ~/.bash_profile, ~/.bash_login, ~/.profile and never ~/.bashrc) -> the first of those that exists; else ~/.profile.
# A profile file that does not exist is created holding only our two lines and recorded in the manifest
# (created_file=), so `gui-gate uninstall` deletes it again when nothing else was added to it.
created_files=()
if [[ -f "$MANIFEST" ]]; then while IFS= read -r l; do [[ "$l" == created_file=* && -e "${l#created_file=}" ]] && created_files+=("${l#created_file=}"); done < "$MANIFEST"; fi
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *)
        if [[ "$NO_MODIFY_PATH" != "1" ]]; then
            line="export PATH=\"$BIN_DIR:\$PATH\""
            case "$(basename "${SHELL:-bash}")" in
                zsh) rc="$HOME/.zshrc" ;;
                bash)
                    if [[ "$os" == "Darwin" ]]; then
                        rc="$HOME/.bash_profile"
                        for f in "$HOME/.bash_profile" "$HOME/.bash_login" "$HOME/.profile"; do
                            if [[ -e "$f" ]]; then rc="$f"; break; fi
                        done
                    else
                        rc="$HOME/.bashrc"
                    fi ;;
                *) rc="$HOME/.profile" ;;
            esac
            if [[ ! -e "$rc" ]]; then
                printf '# gui-gate\n%s\n' "$line" > "$rc"
                created_files+=("$rc")
            elif ! grep -qsF "$line" "$rc"; then
                printf '\n# gui-gate\n%s\n' "$line" >> "$rc"
            fi
            say "added $BIN_DIR to PATH in $rc (open a new terminal)"
        fi
        export PATH="$BIN_DIR:$PATH" ;;
esac

mkdir -p "$LIB_DIR"
{
    echo "# written by gui-gate's installer; read by \`gui-gate uninstall\`"
    printf '%s\n' "${created[@]+"${created[@]}"}" | awk 'NF && !seen[$0]++ {print "created_dir=" $0}'
    printf '%s\n' "${created_files[@]+"${created_files[@]}"}" | awk 'NF && !seen[$0]++ {print "created_file=" $0}'
    if [[ -n "$app_line" ]]; then
        echo "$app_line"
        printf '%s\n' "${owned[@]+"${owned[@]}"}" | awk 'NF && !seen[$0]++ {print "owned_dir=" $0}'
    fi
} > "$MANIFEST.tmp"
mv -f "$MANIFEST.tmp" "$MANIFEST"

# leave no residue: remove what older versions left (old copies, old Python package, stale PATH lines,
# stale MCP entries and flags, our own config backups, old caches); `gui-gate cleanup --dry-run` shows it
"$BIN_DIR/gui-gate" cleanup --quiet || true

# ------------------------------------------------------------------ reviewer model (required)
GG="$BIN_DIR/gui-gate"
CFG_DIR="$HOME/.config/gui-gate"
REV_ENV="${GUI_GATE_REVIEWER_ENV:-$CFG_DIR/reviewer.env}"
reviewer_ok() { "$GG" check-reviewer --config-only >/dev/null 2>&1; }
# set KEY=VALUE in the reviewer.env file (other lines kept), file mode 600
set_rev() {
    local k="$1" v="$2" f="$REV_ENV"
    mkdir -p "$(dirname "$f")"; chmod 700 "$(dirname "$f")" 2>/dev/null || true
    touch "$f"; chmod 600 "$f"
    grep -v "^\(export \)\?$k=" "$f" > "$f.tmp" || true
    printf '%s=%s\n' "$k" "$v" >> "$f.tmp"
    mv -f "$f.tmp" "$f"; chmod 600 "$f"
}
save_key() {  # the key -> ~/.config/gui-gate/reviewer.key (mode 600); never echoed (printf is a builtin)
    local kf="$CFG_DIR/reviewer.key"
    mkdir -p "$CFG_DIR"; chmod 700 "$CFG_DIR" 2>/dev/null || true
    ( umask 077; printf '%s\n' "$1" > "$kf" ); chmod 600 "$kf"
    R_KEY_FILE="$kf"
}
abs_path() { case "$1" in "~/"*) printf '%s\n' "$HOME/${1#\~/}" ;; /*) printf '%s\n' "$1" ;; *) printf '%s\n' "$PWD/$1" ;; esac; }
write_reviewer() {
    [[ -n "$R_URL" ]] && set_rev GUI_GATE_REVIEWER_URL "$R_URL"
    [[ -n "$R_MODEL" ]] && set_rev GUI_GATE_REVIEWER_MODEL "$R_MODEL"
    [[ -n "$R_KEY_FILE" ]] && set_rev GUI_GATE_REVIEWER_KEY_FILE "$(abs_path "$R_KEY_FILE")"
    if [[ -n "$R_PROTOCOL" && "$R_PROTOCOL" != "auto" ]]; then set_rev GUI_GATE_REVIEWER_PROTOCOL "$R_PROTOCOL"; fi
    say "saved the reviewer settings to $REV_ENV"
}
# from here on check what was saved, not this shell's environment (the key never stays in env)
clear_rev_env() { unset GUI_GATE_REVIEWER GUI_GATE_REVIEWER_URL GUI_GATE_REVIEWER_MODEL GUI_GATE_REVIEWER_KEY GUI_GATE_REVIEWER_KEY_FILE GUI_GATE_REVIEWER_PROTOCOL; }
if [[ -n "${GUI_GATE_REVIEWER_KEY:-}" && -z "$R_KEY_FILE" ]]; then
    save_key "$GUI_GATE_REVIEWER_KEY"
fi
if [[ -n "$R_URL$R_MODEL$R_KEY_FILE$R_PROTOCOL" ]]; then
    write_reviewer   # given by flags / env: these replace what was saved before
    clear_rev_env
elif ! reviewer_ok && [[ "$NO_PROMPT" != "1" ]] && (exec </dev/tty) 2>/dev/null; then
    {
        echo
        echo "gui-gate needs a reviewer model: an independent model that checks risky or doubtful steps"
        echo "(any LLM API - OpenAI-compatible chat, OpenAI responses, Anthropic, Gemini - or a System One"
        echo "decision model). The gate does not start without one. Leave the URL empty to skip for now."
    } >&2
    read -r -p "Reviewer API URL (e.g. https://api.deepseek.com/v1): " R_URL </dev/tty || R_URL=""
    if [[ -n "$R_URL" ]]; then
        read -r -p "Model name (e.g. deepseek-chat): " R_MODEL </dev/tty || R_MODEL=""
        read -r -s -p "API key (hidden; saved to $CFG_DIR/reviewer.key, mode 600): " key </dev/tty || key=""
        echo >&2
        if [[ -n "$key" ]]; then save_key "$key"; fi
        unset key
        guess="$(GUI_GATE_REVIEWER_ENV=/nonexistent GUI_GATE_REVIEWER_URL="$R_URL" GUI_GATE_REVIEWER_MODEL="${R_MODEL:-x}" \
                 GUI_GATE_REVIEWER_KEY=x "$GG" check-reviewer --config-only 2>/dev/null | sed -n 's/.*"reviewer":"api:\([a-z]*\):.*/\1/p')"
        read -r -p "Protocol [auto = ${guess:-chat}] (chat|responses|anthropic|gemini|systemone): " R_PROTOCOL </dev/tty || R_PROTOCOL=""
        write_reviewer
        clear_rev_env
    fi
fi
if reviewer_ok; then
    say "checking the reviewer model: $("$GG" check-reviewer --config-only 2>/dev/null | sed -n 's/.*"reviewer":"\([^"]*\)".*/\1/p')"
    "$GG" check-reviewer || echo "warning: the reviewer is configured but did not pass its check (see above); fix the settings in $REV_ENV" >&2
else
    printf '\033[1;31mwarning: no reviewer model is configured: the gate will NOT start until one is set.\033[0m\n' >&2
    "$GG" check-reviewer --config-only >/dev/null || true
    echo "Set GUI_GATE_REVIEWER_URL / _MODEL / _KEY_FILE in $REV_ENV (or re-run this installer on a terminal, or with" >&2
    echo "--reviewer-url URL --reviewer-model NAME --reviewer-key-file PATH), then run: gui-gate check-reviewer" >&2
fi

if [[ ${#CLIENTS[@]} -gt 0 ]]; then
    setup_args=()
    for c in "${CLIENTS[@]}"; do setup_args+=(--client "$c"); done
    say "adding gui-gate to ${CLIENTS[*]}"
    "$BIN_DIR/gui-gate" setup "${setup_args[@]}" || true
else
    say "checking the install"
    "$BIN_DIR/gui-gate" doctor || true
    echo
    echo "Next: add it to your agent, e.g.   gui-gate setup --client claude   (or codex / cursor / pi)"
fi
if [[ "$os" == "Darwin" ]]; then
    echo "macOS: the first GUI action asks for Accessibility and Screen Recording permission for CuaDriver; allow both."
fi
echo "Remove everything this installed: gui-gate uninstall"
