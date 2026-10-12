#!/usr/bin/env bash
# Rung 6, the fork's own: long runs equal clocks one at a time. Every manifest of
# rungs 2-5 (the examples, synthetic, random and, where present, board streams) is
# run a clock at a time and in seeded random pieces, and the two must leave the same
# state at every boundary and the same samples to the bit. Rungs 1-5 judge the
# clock-at-a-time run against the reference, since the trace runs one clock at a
# time; this rung carries that over to the long runs a board makes.
# Usage: scripts/rung6.sh [seed, default 1]   (JOBS=16 by default)
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
REF=${SN76489_PYTHON:-"$ROOT/external/sn76489-python"}
if [ -z "${CHUNKS_BIN:-}" ]; then
    cargo build --release -q --manifest-path "$ROOT/Cargo.toml"
    CHUNKS_BIN="$ROOT/target/release/sn76489-chunks"
fi
shopt -s nullglob
manifests=("$REF"/examples/conformance/*.json "$ROOT"/external/manifests/{synthetic,random,board}/*.json)
results=$(printf '%s\0' "${manifests[@]}" | nice xargs -0 -n1 -P "${JOBS:-16}" "$CHUNKS_BIN" --seed "${1:-1}" | grep -v "^TOTAL" || true)
echo "$results" | grep -v "boundaries agree" || true
same=$(grep -c "boundaries agree" <<<"$results" || true)
echo "TOTAL: $same of ${#manifests[@]} manifests agree, $(grep -o '[0-9]* boundaries' <<<"$results" | awk '{s+=$1} END {print s}') boundaries"
[ "$same" -eq "${#manifests[@]}" ]
