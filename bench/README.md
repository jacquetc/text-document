# bench/

Benchmark and memory-profile harness for the rope-backend migration. Captures
before/after measurements so the migration's gains are measured, not assumed.

## Layout

```
bench/
  compare.sh                  # capture + compare driver
  baseline/                   # raw outputs from the initial `pre-rope` capture
  results/
    memory-<name>.txt         # raw memory_profile output for a label
    memory-<name>.md          # rendered comparison table (hand-curated)
    criterion-<a>-vs-<b>.txt  # criterion --baseline output
    criterion-<name>.md       # headline summary (hand-curated)
```

A `<name>` is a label like `pre-rope`, `post-phase-1`, `post-phase-2`.

## Usage

```bash
# 1. On main, capture the pre-rope baseline.
./bench/compare.sh pre-rope pre-rope

# 2. After phase 1 (InlineElement removed), capture and compare.
./bench/compare.sh pre-rope post-phase-1

# 3. After phase 2 (rope swap), capture and compare against either baseline.
./bench/compare.sh pre-rope         post-phase-2
./bench/compare.sh post-phase-1     post-phase-2
```

The first argument is the criterion baseline to compare against; the second
is the label saved for this run. Pass the same label twice on the first run
to seed the baseline without a comparison step.

## Fairness conditions

Keep these consistent across baseline and post-migration captures:

- **Build**: release profile with workspace defaults (`lto = "fat"`,
  `codegen-units = 1`, `panic = "abort"`). `compare.sh` runs
  `cargo build --release --workspace` first.
- **Sample size**: criterion defaults (100 samples / 5 s per bench).
- **Proptest seed**: `PROPTEST_RNG_SEED=20260515` exported by `compare.sh`.
- **OS state**: close foreground apps; on macOS, disable App Nap for the
  cargo process or run from a Terminal session marked "Prevent App Nap";
  on Linux, set the CPU governor to `performance`.
- **Hardware**: same machine for both runs. Record the host below.

## Recorded host

Baseline `pre-rope` was captured on:

- **OS**: macOS 26.5 (Darwin 25.5.0 ARM64)
- **CPU**: Apple M4
- **RAM**: 16 GiB
- **Toolchain**: see `rust-toolchain.toml` if present; otherwise
  `rustc --version` at the time of capture (record below).

When re-running on a different host, append a new section here rather than
overwriting; comparisons across hosts are not meaningful.

## Memory profile vs criterion

- `memory_profile` is run as a release example
  (`cargo run --release --example memory_profile -p text-document`).
  It uses a counting `GlobalAlloc`; output is plain text with `live=`/`peak=`
  lines per scenario.
- Criterion benches in `crates/public_api/benches/{benchmarks,io_benchmarks}.rs`
  capture wall-clock timings. The `--save-baseline NAME` and `--baseline NAME`
  flags produce comparable runs across migrations.

## Pre-existing baseline suite (do not duplicate)

`crates/public_api/benches/benchmarks.rs` already provides groups:
`creation`, `editing`, `navigation`, `history`, `formatting`, `tables`,
`lists`, `session`.

`crates/public_api/benches/io_benchmarks.rs` already provides:
`plain_text`, `rich_text`, `snapshots`.

New groups added for the migration measure scenarios these don't cover
(large-document inserts, format-run-heavy workloads, rope-friendly
worst cases). See plan §6.2.
