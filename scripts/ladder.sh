#!/usr/bin/env bash
# Every rung, in order, against the reference checkout in external/: the gate for
# every commit that touches src/. Rung 4 needs the exported board streams (local);
# SKIP4=1 leaves it out, and then this is not "the whole ladder".
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
REF=${SN76489_PYTHON:-"$ROOT/external/sn76489-python"}
echo "commit $(git -C "$ROOT" rev-parse --short HEAD)$(git -C "$ROOT" diff --quiet -- src || echo ' + working tree'), reference $(git -C "$REF" rev-parse --short HEAD)"
start=$(date +%s)
cargo test --release -q --manifest-path "$ROOT/Cargo.toml" 2>&1 | grep -E "^test result: .* [1-9][0-9]* passed" || { echo "cargo test failed"; exit 1; }
rung() {  # a rung's total line; a rung that fails stops the ladder
    local out
    if out=$("$ROOT/scripts/rung$1.sh" 2>&1); then
        echo "rung $1: $(tail -n 1 <<<"$out")"
    else
        echo "rung $1 FAILED:"; grep -v "traces are identical\| identical$" <<<"$out" | tail -n 8
        exit 1
    fi
}
rung 1
rung 2
rung 3
rung 5
[ "${SKIP4:-0}" = 1 ] || rung 4
rung 6
echo "ladder clean in $(( $(date +%s) - start )) s"
