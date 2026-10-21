#!/usr/bin/env bash
# gui-gate installer (macOS / Linux): the gate binary from this repo's GitHub release,
# plus the latest Cua Driver through Cua's own official installer.
#
#   curl -fsSL https://github.com/cszf1/gui-gate/releases/latest/download/install.sh | bash
#   curl -fsSL .../install.sh | bash -s -- --client claude      # also add it to Claude Code
#
# Flags (or env):
#   --client NAME      claude|codex|cursor|pi (repeatable; GUI_GATE_CLIENT=claude,codex)
#   --version X.Y.Z    pin a gui-gate release              (GUI_GATE_VERSION)
#   --bin-dir PATH     where gui-gate goes, default ~/.local/bin (GUI_GATE_INSTALL_DIR)
#   --skip-driver      do not install / update Cua Driver   (GUI_GATE_SKIP_DRIVER=1)
#   --no-modify-path   do not add the bin dir to your shell profile
#   GUI_GATE_BASE_URL  download gui-gate from this URL instead of GitHub (a mirror)
# Cua Driver's own env vars (e.g. CUA_DRIVER_RS_VERSION=0.38.0) are passed through to its installer.
set -euo pipefail

REPO="cszf1/gui-gate"
BIN_DIR="${GUI_GATE_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${GUI_GATE_VERSION:-}"
SKIP_DRIVER="${GUI_GATE_SKIP_DRIVER:-0}"
NO_MODIFY_PATH=0
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
        --skip-driver) SKIP_DRIVER=1; shift ;;
        --no-modify-path) NO_MODIFY_PATH=1; shift ;;
        -h|--help) sed -n '2,16p' "$0" 2>/dev/null || true; exit 0 ;;
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
mkdir -p "$BIN_DIR"
install -m 0755 "$tmp/gui-gate" "$BIN_DIR/.gui-gate.new"
mv -f "$BIN_DIR/.gui-gate.new" "$BIN_DIR/gui-gate"   # atomic swap; a running gate keeps its old copy
say "installed gui-gate $("$BIN_DIR/gui-gate" --version 2>/dev/null || true) -> $BIN_DIR/gui-gate"

if [[ "$SKIP_DRIVER" != "1" ]]; then
    say "installing the latest Cua Driver with Cua's official installer"
    drv_args=()
    [[ "$BIN_DIR" != "$HOME/.local/bin" ]] && drv_args+=(--bin-dir "$BIN_DIR")
    [[ "$NO_MODIFY_PATH" == "1" ]] && drv_args+=(--no-modify-path)
    drv_script="$(curl -fsSL --retry 3 https://cua.ai/driver/install.sh)" || die "could not download Cua Driver's installer"
    bash -c "$drv_script" cua-driver-install ${drv_args[@]+"${drv_args[@]}"} </dev/null \
        || die "Cua Driver's installer failed (see above). gui-gate itself is installed; re-run this command to retry."
fi

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *)
        if [[ "$NO_MODIFY_PATH" != "1" ]]; then
            line="export PATH=\"$BIN_DIR:\$PATH\""
            case "$(basename "${SHELL:-bash}")" in
                zsh) rc="$HOME/.zshrc" ;;
                bash) if [[ "$os" == "Darwin" ]]; then rc="$HOME/.bash_profile"; else rc="$HOME/.bashrc"; fi ;;
                *) rc="$HOME/.profile" ;;
            esac
            grep -qsF "$line" "$rc" || printf '\n# gui-gate\n%s\n' "$line" >> "$rc"
            say "added $BIN_DIR to PATH in your shell profile (open a new terminal)"
        fi
        export PATH="$BIN_DIR:$PATH" ;;
esac

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
