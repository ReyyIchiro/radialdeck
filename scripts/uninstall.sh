#!/usr/bin/env bash
# uninstall.sh — remove a user-local RadialDeck installation
set -euo pipefail

PREFIX="${HOME}/.local"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --prefix) PREFIX="$2"; shift 2 ;;
        *) echo "Unknown: $1"; exit 1 ;;
    esac
done

rm -fv "$PREFIX/bin/radial-deck"
rm -fv "$PREFIX/share/applications/io.github.raihan.RadialDeck.desktop"
rm -fv "$PREFIX/share/icons/hicolor/scalable/apps/io.github.raihan.RadialDeck.svg"
rm -fv "$PREFIX/share/kio/servicemenus/io.github.raihan.RadialDeck.desktop"
kbuildsycoca6 >/dev/null 2>&1 || true
update-desktop-database "$PREFIX/share/applications" 2>/dev/null || true
echo "RadialDeck uninstalled from $PREFIX"
