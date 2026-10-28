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
#   settings - the normal toplevel still maps in the nested session
#   projects - the Projects page lists the folders of the active project
#   dark     - the same window under the dark preference
#   cli      - `radial-deck convert` runs without any display
#
# Usage: scripts/ui-smoke.sh
#
# Artifacts stay in $RADIAL_DECK_SMOKE_OUT (default /tmp/radial-deck-ui-smoke):
# screenshots, one log per app run and the compositor log. Config, state and
# cache are redirected inside $OUT, so the real profile is never written. Only
# the private runtime/cache directories and the scratch files are removed.
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

# Two concurrent runs would fight over the fixed socket name and the nested
# Xwayland display, and the geometry-based display lookup could attach to the
# other run's server. Refuse to overlap instead of producing phantom failures.
exec 9>"${TMPDIR:-/tmp}/radial-deck-ui-smoke.lock"
if ! flock -n 9; then
    say "another ui-smoke run is already using this machine; wait for it to finish"
    exit 1
fi

rm -rf "$OUT"
mkdir -p "$OUT/runtime" "$OUT/shots" "$OUT/work" "$OUT/config" "$OUT/state" "$OUT/cache"
chmod 700 "$OUT/runtime"

export XDG_RUNTIME_DIR="$OUT/runtime"
# Config and state (history, logs) must not touch the real profile: the test
# app writes on save and records every output it creates. Without these two the
# run would leak into ~/.config/radial-deck and ~/.local/state/radial-deck.
export XDG_CONFIG_HOME="$OUT/config"
export XDG_STATE_HOME="$OUT/state"
export XDG_CACHE_HOME="$OUT/cache"
export XDG_CURRENT_DESKTOP=KDE
export RADIAL_DECK_NO_HOTKEY=1

# KWin 6.4 routes XTEST injection through the EIS portal and silently drops it
# for clients it has not been told about. `xdotool key` then reports success
# while nothing reaches the app, which turns every keyboard check into a false
# failure. The allowlist normally lives in the real kwinrc; this run has its own
# empty config home, so write it here instead of reading the user's profile.
cat >"$OUT/config/kwinrc" <<'EOF'
[Xwayland]
XwaylandEisNoPromptApps=xdotool
EOF

KWIN_PID=""
APP_PID=""
APP_LANE=0
APP_LOG="$OUT/app.log"
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
        # Processes can exit between the glob and the read; that is fine.
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
    disown "$BUS_PID" 2>/dev/null
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
    reap_private_bus 2>/dev/null
    [[ -n "$BUS_PID" ]] && kill -9 "$BUS_PID" 2>/dev/null
    reap_private_bus 2>/dev/null
    # Waiting on a killed compositor can hang on its still-open pipe, so reap
    # with a deadline instead of a bare `wait`.
    for _ in $(seq 20); do
        kill -0 "$KWIN_PID" 2>/dev/null || break
        sleep 0.1
    done
    rm -rf "$OUT/runtime" "$OUT/work" "$OUT/bus" "$OUT/cache"
}
trap cleanup EXIT

