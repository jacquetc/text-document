//! Regression guard for `shift_after`'s complexity (regression
//! review "Cause C"). `shift_after` must be sub-linear when no
//! entries actually need shifting — the `partition_point`
//! implementation skips the entire entries vec via O(log n) binary
//! search. This test guards against a regression back to the
//! pre-fix O(N) full-scan.
//!
//! Strategy: call `shift_after` with a threshold past every entry's
//! `byte_start`, so no entry should be mutated. Time it against
//! indexes of very different sizes. With the O(log n) implementation
//! the ratio stays near 1; a regression to O(N) sends it past 50×.
//!
//! `std::hint::black_box` is used to defeat release-mode constant
//! propagation that would otherwise elide the call entirely.

use common::database::block_offset_index::BlockOffsetIndex;
use std::hint::black_box;
use std::time::{Duration, Instant};

fn build_index(n: u32) -> BlockOffsetIndex {
    let mut idx = BlockOffsetIndex::new();
    for i in 0..n {
        idx.push_block(i as u64, i * 100);
    }
    idx.set_total_bytes(n * 100);
    idx
}

/// Time `iters` calls to `shift_after` with a threshold past every
/// entry (so the iteration scans them all and updates none). Uses
/// `black_box` on inputs and result so the compiler can't fold the
/// call away.
fn time_noop_shifts(n: u32, iters: u32) -> Duration {
    let mut idx = build_index(n);
    // Warm up to let the allocator and CPU caches settle.
    for _ in 0..50 {
        idx.shift_after(black_box(u32::MAX), black_box(1));
    }
    let start = Instant::now();
    for _ in 0..iters {
        idx.shift_after(black_box(u32::MAX), black_box(1));
        black_box(&idx);
    }
    start.elapsed()
}

/// `shift_after` must be sub-linear when no entries actually need
/// shifting (threshold past the last entry). The `partition_point`
/// implementation skips the entire entries vec via O(log n) binary
/// search; this test guards against a regression back to the
/// pre-fix O(N) full-scan.
#[test]
fn shift_after_noop_does_not_scale_with_entry_count() {
    const ITERS: u32 = 1_000;
    let t_small = time_noop_shifts(1_000, ITERS);
    let t_large = time_noop_shifts(100_000, ITERS);

    let ratio = t_large.as_nanos() as f64 / t_small.as_nanos().max(1) as f64;
    assert!(
        ratio < 20.0,
        "shift_after appears O(N) on the noop path: a 100× larger \
         index took {:.1}× longer ({:?} for 100_000 entries vs {:?} \
         for 1_000). The fix is a `partition_point`-based binary \
         search — after that, the ratio should approach 1.",
        ratio,
        t_large,
        t_small,
    );
}
