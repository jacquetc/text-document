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

/// Split an existing block in the rope at `byte_offset_in_block`:
/// - inserts a `\n` inter-block boundary at the absolute byte position
///   `block_start + byte_offset_in_block` in the rope
/// - shifts entries past that position by +1 byte
/// - inserts a new entry for `new_block_id` at
///   `block_start + byte_offset_in_block + 1` (right after the newline),
///   placed immediately after the original block in the entries Vec
///
/// `byte_offset_in_block` may be 0 (split before first char of block,
/// i.e. insert empty block before this one) or equal to the block's
/// byte length (split after last char, i.e. insert empty block after).
#[allow(unused_variables)]
pub fn rope_split_block(
    store: &Store,
    current_block_id: EntityId,
    byte_offset_in_block: u32,
    new_block_id: EntityId,
) {
    #[cfg(feature = "rope_backend")]
    {
        let (block_start, current_idx) = {
            let offsets = store.block_offsets.read().unwrap();
            let Some((start, _end)) = offsets.range_of(current_block_id) else {
                return;
            };
            let idx = offsets
                .entries
                .iter()
                .position(|(id, _)| *id == current_block_id)
                .unwrap();
            (start, idx)
        };
        let split_byte = block_start + byte_offset_in_block;

        // 1. Insert the `\n` boundary at the split point.
        {
            let mut rope = store.rope.write().unwrap();
            let char_idx = rope.byte_to_char(split_byte as usize);
            rope.insert(char_idx, "\n");
        }

        // 2. Shift entries past the split (and total_bytes) by +1.
        //    Threshold > split_byte so the new entry we insert next
        //    isn't double-shifted.
        store
            .block_offsets
            .write()
            .unwrap()
            .shift_after(split_byte + 1, 1);

        // 3. Register the new block at `split_byte + 1`, immediately
        //    after the original in the entries Vec.
        store
            .block_offsets
            .write()
            .unwrap()
            .insert_at(current_idx + 1, new_block_id, split_byte + 1);
    }
}

/// Merge `start_block` and `end_block` by deleting the rope range
/// `[start_block.start + byte_so .. end_block.start + byte_eo)` — i.e.
/// the suffix of `start_block`, every block between (and their
/// boundary newlines), and the prefix of `end_block`. Removes the
/// index entries for every block strictly between `start_block` and
/// `end_block` (inclusive of `end_block` itself); the surviving
/// content lives in `start_block`. Shifts any blocks past `end_block`
/// by the negative delta.
///
/// No-op if `start_block` is not in the index. Skipped for any
/// intermediate block id whose range is missing from the index (e.g.
/// table cells until step 5.5e).
#[allow(unused_variables)]
pub fn rope_merge_block_range(
    store: &Store,
    start_block_id: EntityId,
    byte_so_in_start: u32,
    end_block_id: EntityId,
    byte_eo_in_end: u32,
) {
    #[cfg(feature = "rope_backend")]
    {
        let (start_block_byte, end_block_byte, start_idx, end_idx) = {
            let offsets = store.block_offsets.read().unwrap();
            let Some((sb, _)) = offsets.range_of(start_block_id) else {
                return;
            };
            let Some((eb, _)) = offsets.range_of(end_block_id) else {
                return;
            };
            let si = offsets
                .entries
                .iter()
                .position(|(id, _)| *id == start_block_id)
                .unwrap();
            let ei = offsets
                .entries
                .iter()
                .position(|(id, _)| *id == end_block_id)
                .unwrap();
            (sb, eb, si, ei)
        };
        if end_idx <= start_idx {
            return;
        }

        let delete_start = start_block_byte + byte_so_in_start;
        let delete_end = end_block_byte + byte_eo_in_end;
        if delete_end <= delete_start {
            return;
        }
        let deleted_bytes = delete_end - delete_start;

        // 1. Remove the rope range.
        {
            let mut rope = store.rope.write().unwrap();
            let char_start = rope.byte_to_char(delete_start as usize);
            let char_end = rope.byte_to_char(delete_end as usize);
            rope.remove(char_start..char_end);
        }

        // 2. Remove block_offsets entries for [start_idx+1 ..= end_idx].
        //    Vec::drain handles this in one go.
        {
            let mut offsets = store.block_offsets.write().unwrap();
            offsets.entries.drain((start_idx + 1)..=end_idx);
        }

        // 3. Shift any remaining entries past the deletion by -deleted_bytes.
        //    Threshold > delete_start because start_block's own entry
        //    sits at delete_start - byte_so_in_start (≤ delete_start)
        //    and must not move.
        store
            .block_offsets
            .write()
            .unwrap()
            .shift_after(delete_start + 1, -(deleted_bytes as i32));
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