start_compositor() {
    kwin_wayland --virtual --xwayland --socket wayland-rdtest \
        --width 1280 --height 800 --no-lockscreen >"$OUT/kwin.log" 2>&1 &
    KWIN_PID=$!
    disown "$KWIN_PID" 2>/dev/null
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
    if [[ -n "$APP_PID" ]]; then
        kill -9 "$APP_PID" 2>/dev/null
        wait "$APP_PID" 2>/dev/null
        APP_PID=""
    fi
    # Keep each lane's log: a crash in one lane must not be erased by the next
    # start, and the filename makes the failing flow obvious from the artifacts.
    APP_LANE=$((APP_LANE + 1))
    APP_LOG="$OUT/app-$APP_LANE.log"
    env GDK_BACKEND="$backend" "$BIN" "$@" >"$APP_LOG" 2>&1 &
    APP_PID=$!
    # Without disown, bash prints "Killed" for every replaced app and clutters
    # the report; the PID is still ours to manage.
    disown "$APP_PID" 2>/dev/null
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

log_clean() { ! grep -qE 'assertion .* failed|init_layer_shell' "$APP_LOG"; }
app_alive() { kill -0 "$APP_PID" 2>/dev/null; }

x11_focus() {
    local window
    window="$(xdotool search --onlyvisible --name RadialDeck | head -1)"
    [[ -n "$window" ]] || return 1
    # `windowactivate` goes through the WM and KWin may refuse it under focus
    # stealing prevention, which silently breaks every later key press. Setting
    # the input focus directly is deterministic inside this throwaway session.
    xdotool windowraise "$window" 2>/dev/null
    xdotool windowfocus --sync "$window" 2>/dev/null
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
    grep -q 'tool compress finished: 1 created, 0 failed' "$APP_LOG"
# Recording history used to re-enter the history borrow and abort the process
# right after a successful run, so liveness here is a real regression check.
check "x11: process survives a completed tool run" app_alive
check "x11: no panic after a completed tool run" \
    bash -c "! grep -qE 'panicked|already mutably borrowed' '$APP_LOG'"

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

# --- lane 3: live-modifier switching ---------------------------------------
# Tangerine-style gesture: while the wheel is open, a bare modifier key
# switches mode. xdotool sends the key to the focused overlay, exactly like a
# real key press; the controller logs the switch at INFO so the check is
# deterministic instead of comparing screenshots.
start_app x11 --actions "$OUT/work/photo.png"
x11_focus
sleep 1
modifier_switch() {
    xdotool keydown "$1" >/dev/null 2>&1
    sleep 0.4
    xdotool keyup "$1" >/dev/null 2>&1
    sleep 0.8
    grep -q "live-modifier switch to $2" "$APP_LOG"
}
check "x11: Shift switches to the Convert wheel" \
    modifier_switch shift Convert
check "x11: Control switches to the Actions wheel" \
    modifier_switch ctrl Actions
check "x11: Super switches to the Tools wheel" \
    modifier_switch super Tools
check "x11: Alt also switches to the Tools wheel" \
    modifier_switch alt Tools
check "x11: sources survive a modifier switch" \
    bash -c "grep -q 'show actions: mode=Tools sources=\[\"$OUT/work/photo.png\"\]' '$APP_LOG' || ! grep -q 'show actions: mode=Tools sources=\[\]' '$APP_LOG'"
send_key Escape
sleep 1

# --- lane 4: settings window ------------------------------------------------
# The settings window is a regular toplevel, not a layer-shell surface, so it
# also proves the app still maps a normal window inside the nested session.
start_app wayland --settings
sleep 1
shoot settings
sleep 0.5
check "wayland: settings window draws" \
    test "$(drawn_pixels settings | cut -d. -f1)" -gt 20000
# A Wayland toplevel is not visible to xdotool, so liveness plus a clean log
# is the meaningful check here; the screenshot above proves it rendered.
check "wayland: settings process stays alive" app_alive
check "wayland: settings log has no panic" \
    bash -c "! grep -qE 'panicked|SIGABRT' '$APP_LOG'"

# The sidebar is keyboard-driven: the window focuses the page list on open, so
# Down moves the selection and switches pages without depending on pixels.
start_app x11 --settings
check "x11: settings window is mapped" x11_focus
sleep 1
shoot settings-general
for page in shortcuts projects tools output advanced; do
    send_key Down
    sleep 1
    shoot "settings-$page"
done
# Every Down press must land on a different page: comparing consecutive
# captures catches a stuck selection, which a single diff would not.
check "x11: Down walks every settings page" \
    bash -c "prev=general
    for page in shortcuts projects tools output advanced; do
        compare -metric AE \"$OUT/shots/settings-\$prev.png\" \"$OUT/shots/settings-\$page.png\" null: 2>&1 | grep -qx '0' && exit 1
        prev=\$page
    done; exit 0"
check "x11: settings survives page navigation" app_alive
check "x11: no panic after page navigation" \
    bash -c "! grep -qE 'panicked|SIGABRT' '$APP_LOG'"

# --- lane 5: seeded project list --------------------------------------------
# The Projects page must show folders, not just counts. Seeding goes through
# the config file because the Manage window's dialogs need a real portal.
mkdir -p "$OUT/work/docs" "$OUT/work/assets"
python3 - "$XDG_CONFIG_HOME/radial-deck/config.json" "$OUT/work" <<'PY'
import json, sys, uuid
path, work = sys.argv[1], sys.argv[2]
with open(path) as handle:
    config = json.load(handle)
project = config["projects"][0]
project["folders"] = [
    {"id": str(uuid.uuid4()), "label": label, "path": f"{work}/{name}", "color": None}
    for label, name in (("Documents", "docs"), ("Assets", "assets"))
]
with open(path, "w") as handle:
    json.dump(config, handle, indent=2)
PY
start_app x11 --settings
check "x11: seeded settings window is mapped" x11_focus
sleep 1
send_key Down
sleep 0.5
send_key Down                 # General -> Shortcuts -> Projects
sleep 1
shoot settings-projects-seeded
# Two conditions: the window really is on Projects (not the start page), and
# the seeded folders changed what that page draws.
check "x11: seeded project list is on the Projects page" \
    bash -c "! compare -metric AE '$OUT/shots/settings-general.png' '$OUT/shots/settings-projects-seeded.png' null: 2>&1 | grep -qx '0'"
check "x11: project list renders its folders" \
    bash -c "! compare -metric AE '$OUT/shots/settings-projects.png' '$OUT/shots/settings-projects-seeded.png' null: 2>&1 | grep -qx '0'"

# --- lane 6: dark theme -----------------------------------------------------
# The settings CSS mixes theme colours (@theme_fg_color) with translucent
# overlays, so both variants must resolve; a light-only check would miss a
# white-on-white regression.
sed -i 's/"theme": "system"/"theme": "dark"/' "$XDG_CONFIG_HOME/radial-deck/config.json"
start_app x11 --settings
check "x11: dark settings window is mapped" x11_focus
sleep 1
shoot settings-dark
check "x11: dark preference darkens the window" \
    bash -c "[[ \$(magick '$OUT/shots/settings-dark.png' -colorspace gray -format '%[fx:mean]' info:) < \$(magick '$OUT/shots/settings-general.png' -colorspace gray -format '%[fx:mean]' info:) ]]"
check "x11: no panic in dark theme" \
    bash -c "! grep -qE 'panicked|SIGABRT' '$APP_LOG'"
sed -i 's/"theme": "dark"/"theme": "system"/' "$XDG_CONFIG_HOME/radial-deck/config.json"

# --- lane 7: headless batch converter --------------------------------------
# The CLI path must not need GTK at all: it runs here with the same private
# environment but is a separate process, so a GTK init regression would hang
# or fail the exit status.
batch_dir="$OUT/work/batch"
mkdir -p "$batch_dir/out"
cp "$OUT/work/photo.png" "$batch_dir/one.png"
if "$BIN" convert --to webp --into "$batch_dir/out" "$batch_dir/one.png" \
    >"$OUT/batch.log" 2>&1; then
    batch_rc=0
else
    batch_rc=$?
fi
check "cli: batch conversion exits 0" test "$batch_rc" -eq 0
check "cli: batch conversion wrote one.webp" test -f "$batch_dir/out/one.webp"
if "$BIN" convert --to webp --into "$batch_dir/out" "$batch_dir/one.png" \
    >>"$OUT/batch.log" 2>&1; then
    batch_rc=0
else
    batch_rc=$?
fi
check "cli: a second run never overwrites" test -f "$batch_dir/out/one-1.webp"
if "$BIN" convert --to definitely-not-a-format "$batch_dir/one.png" \
    >>"$OUT/batch.log" 2>&1; then
    bad_rc=0
else
    bad_rc=$?
fi
check "cli: an unknown target exits 2" test "$bad_rc" -eq 2
if "$BIN" convert --help >"$OUT/batch-help.log" 2>&1; then
    help_rc=0
else
    help_rc=$?
fi
check "cli: convert --help exits 0" test "$help_rc" -eq 0

say ""
say "checks: $CHECKS, failures: $FAILURES"
[[ $FAILURES -eq 0 ]]
