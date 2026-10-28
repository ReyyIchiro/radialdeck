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
SERVICEMENUS="$SHARE/kio/servicemenus"

# ── Check dependencies ────────────────────────────────────────────────────────
missing=()
pkg-config --exists gtk4 2>/dev/null || missing+=("gtk4")
pkg-config --exists gtk4-layer-shell-0 2>/dev/null || missing+=("gtk4-layer-shell")
command -v xdg-open >/dev/null 2>&1 || missing+=("xdg-utils")
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

# ── Install Dolphin service menu (right-click → RadialDeck actions) ───────────
mkdir -p "$SERVICEMENUS"
sed "s|Exec=radial-deck|Exec=$BIN/radial-deck|" \
    packaging/io.github.raihan.RadialDeck.servicemenu.desktop \
    > "$SERVICEMENUS/io.github.raihan.RadialDeck.desktop"
chmod 644 "$SERVICEMENUS/io.github.raihan.RadialDeck.desktop"
kbuildsycoca6 >/dev/null 2>&1 || true

# ── Done ──────────────────────────────────────────────────────────────────────
cat <<MSG

✔ RadialDeck installed to $PREFIX

  Start it in the background (tray + global shortcuts):
    $BIN/radial-deck --hidden

  Enable "Launch at login" in Settings so the hotkeys survive a reboot.
  Right-click any file in Dolphin → "RadialDeck actions…"

  If a hotkey does nothing, run the diagnostics:
    $ROOT/scripts/doctor.sh

  To uninstall:
    $ROOT/scripts/uninstall.sh --prefix $PREFIX
MSG
