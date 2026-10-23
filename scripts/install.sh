#!/usr/bin/env bash
# install.sh — install RadialDeck to ~/.local (no sudo required)
# Usage: ./scripts/install.sh [--prefix PREFIX] [--release | --debug]
set -euo pipefail

PREFIX="${HOME}/.local"
PROFILE="release"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --prefix) PREFIX="$2"; shift 2 ;;
        --debug)  PROFILE="debug";   shift ;;
        --release) PROFILE="release"; shift ;;
        -h|--help)
            echo "Usage: $0 [--prefix ~/.local] [--release|--debug]"
            exit 0 ;;
        *) echo "Unknown argument: $1"; exit 1 ;;
    esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$PREFIX/bin"
SHARE="$PREFIX/share"
APPS="$SHARE/applications"
ICONS="$SHARE/icons/hicolor/scalable/apps"

# ── Check dependencies ────────────────────────────────────────────────────────
missing=()
for pkg in gtk4 xdg-utils; do
    pkg-config --exists "$pkg" 2>/dev/null || missing+=("$pkg dev headers")
done
if ! pkg-config --exists gtk4-layer-shell-0 2>/dev/null; then
    missing+=("gtk4-layer-shell")
fi
if [[ ${#missing[@]} -gt 0 ]]; then
    echo "ERROR: missing dependencies: ${missing[*]}"
    echo "  Install with: sudo pacman -S gtk4 gtk4-layer-shell"
    exit 1
fi

# ── Build ─────────────────────────────────────────────────────────────────────
echo "Building RadialDeck ($PROFILE)…"
cd "$ROOT"
if [[ "$PROFILE" == "release" ]]; then
    cargo build --release --locked
else
    cargo build --locked
fi

# ── Install binary ────────────────────────────────────────────────────────────
install -Dm755 "target/$PROFILE/radial-deck" "$BIN/radial-deck"

# ── Install desktop entry (Exec = absolute path for portal app ID lookup) ─────
mkdir -p "$APPS"
sed "s|Exec=radial-deck|Exec=$BIN/radial-deck|" \
    packaging/io.github.raihan.RadialDeck.desktop \
    > "$APPS/io.github.raihan.RadialDeck.desktop"
chmod 644 "$APPS/io.github.raihan.RadialDeck.desktop"
update-desktop-database "$APPS" 2>/dev/null || true

# ── Install icon ──────────────────────────────────────────────────────────────
install -Dm644 packaging/io.github.raihan.RadialDeck.svg \
    "$ICONS/io.github.raihan.RadialDeck.svg"

# ── Done ──────────────────────────────────────────────────────────────────────
cat <<MSG

✔ RadialDeck installed to $PREFIX

  Run once to register global shortcut:
    radial-deck --hidden

  Or open the launcher immediately:
    radial-deck --show

  To uninstall:
    $ROOT/scripts/uninstall.sh --prefix $PREFIX
MSG
