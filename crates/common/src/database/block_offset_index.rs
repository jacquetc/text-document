//! Sorted index of block id → rope byte range.
//!
//! Skeleton for Phase 2 step 3. Method bodies are stubbed with
//! `unimplemented!()`; the real implementation lands alongside
//! `RopeStore` use-case migration.

#![allow(dead_code)]

use crate::types::EntityId;
use im::HashMap;

/// Sorted, gap-free index of `(block_id, byte_start)` pairs.
///
/// Each block extends from its `byte_start` to the next entry's
/// `byte_start` (or the rope's end for the last block).
#[derive(Debug, Clone, Default)]
pub struct BlockOffsetIndex {
    pub(crate) entries: Vec<(EntityId, u32)>,
}

impl BlockOffsetIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Byte range `(start, end)` of a block, or `None` if not indexed.
    pub fn range_of(&self, _block_id: EntityId) -> Option<(u32, u32)> {
        unimplemented!("BlockOffsetIndex::range_of — Phase 2 step 3")
    }

    /// Block id covering a given byte offset (binary search).
    pub fn block_at_byte(&self, _byte: u32) -> Option<EntityId> {
        unimplemented!("BlockOffsetIndex::block_at_byte — Phase 2 step 3")
    }

    /// Translate an absolute rope byte offset to `(block_id, byte_in_block)`.
    pub fn byte_to_char_in_block(
        &self,
        _rope: &ropey::Rope,
        _byte: u32,
    ) -> Option<(EntityId, u32)> {
        unimplemented!("BlockOffsetIndex::byte_to_char_in_block — Phase 2 step 3")
    }

    /// Shift every entry's `byte_start` ≥ `threshold` by `delta` bytes.
    /// Used after a rope insert/delete to keep the index in sync.
    pub fn shift_after(&mut self, _threshold: u32, _delta: i32) {
        unimplemented!("BlockOffsetIndex::shift_after — Phase 2 step 3")
    }

    /// Rebuild from the blocks table + each frame's child order. Called
    /// after structural edits that can't be expressed as a `shift_after`.
    pub fn rebuild(
        &mut self,
        _blocks: &HashMap<EntityId, crate::entities::Block>,
        _frame_order: &[EntityId],
    ) {
        unimplemented!("BlockOffsetIndex::rebuild — Phase 2 step 3")
    }
}
