//! `BlockOffsetIndex` property tests.
//!
//! Verifies the contract carried by Phase 2 step 3:
//! - `range_of` / `block_at_byte` are inverse-consistent
//! - `shift_after` preserves invariants
//! - The index round-trips through clone (used by snapshots)

use common::database::block_offset_index::BlockOffsetIndex;
use common::types::EntityId;
use proptest::prelude::*;

// ── Unit tests ──────────────────────────────────────────────────────

#[test]
fn empty_index_returns_none_everywhere() {
    let idx = BlockOffsetIndex::new();
    assert_eq!(idx.range_of(1), None);
    assert_eq!(idx.block_at_byte(0), None);
    assert_eq!(idx.block_at_byte(100), None);
    assert_eq!(idx.byte_to_block_byte(0), None);
    assert!(idx.is_empty());
    assert_eq!(idx.len(), 0);
}

#[test]
fn single_block_range_covers_full_rope() {
    let mut idx = BlockOffsetIndex::new();
    idx.set_total_bytes(100);
    idx.push(42, 0);

    assert_eq!(idx.range_of(42), Some((0, 100)));
    assert_eq!(idx.block_at_byte(0), Some(42));
    assert_eq!(idx.block_at_byte(50), Some(42));
    assert_eq!(idx.block_at_byte(100), Some(42)); // at-end belongs to last
    assert_eq!(idx.block_at_byte(101), None);
    assert_eq!(idx.byte_to_block_byte(30), Some((42, 30)));
}

#[test]
fn three_blocks_disjoint_ranges() {
    let mut idx = BlockOffsetIndex::new();
    idx.set_total_bytes(30);
    idx.push(1, 0);
    idx.push(2, 10);
    idx.push(3, 20);

    assert_eq!(idx.range_of(1), Some((0, 10)));
    assert_eq!(idx.range_of(2), Some((10, 20)));
    assert_eq!(idx.range_of(3), Some((20, 30)));
    assert_eq!(idx.range_of(99), None);

    assert_eq!(idx.block_at_byte(0), Some(1));
    assert_eq!(idx.block_at_byte(9), Some(1));
    assert_eq!(idx.block_at_byte(10), Some(2)); // exact start = that block
    assert_eq!(idx.block_at_byte(19), Some(2));
    assert_eq!(idx.block_at_byte(20), Some(3));
    assert_eq!(idx.block_at_byte(30), Some(3)); // at-end belongs to last

    assert_eq!(idx.byte_to_block_byte(15), Some((2, 5)));
}

#[test]
fn shift_after_propagates_to_total_and_entries() {
    let mut idx = BlockOffsetIndex::new();
    idx.set_total_bytes(30);
    idx.push(1, 0);
    idx.push(2, 10);
    idx.push(3, 20);

    // Insert 5 bytes at offset 12 (inside block 2).
    idx.shift_after(12, 5);
    assert_eq!(idx.range_of(1), Some((0, 10)));
    assert_eq!(idx.range_of(2), Some((10, 25))); // grew by 5
    assert_eq!(idx.range_of(3), Some((25, 35))); // start shifted +5
    assert_eq!(idx.total_bytes(), 35);
}

#[test]
fn shift_after_negative_when_threshold_above_first_block() {
    let mut idx = BlockOffsetIndex::new();
    idx.set_total_bytes(30);
    idx.push(1, 0);
    idx.push(2, 10);
    idx.push(3, 20);

    // Delete 5 bytes at offset 12 — only entries with byte_start ≥ 12 shift.
    idx.shift_after(12, -5);
    assert_eq!(idx.range_of(1), Some((0, 10)));
    assert_eq!(idx.range_of(2), Some((10, 15)));
    assert_eq!(idx.range_of(3), Some((15, 25)));
    assert_eq!(idx.total_bytes(), 25);
}

#[test]
fn remove_at_drops_entry_and_subsequent_blocks_extend() {
    let mut idx = BlockOffsetIndex::new();
    idx.set_total_bytes(30);
    idx.push(1, 0);
    idx.push(2, 10);
    idx.push(3, 20);

    // Drop block 2 from the index (caller responsible for fixing the rope).
    idx.remove_at(1);
    assert_eq!(idx.len(), 2);
    assert_eq!(idx.range_of(1), Some((0, 20))); // now extends to block 3's start
    assert_eq!(idx.range_of(2), None);
    assert_eq!(idx.range_of(3), Some((20, 30)));
}

