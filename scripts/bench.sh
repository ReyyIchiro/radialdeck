#!/usr/bin/env bash
# Performance spot-checks with a stable, display-free methodology.
#
# Measured (release binary):
#   size    - stripped binary size in bytes
#   startup - best wall time of `radial-deck --version` (argv path, no GTK)
#   list    - best wall time of `radial-deck convert --list` (engine cold path)
#   convert - throughput of a 24-image batch conversion, ms per image
#
# First run writes scripts/bench-baseline.json. Later runs compare against it
# and fail when any metric regresses more than BENCH_TOLERANCE (default 25%).
# Rewrite the baseline after an intentional change: RADIAL_DECK_BENCH_REGEN=1.
# Numbers are machine-specific; the baseline is a local contract, not a
# portable score. Idle RSS needs a real session and is documented in
# docs/architecture.md instead of measured here.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${RADIAL_DECK_BIN:-$ROOT/target/release/radial-deck}"
BASELINE="${RADIAL_DECK_BENCH_FILE:-$ROOT/scripts/bench-baseline.json}"
TOLERANCE="${BENCH_TOLERANCE:-25}"

[[ -x "$BIN" ]] || { echo "missing release binary: $BIN (run cargo build --release)"; exit 1; }
command -v magick >/dev/null || { echo "missing required tool: magick"; exit 1; }
command -v python3 >/dev/null || { echo "missing required tool: python3"; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

measure() {
    python3 - "$BIN" "$WORK" <<'PY'
import json
import statistics
import subprocess
import sys
import time
from pathlib import Path

binary, work = Path(sys.argv[1]), Path(sys.argv[2])

def best_ms(command, runs):
    # Best-of-N: spawn scheduling noise on a busy desktop only ever adds time,
    # so the minimum is the stable number for latency (median is not).
    samples = []
    for _ in range(runs):
        start = time.perf_counter()
        subprocess.run(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)
        samples.append((time.perf_counter() - start) * 1000.0)
    return round(min(samples), 2)

size = binary.stat().st_size
startup = best_ms([str(binary), "--version"], 20)
listing = best_ms([str(binary), "convert", "--list"], 20)

source = work / "bench-src"
source.mkdir()
for index in range(24):
    subprocess.run(
        ["magick", "-size", "320x200", f"gradient:blue-red", str(source / f"img-{index:02}.png")],
        check=True, stdout=subprocess.DEVNULL,
    )
# Median of three batch runs: a single run on a busy desktop swings with
# whatever else the machine is doing, the median does not.
samples = []
for _ in range(3):
    start = time.perf_counter()
    subprocess.run(
        [str(binary), "convert", "--to", "webp", "--subfolder", "out", *sorted(source.glob("*.png"))],
        stdout=subprocess.DEVNULL, check=True,
    )
    samples.append((time.perf_counter() - start) * 1000.0 / 24)
convert_ms = round(statistics.median(samples), 2)

print(json.dumps({
    "size": size,
    "startup_ms": startup,
    "list_ms": listing,
    "convert_ms_per_image": convert_ms,
}))
PY
}

CURRENT="$(measure)"

if [[ "${RADIAL_DECK_BENCH_REGEN:-0}" == "1" || ! -s "$BASELINE" ]]; then
    printf '%s\n' "$CURRENT" > "$BASELINE"
    echo "baseline written: $BASELINE"
    echo "$CURRENT"
    exit 0
fi

python3 - "$BASELINE" "$CURRENT" "$TOLERANCE" <<'PY'
import json
import sys

baseline = json.loads(open(sys.argv[1]).read())
current = json.loads(sys.argv[2])
tolerance = float(sys.argv[3])

failures = []
for key, value in current.items():
    base = baseline.get(key)
    if base is None:
        print(f"NEW    {key}: {value}")
        continue
    limit = float(base) * (1 + tolerance / 100.0)
    if value <= limit:
        print(f"OK     {key}: {value} (baseline {base}, limit {limit:.0f})")
    else:
        print(f"FAIL   {key}: {value} (baseline {base}, limit {limit:.0f})")
        failures.append(key)

if failures:
    print(f"\nregression in: {', '.join(failures)}")
    print("if the change is intentional, rewrite the baseline with RADIAL_DECK_BENCH_REGEN=1")
    raise SystemExit(1)
PY
