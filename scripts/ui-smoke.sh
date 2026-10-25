#!/usr/bin/env bash
# UI smoke test that never touches the real desktop.
#
# The application is started inside a nested KWin virtual session
# (`kwin_wayland --virtual`), so overlays, layer-shell surfaces and the Float
# shelf are rendered into an offscreen framebuffer. Screenshots are taken with
# spectacle against that nested compositor and input is injected through the
# nested Xwayland server, which leaves the physical screen and pointer alone.
#
# The run also gets its own D-Bus session bus. Without it the application would
# register a tray icon and post notifications on the real desktop, and every
# service it activates would inherit this script's stdout, keeping a caller's
# pipe open long after the checks finish.
#
# Lanes:
#   wayland  - layer-shell overlay actually maps and draws (screenshot proof)
#   x11      - keyboard-driven flows: tool form submission, Float, Esc
#
# Usage: scripts/ui-smoke.sh
#
# Artifacts stay in $RADIAL_DECK_SMOKE_OUT (default /tmp/radial-deck-ui-smoke):
# screenshots, the app log and the compositor log. Only the private runtime
# directory and the scratch conversion files are removed.
#
# Exit status: 0 when every check passes, 1 otherwise.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/target/release/radial-deck"
OUT="${RADIAL_DECK_SMOKE_OUT:-${TMPDIR:-/tmp}/radial-deck-ui-smoke}"

FAILURES=0
CHECKS=0

say() { printf '%s\n' "$*"; }
pass() { CHECKS=$((CHECKS + 1)); printf 'PASS  %s\n' "$*"; }
fail() { CHECKS=$((CHECKS + 1)); FAILURES=$((FAILURES + 1)); printf 'FAIL  %s\n' "$*"; }

check() {
    local description="$1"
    shift
    if "$@" >/dev/null 2>&1; then pass "$description"; else fail "$description"; fi
}

[[ -x "$BIN" ]] || { say "missing release binary: $BIN (run cargo build --release)"; exit 1; }
for tool in kwin_wayland spectacle xdotool magick compare; do
    command -v "$tool" >/dev/null || { say "missing required tool: $tool"; exit 1; }
done

rm -rf "$OUT"
mkdir -p "$OUT/runtime" "$OUT/shots" "$OUT/work"
chmod 700 "$OUT/runtime"

export XDG_RUNTIME_DIR="$OUT/runtime"
export XDG_CURRENT_DESKTOP=KDE
export RADIAL_DECK_NO_HOTKEY=1

KWIN_PID=""
APP_PID=""
BUS_PID=""
BUS_ADDRESS="unix:path=$OUT/bus"

# Kills every process that belongs to this run: the compositor's helpers, portal
# services, KDE Connect, spectacle's background daemon. They daemonise and get
# reparented, so they outlive the bus and keep a caller's stdout pipe open.
#
# The bus address is matched as a prefix because D-Bus appends ",guid=...".
# XDG_RUNTIME_DIR is matched as a second criterion; both are private to the run.
reap_private_bus() {
    local proc pid environ
    for proc in /proc/[0-9]*; do
        pid="${proc##*/}"
        # This script exports both variables too; never reap itself.
        [[ "$pid" == "$$" ]] && continue
        environ="$proc/environ"
        [[ -r "$environ" ]] || continue
        environ="$(tr '\0' '\n' <"$environ" 2>/dev/null)" || continue
        if grep -qF "DBUS_SESSION_BUS_ADDRESS=$BUS_ADDRESS" <<<"$environ" ||
            grep -qxF "XDG_RUNTIME_DIR=$OUT/runtime" <<<"$environ"; then
            kill -9 "$pid" 2>/dev/null
        fi
    done
}

start_private_bus() {
    dbus-daemon --session --address="$BUS_ADDRESS" --nofork --nopidfile \
        >"$OUT/bus.log" 2>&1 &
    BUS_PID=$!
    for _ in $(seq 50); do
        [[ -S "$OUT/bus" ]] && break
        sleep 0.1
    done
    [[ -S "$OUT/bus" ]] || { say "private session bus did not start"; return 1; }
    export DBUS_SESSION_BUS_ADDRESS="$BUS_ADDRESS"
}

cleanup() {
    [[ -n "$APP_PID" ]] && kill -9 "$APP_PID" 2>/dev/null
    [[ -n "$KWIN_PID" ]] && kill -9 "$KWIN_PID" 2>/dev/null
    # Services can be reactivated while the bus is still alive, so reap, drop the
    # bus, then reap once more for anything that came up in between.
    reap_private_bus
    [[ -n "$BUS_PID" ]] && kill -9 "$BUS_PID" 2>/dev/null
    reap_private_bus
    # Waiting on a killed compositor can hang on its still-open pipe, so reap
    # with a deadline instead of a bare `wait`.
    for _ in $(seq 20); do
        kill -0 "$KWIN_PID" 2>/dev/null || break
        sleep 0.1
    done
    rm -rf "$OUT/runtime" "$OUT/work" "$OUT/bus"
}
trap cleanup EXIT

