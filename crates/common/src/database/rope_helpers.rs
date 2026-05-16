//! Helpers for writing the global character rope from use cases.
//!
//! Phase 2 step 5 (use-case migration). Each helper has two bodies:
//! - Under `rope_backend`: real ropey/BlockOffsetIndex updates
//! - Under default: no-op
//!
//! Callers invoke them unconditionally. The `Store` type alias
//! resolves to the right backend at compile time; the cfg-gated
//! function body only touches `store.rope` / `store.block_offsets`
//! when those fields exist.
//!
//! When step 7 drops the `rope_backend` feature gate, the no-op
//! branches go away and the helper bodies become unconditional.

use crate::database::Store;
use crate::types::EntityId;

/// Reset the rope to empty and clear `block_offsets`. Called by
/// importers when they replace the entire document content.
#[allow(unused_variables)]
pub fn rope_reset(store: &Store) {
    #[cfg(feature = "rope_backend")]
    {
        *store.rope.write().unwrap() = ropey::Rope::new();
        *store.block_offsets.write().unwrap() = crate::database::block_offset_index::BlockOffsetIndex::new();
    }
}

/// Append `text` to the end of the rope and register `block_id` at
/// the byte position where the text starts. Returns that byte offset.
///
/// Callers are responsible for inserting an inter-block `\n`
/// (`rope_insert_block_boundary`) before each block AFTER the first
/// in a contiguous frame.
#[allow(unused_variables)]
pub fn rope_append_block(store: &Store, block_id: EntityId, text: &str) -> u32 {
    #[cfg(feature = "rope_backend")]
    {
        let mut rope = store.rope.write().unwrap();
        let byte_start = rope.len_bytes() as u32;
        let char_end = rope.len_chars();
        rope.insert(char_end, text);
        let new_total = rope.len_bytes() as u32;
        drop(rope);

        let mut offsets = store.block_offsets.write().unwrap();
        offsets.push(block_id, byte_start);
        offsets.set_total_bytes(new_total);
        return byte_start;
    }
    #[cfg(not(feature = "rope_backend"))]
    {
        0
    }
}

/// Append a single `\n` inter-block boundary character to the end of
/// the rope. Does NOT register a block — this is the sentinel between
/// two adjacent blocks within the same frame (plan §1.4).
#[allow(unused_variables)]
pub fn rope_insert_block_boundary(store: &Store) {
    #[cfg(feature = "rope_backend")]
    {
        let mut rope = store.rope.write().unwrap();
        let char_end = rope.len_chars();
        rope.insert(char_end, "\n");
        let new_total = rope.len_bytes() as u32;
        drop(rope);

        store.block_offsets.write().unwrap().set_total_bytes(new_total);
    }
}

/// Insert `text` at `byte_offset_in_block` inside the block identified
/// by `block_id`. Looks up the block's start in the rope via
/// `block_offsets.range_of()`, splices into the rope, and shifts
/// subsequent block offsets by the inserted byte length.
///
/// Silently no-ops if the block is not registered in the offset index
/// (this can happen for blocks whose content lives outside the global
/// rope, e.g. table cells until step 5.5).
#[allow(unused_variables)]
pub fn rope_insert_in_block(
    store: &Store,
    block_id: EntityId,
    byte_offset_in_block: u32,
    text: &str,
) {
    #[cfg(feature = "rope_backend")]
    {
        let inserted_bytes = text.len() as u32;
        if inserted_bytes == 0 {
            return;
        }
        let block_byte_start = {
            let offsets = store.block_offsets.read().unwrap();
            let Some((start, _end)) = offsets.range_of(block_id) else {
                return;
            };
            start
        };
        let rope_byte = block_byte_start + byte_offset_in_block;
        {
            let mut rope = store.rope.write().unwrap();
            let char_idx = rope.byte_to_char(rope_byte as usize);
            rope.insert(char_idx, text);
        }
        // Shift entries past this block by inserted_bytes. Threshold
        // is one byte past block_byte_start so the current block's own
        // entry isn't moved.
        store
            .block_offsets
            .write()
            .unwrap()
            .shift_after(block_byte_start + 1, inserted_bytes as i32);
    }
}

/// Delete bytes `[byte_start_in_block..byte_end_in_block)` from inside
/// the block identified by `block_id`. Shifts subsequent block offsets
/// by the deleted byte length. No-op for blocks not in the index.
#[allow(unused_variables)]
pub fn rope_delete_in_block(
    store: &Store,
    block_id: EntityId,
    byte_start_in_block: u32,
    byte_end_in_block: u32,
) {
    #[cfg(feature = "rope_backend")]
    {
        if byte_end_in_block <= byte_start_in_block {
            return;
        }
        let deleted_bytes = byte_end_in_block - byte_start_in_block;
        let block_byte_start = {
            let offsets = store.block_offsets.read().unwrap();
            let Some((start, _end)) = offsets.range_of(block_id) else {
                return;
            };
            start
        };
        let rope_byte_start = block_byte_start + byte_start_in_block;
        let rope_byte_end = block_byte_start + byte_end_in_block;
        {
            let mut rope = store.rope.write().unwrap();
            let char_start = rope.byte_to_char(rope_byte_start as usize);
            let char_end = rope.byte_to_char(rope_byte_end as usize);
            rope.remove(char_start..char_end);
        }
        store
            .block_offsets
            .write()
            .unwrap()
            .shift_after(block_byte_start + 1, -(deleted_bytes as i32));
    }
}
