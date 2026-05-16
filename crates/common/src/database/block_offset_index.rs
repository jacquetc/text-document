//! Sorted index of block id → rope byte range.
//!
//! Tracks the byte offset at which each block starts in the global
//! rope, plus the rope's total byte length. Each block extends from
//! its `byte_start` to the next block's `byte_start` (or to
//! `total_bytes` for the last block).
//!
//! Invariants:
//! - `entries` is sorted by `byte_start` ascending.
//! - No two entries share the same `byte_start` (blocks are disjoint).
//! - The last entry's `byte_start ≤ total_bytes`.
//! - Empty `entries` ⟺ no blocks in the document.

use crate::types::EntityId;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockOffsetIndex {
    /// `(block_id, byte_start)` pairs sorted by `byte_start` ascending.
    pub entries: Vec<(EntityId, u32)>,

    /// Total byte length of the rope this index describes. The last
    /// block extends from its `byte_start` to this value.
    pub total_bytes: u32,
}

impl BlockOffsetIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn total_bytes(&self) -> u32 {
        self.total_bytes
    }

    pub fn set_total_bytes(&mut self, total: u32) {
        self.total_bytes = total;
    }

    /// Insert a block at a given byte position. The caller is
    /// responsible for keeping the `byte_start` ordered relative to
    /// neighbours — this method does NOT re-sort.
    pub fn insert_at(&mut self, position: usize, block_id: EntityId, byte_start: u32) {
        self.entries.insert(position, (block_id, byte_start));
    }

    /// Append a block at the end (its `byte_start` must be ≥ the last
    /// entry's `byte_start`).
    pub fn push(&mut self, block_id: EntityId, byte_start: u32) {
        debug_assert!(
            self.entries
                .last()
                .map(|(_, bs)| byte_start >= *bs)
                .unwrap_or(true),
            "push must preserve ordering"
        );
        self.entries.push((block_id, byte_start));
    }

    /// Remove the entry at the given position. Panics if out of bounds.
    pub fn remove_at(&mut self, position: usize) -> (EntityId, u32) {
        self.entries.remove(position)
    }

    /// Byte range `(start, end)` of a block. `end` is the next block's
    /// `byte_start` (or `total_bytes` for the last block). Returns
    /// `None` if the block id is not indexed.
    pub fn range_of(&self, block_id: EntityId) -> Option<(u32, u32)> {
        let idx = self.entries.iter().position(|(id, _)| *id == block_id)?;
        let start = self.entries[idx].1;
        let end = self
            .entries
            .get(idx + 1)
            .map(|(_, bs)| *bs)
            .unwrap_or(self.total_bytes);
        Some((start, end))
    }

    /// Block id whose byte range covers `byte`. Returns `None` if the
    /// index is empty or `byte` falls past `total_bytes`.
    ///
    /// `byte == total_bytes` is treated as belonging to the last block
    /// (this is the cursor-at-end-of-document case).
    pub fn block_at_byte(&self, byte: u32) -> Option<EntityId> {
        if self.entries.is_empty() {
            return None;
        }
        if byte > self.total_bytes {
            return None;
        }
        // Binary search for the largest entry whose byte_start ≤ byte.
        let idx = match self.entries.binary_search_by_key(&byte, |(_, bs)| *bs) {
            Ok(i) => i,             // exact match
            Err(0) => return None,  // byte before first block
            Err(i) => i - 1,        // largest byte_start < byte
        };
        Some(self.entries[idx].0)
    }

    /// Convert an absolute rope byte offset into
    /// `(block_id, byte_in_block)`. Returns `None` for offsets past the
    /// end or for an empty index.
    pub fn byte_to_block_byte(&self, byte: u32) -> Option<(EntityId, u32)> {
        let block_id = self.block_at_byte(byte)?;
        let (start, _) = self.range_of(block_id)?;
        Some((block_id, byte - start))
    }

    /// Shift every entry whose `byte_start ≥ threshold` by `delta`
    /// bytes, and adjust `total_bytes` by `delta`. Used after a rope
    /// insert (positive delta) or delete (negative delta) to keep the
    /// index in sync without a full rebuild.
    pub fn shift_after(&mut self, threshold: u32, delta: i32) {
        for (_, bs) in self.entries.iter_mut() {
            if *bs >= threshold {
                *bs = apply_delta(*bs, delta);
            }
        }
        self.total_bytes = apply_delta(self.total_bytes, delta);
    }
}

fn apply_delta(value: u32, delta: i32) -> u32 {
    if delta >= 0 {
        value
            .checked_add(delta as u32)
            .expect("byte offset overflow")
    } else {
        let abs = (-delta) as u32;
        value
            .checked_sub(abs)
            .expect("byte offset would go negative")
    }
}