start_compositor() {
    kwin_wayland --virtual --xwayland --socket wayland-rdtest \
        --width 1280 --height 800 --no-lockscreen >"$OUT/kwin.log" 2>&1 &
    KWIN_PID=$!
    for _ in $(seq 100); do
        [[ -S "$XDG_RUNTIME_DIR/wayland-rdtest" ]] && break
        sleep 0.1
    done
    [[ -S "$XDG_RUNTIME_DIR/wayland-rdtest" ]] || { say "kwin did not start"; return 1; }

    # Xwayland picks the next free display number, and stale sockets from dead
    # servers can linger, so identify the nested server by its geometry.
    for _ in $(seq 150); do
        for socket in /tmp/.X11-unix/X*; do
            local display=":${socket##*/X}"
            if DISPLAY="$display" timeout 2 xdotool getdisplaygeometry 2>/dev/null | grep -q '^1280 800$'; then
                NESTED_DISPLAY="$display"
                return 0
            fi
        done
        sleep 0.1
    done
    say "nested Xwayland display not found"
    return 1
}

start_app() {
    local backend="$1"
    shift
    [[ -n "$APP_PID" ]] && { kill -9 "$APP_PID" 2>/dev/null; APP_PID=""; }
    env GDK_BACKEND="$backend" "$BIN" "$@" >"$OUT/app.log" 2>&1 &
    APP_PID=$!
    sleep 3
}

shoot() {
    local name="$1"
    timeout 20 spectacle -b -n -f -o "$OUT/shots/$name.png" \
        >"$OUT/shots/$name.log" 2>&1
    local status=$?
    [[ -s "$OUT/shots/$name.png" ]] && return 0
    say "capture '$name' failed (spectacle exit $status): $(tail -1 "$OUT/shots/$name.log" 2>/dev/null)"
    return 1
}

# Non-black pixel count: the nested framebuffer is black, so anything drawn by
# the application raises it well above zero.
drawn_pixels() {
    magick "$OUT/shots/$1.png" -colorspace gray -threshold 12% -format '%[fx:mean*w*h]' info:
}

log_clean() { ! grep -qE 'assertion .* failed|init_layer_shell' "$OUT/app.log"; }
app_alive() { kill -0 "$APP_PID" 2>/dev/null; }

x11_focus() {
    local window
    window="$(xdotool search --onlyvisible --name RadialDeck | head -1)"
    [[ -n "$window" ]] || return 1
    xdotool windowactivate --sync "$window" 2>/dev/null
}

send_key() { xdotool key --clearmodifiers "$1"; }

say "artifact directory: $OUT"

start_private_bus || exit 1
start_compositor || exit 1
# Point every child, including spectacle, at the nested session. Inheriting the
# real WAYLAND_DISPLAY would make captures silently target a socket that does
# not exist under this private XDG_RUNTIME_DIR.
export WAYLAND_DISPLAY=wayland-rdtest DISPLAY="$NESTED_DISPLAY"
say "nested session: WAYLAND_DISPLAY=$WAYLAND_DISPLAY DISPLAY=$DISPLAY"

# --- lane 1: layer-shell overlay on Wayland ---------------------------------
cp "$ROOT/target/smoke/photo.png" "$OUT/work/photo.png" 2>/dev/null ||
    magick -size 96x64 gradient:navy-orange "$OUT/work/photo.png"

start_app wayland --actions "$OUT/work/photo.png"
check "wayland: overlay process stays alive" app_alive
check "wayland: no layer-shell assertion in log" log_clean
shoot actions
sleep 0.5
actions_pixels="$(drawn_pixels actions)"
check "wayland: actions overlay draws on the virtual output" \
    test "${actions_pixels%.*}" -gt 20000

start_app wayland --tools "$OUT/work/photo.png"
shoot tools
sleep 0.5
check "wayland: tools wheel draws" test "$(drawn_pixels tools | cut -d. -f1)" -gt 20000
check "wayland: tools and actions differ" \
    bash -c "[[ -s '$OUT/shots/actions.png' && -s '$OUT/shots/tools.png' ]] && \
        ! compare -metric AE '$OUT/shots/actions.png' '$OUT/shots/tools.png' null: 2>&1 | grep -qx '0'"

# --- lane 2: keyboard flows through the nested Xwayland server ---------------
start_app x11 --tools "$OUT/work/photo.png"
check "x11: overlay window is mapped" x11_focus
send_key 2                    # wedge 2 is Compress for images
sleep 1
send_key ctrl+a
xdotool type --delay 40 '45'
send_key Return
sleep 4
check "x11: compress form wrote photo-compressed.png" test -f "$OUT/work/photo-compressed.png"
check "x11: compressed output keeps the source dimensions" \
    bash -c "[[ \$(magick identify -format '%wx%h' '$OUT/work/photo-compressed.png') == \$(magick identify -format '%wx%h' '$OUT/work/photo.png') ]]"
check "x11: compress run is logged as finished" \
    grep -q 'tool compress finished: 1 created, 0 failed' "$OUT/app.log"

start_app x11 --actions "$OUT/work/photo.png"
check "x11: actions overlay is mapped" x11_focus
send_key 6                    # wedge 6 is Float
sleep 2
shelf_window="$(xdotool search --onlyvisible --name '^Float$' | head -1)"
check "x11: Float opens a shelf window" test -n "$shelf_window"
check "x11: shelf is 340x420" \
    bash -c "xdotool getwindowgeometry '$shelf_window' | grep -q 'Geometry: 340x420'"
send_key Escape
sleep 1
check "x11: Escape hides the overlay" \
    bash -c '[[ -z "$(xdotool search --onlyvisible --name RadialDeck)" ]]' 
check "x11: process survives the overlay cycle" app_alive
check "x11: no layer-shell assertion in log" log_clean

say ""
say "checks: $CHECKS, failures: $FAILURES"
[[ $FAILURES -eq 0 ]]
