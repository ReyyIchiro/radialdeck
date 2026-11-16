#!/usr/bin/env bash
# Render packaging/aur/PKGBUILD.tpl into packaging/aur/PKGBUILD for one release.
#
# Usage:
#   scripts/make-release.sh --version 0.3.0 [--output DIR]
#
# The script downloads the release source tarball from GitHub Releases,
# computes its sha256, and substitutes PKGVER and SHA256 in the template.
# It writes PKGBUILD and regenerates .SRCINFO via makepkg when available.

set -euo pipefail

REPO_URL="https://github.com/ReyyIchiro/radialdeck"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TPL="$ROOT/packaging/aur/PKGBUILD.tpl"
OUT="$ROOT/packaging/aur"

usage() {
    sed -n '2,10p' "$0"
    exit "${1:-0}"
}

VERSION=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="${2:?--version requires a value}"; shift 2 ;;
        --output) OUT="${2:?--output requires a value}"; shift 2 ;;
        -h|--help) usage 0 ;;
        *) echo "unknown option: $1" >&2; usage 1 ;;
    esac
done

if [[ -z "$VERSION" ]]; then
    echo "error: --version is required" >&2
    usage 1
fi

if [[ ! -f "$TPL" ]]; then
    echo "error: template not found: $TPL" >&2
    exit 1
fi

mkdir -p "$OUT"

TARBALL="radialdeck-${VERSION}.tar.gz"
URL="$REPO_URL/releases/download/v${VERSION}/${TARBALL}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Downloading $URL"
if ! curl -fL "$URL" -o "$TMP/$TARBALL"; then
    echo "error: could not download $URL" >&2
    echo "hint: ensure the v${VERSION} release exists and the asset name matches" >&2
    exit 1
fi

SHA="$(sha256sum "$TMP/$TARBALL" | awk '{print $1}')"
echo "sha256($TARBALL) = $SHA"

sed -e "s|{{PKGVER}}|${VERSION}|g" \
    -e "s|{{SHA256}}|${SHA}|g" \
    "$TPL" > "$OUT/PKGBUILD"

if command -v makepkg >/dev/null 2>&1; then
    echo "Regenerating .SRCINFO"
    (cd "$OUT" && makepkg --printsrcinfo > .SRCINFO)
else
    echo "warn: makepkg not found; .SRCINFO not regenerated" >&2
fi

echo "Wrote $OUT/PKGBUILD"
