#!/usr/bin/env bash
# doctor.sh — diagnose why RadialDeck hotkeys or the app do not work
# Usage: ./scripts/doctor.sh
#
# Read-only by default. Pass --fix to restart the portal (session-scoped, no
# config is written outside the user's own directories).
set -uo pipefail

FIX=0
for arg in "$@"; do
    case "$arg" in
        --fix) FIX=1 ;;
        -h|--help)
            echo "Usage: $0 [--fix]"
            echo "  --fix   restart xdg-desktop-portal so it re-detects the desktop"
            exit 0 ;;
        *) echo "Unknown argument: $arg" >&2; exit 2 ;;
    esac
done

fail=0
warn=0

ok()   { printf '  \033[32m✔\033[0m %s\n' "$1"; }
bad()  { printf '  \033[31m✘\033[0m %s\n' "$1"; fail=$((fail + 1)); }
note() { printf '  \033[33m!\033[0m %s\n' "$1"; warn=$((warn + 1)); }

echo "RadialDeck doctor"
echo

# ── 1. Binary ────────────────────────────────────────────────────────────────
echo "Binary"
if command -v radial-deck >/dev/null 2>&1; then
    ok "radial-deck found at $(command -v radial-deck)"
else
    bad "radial-deck is not on PATH (install with scripts/install.sh)"
fi

# ── 2. Process ───────────────────────────────────────────────────────────────
echo "Process"
if pgrep -f '(^|/)radial-deck( |$)' >/dev/null 2>&1; then
    ok "running (pid $(pgrep -f '(^|/)radial-deck( |$)' | head -1))"
else
    note "not running — no process can hold the hotkey; start it or enable Launch at login"
fi

# ── 3. Autostart ─────────────────────────────────────────────────────────────
echo "Autostart"
autostart="${XDG_CONFIG_HOME:-$HOME/.config}/autostart/io.github.raihan.RadialDeck.desktop"
if [[ -f $autostart ]]; then
    ok "$autostart"
else
    note "no autostart entry — hotkeys will not return after reboot"
fi

# ── 4. Portal interface ──────────────────────────────────────────────────────
# `busctl … | grep -q` would fail under `pipefail`: grep exits as soon as it
# matches, busctl dies of SIGPIPE, and the pipeline reports failure even though
# the interface is present. Capture the listing first, then match on it.
portal_interfaces=""
has_global_shortcuts() {
    if ! command -v busctl >/dev/null 2>&1; then
        return 1
    fi
    if [[ -z $portal_interfaces ]]; then
        portal_interfaces=$(busctl --user introspect org.freedesktop.portal.Desktop \
            /org/freedesktop/portal/desktop 2>/dev/null)
    fi
    [[ $portal_interfaces == *GlobalShortcuts* ]]
}

echo "Portal"
if ! command -v busctl >/dev/null 2>&1; then
    note "busctl not available; skipping portal checks"
elif has_global_shortcuts; then
    ok "org.freedesktop.portal.GlobalShortcuts is exported"
else
    bad "GlobalShortcuts is NOT exported — the portal fell back to a backend without it"
    if [[ $FIX -eq 1 ]]; then
        echo "     restarting xdg-desktop-portal…"
        systemctl --user restart xdg-desktop-portal
        sleep 3
        portal_interfaces=""
        if has_global_shortcuts; then
            ok "GlobalShortcuts is exported after the restart"
            echo "     restart RadialDeck so it can register the shortcuts again"
        else
            bad "still missing after restart — check the desktop environment"
        fi
    else
        echo "     run '$0 --fix' to restart the portal"
    fi
fi

# ── 5. Desktop pin ───────────────────────────────────────────────────────────
dropin="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/xdg-desktop-portal.service.d/desktop-env.conf"
echo "Persistence"
if [[ -f $dropin ]]; then
    ok "desktop pinned for the portal ($dropin)"
else
    note "no systemd drop-in — the portal may lose GlobalShortcuts again at login"
    echo "     see the Troubleshooting section in README.md"
fi

echo
if [[ $fail -eq 0 && $warn -eq 0 ]]; then
    echo "Everything looks healthy."
else
    printf '%d problem(s), %d warning(s).\n' "$fail" "$warn"
fi
[[ $fail -eq 0 ]]
