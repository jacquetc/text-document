#!/usr/bin/env bash
# Capture memory + criterion measurements and compare against a baseline.
#
# Usage:
#   ./bench/compare.sh <baseline-label> <new-label>
#
# Examples:
#   ./bench/compare.sh pre-rope pre-rope            # seed the baseline
#   ./bench/compare.sh pre-rope post-phase-1
#   ./bench/compare.sh post-phase-1 post-phase-2
#
# Run from the workspace root.

set -euo pipefail

if [ "$#" -lt 2 ]; then
    echo "usage: $0 <baseline-label> <new-label>" >&2
    exit 2
fi

BASELINE="$1"
NEW="$2"

# Deterministic proptest fuzzing across runs.
export PROPTEST_RNG_SEED="${PROPTEST_RNG_SEED:-20260515}"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

RESULTS="bench/results"
mkdir -p "$RESULTS"

echo "== rustc version =="
rustc --version | tee "$RESULTS/rustc-${NEW}.txt"

echo
echo "== Building release workspace =="
cargo build --release --workspace

echo
echo "== Memory profile [$NEW] =="
cargo run --release --example memory_profile -p text-document \
    | tee "$RESULTS/memory-${NEW}.txt"

echo
echo "== Criterion: benchmarks (saving as $NEW) =="
cargo bench -p text-document --bench benchmarks -- --save-baseline "$NEW"

echo
echo "== Criterion: io_benchmarks (saving as $NEW) =="
cargo bench -p text-document --bench io_benchmarks -- --save-baseline "$NEW"

if [ "$BASELINE" != "$NEW" ]; then
    echo
    echo "== Criterion comparison: $BASELINE -> $NEW =="
    cargo bench -p text-document --bench benchmarks -- --baseline "$BASELINE" \
        | tee "$RESULTS/criterion-${BASELINE}-vs-${NEW}.txt"
    cargo bench -p text-document --bench io_benchmarks -- --baseline "$BASELINE" \
        | tee -a "$RESULTS/criterion-${BASELINE}-vs-${NEW}.txt"
else
    echo
    echo "Baseline seeded. Re-run with a different second argument to compare."
fi

echo
echo "Done. Outputs:"
ls -la "$RESULTS" | grep "${NEW}"
