#!/usr/bin/env bash
# The fork's benchmark: four workloads driven through run() as a board drives the
# chip, in pieces of at most a Mega Drive scanline (228 input clocks), the first ten
# emulated seconds of each:
#  - the reference's benchmark stream, the seed-1 random walk of 40,000 latches, on
#    the YM7101;
#  - where the exported board streams are present (local, since they are derived from
#    ROMs): Gunstar Heroes on the YM7101, Sonic the Hedgehog on the Master System,
#    Columns on the Game Gear (stereo).
# Prints each workload, a check over all their samples, and "total S s".
# Usage: scripts/bench.sh [sn76489-bench options]   env: BENCH_BIN (another build)
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
REF=${SN76489_PYTHON:-"$ROOT/external/sn76489-python"}
OUT="$ROOT/external/manifests"
STREAMS=${STREAMS:-"$ROOT/../sn76489-python/validation/streams"}
if [ ! -f "$OUT/bench/bench-random-seed-1-40000-ym7101.json" ]; then
    "$REF/.venv/bin/python" "$ROOT/scripts/stream_manifests.py" --reference "$REF" --bench --out "$OUT/bench" > /dev/null
fi
workloads=("$OUT/bench/bench-random-seed-1-40000-ym7101.json")
if [ ! -f "$OUT/board/genesis-gunstar-heroes-usa-ym7101.json" ] && [ -d "$STREAMS" ]; then
    "$REF/.venv/bin/python" "$ROOT/scripts/stream_manifests.py" --reference "$REF" --streams "$STREAMS" --out "$OUT/board" > /dev/null
fi
for name in genesis-gunstar-heroes-usa-ym7101 \
    sms2-sonic-the-hedgehog-usa-europe-brazil-en-sega-vdp \
    gamegear-columns-usa-europe-brazil-en-game-gear; do
    [ ! -f "$OUT/board/$name.json" ] || workloads+=("$OUT/board/$name.json")
done
if [ -z "${BENCH_BIN:-}" ]; then
    cargo build --release -q --manifest-path "$ROOT/Cargo.toml" --bin sn76489-bench
    BENCH_BIN="$ROOT/target/release/sn76489-bench"
fi
exec "$BENCH_BIN" "$@" "${workloads[@]}"