#[test]
fn clone_roundtrips() {
    let mut idx = BlockOffsetIndex::new();
    idx.set_total_bytes(42);
    idx.push(1, 0);
    idx.push(2, 21);

    let cloned = idx.clone();
    assert_eq!(idx, cloned);
}

// ── Property tests ──────────────────────────────────────────────────

/// Generate a non-empty, sorted, gap-free sequence of `(id, byte_start)`
/// entries with a matching `total_bytes`.
fn arb_index() -> impl Strategy<Value = BlockOffsetIndex> {
    proptest::collection::vec(1u32..50u32, 1..10).prop_map(|gaps| {
        let mut idx = BlockOffsetIndex::new();
        let mut cursor = 0u32;
        for (i, gap) in gaps.iter().enumerate() {
            idx.push((i as EntityId) + 1, cursor);
            cursor = cursor.saturating_add(*gap);
        }
        idx.set_total_bytes(cursor);
        idx
    })
}

proptest! {
    /// For any byte offset in `[0, total_bytes]`, `block_at_byte`
    /// returns *some* block id, and that block's range contains the
    /// byte.
    #[test]
    fn prop_block_at_byte_returns_a_containing_block(
        idx in arb_index(),
        byte_frac in 0u32..1000u32,
    ) {
        let total = idx.total_bytes();
        prop_assume!(total > 0);
        let byte = byte_frac % (total + 1);

        let block_id = idx
            .block_at_byte(byte)
            .expect("block_at_byte must succeed for byte ≤ total_bytes");
        let (start, end) = idx.range_of(block_id).expect("block must be indexed");
        prop_assert!(
            start <= byte && byte <= end,
            "byte {} must fall inside [{}, {}] for block {}",
            byte, start, end, block_id
        );
    }

    /// `byte_to_block_byte` must agree with `range_of`.
    #[test]
    fn prop_byte_to_block_byte_consistent_with_range_of(
        idx in arb_index(),
        byte_frac in 0u32..1000u32,
    ) {
        let total = idx.total_bytes();
        prop_assume!(total > 0);
        let byte = byte_frac % (total + 1);

        let (block_id, byte_in) = idx
            .byte_to_block_byte(byte)
            .expect("byte must map to a block");
        let (start, _end) = idx.range_of(block_id).unwrap();
        prop_assert_eq!(byte_in, byte - start);
    }

    /// `shift_after(threshold=0, delta=0)` is a no-op.
    #[test]
    fn prop_shift_zero_is_noop(idx in arb_index()) {
        let mut moved = idx.clone();
        moved.shift_after(0, 0);
        prop_assert_eq!(idx, moved);
    }

    /// A positive `shift_after` followed by the negative inverse at
    /// the same threshold returns the original state.
    #[test]
    fn prop_shift_is_reversible(
        idx in arb_index(),
        threshold_frac in 0u32..1000u32,
        delta in 1i32..50i32,
    ) {
        let threshold = idx
            .total_bytes()
            .checked_add(1)
            .map(|t| threshold_frac % t)
            .unwrap_or(0);

        let mut moved = idx.clone();
        moved.shift_after(threshold, delta);
        moved.shift_after(threshold, -delta);
        prop_assert_eq!(idx, moved);
    }

    /// Entries remain sorted by `byte_start` after any `shift_after`.
    #[test]
    fn prop_shift_preserves_sort(
        idx in arb_index(),
        threshold_frac in 0u32..1000u32,
        delta in 0i32..50i32,
    ) {
        let threshold = idx
            .total_bytes()
            .checked_add(1)
            .map(|t| threshold_frac % t)
            .unwrap_or(0);

        let mut moved = idx;
        moved.shift_after(threshold, delta);

        let starts: Vec<u32> = moved.entries.iter().map(|(_, bs)| *bs).collect();
        for window in starts.windows(2) {
            prop_assert!(
                window[0] <= window[1],
                "entries must stay sorted by byte_start"
            );
        }
    }
}
