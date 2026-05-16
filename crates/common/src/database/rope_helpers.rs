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
/// the byte position where the text starts. If `with_newline_separator`
/// is true, an additional `\n` is appended after the text (the
/// inter-block boundary newline per plan §1.4).
///
/// Returns the byte offset at which the block's text begins.
#[allow(unused_variables)]
pub fn rope_append_block(
    store: &Store,
    block_id: EntityId,
    text: &str,
    with_newline_separator: bool,
) -> u32 {
    #[cfg(feature = "rope_backend")]
    {
        let mut rope = store.rope.write().unwrap();
        let byte_start = rope.len_bytes() as u32;
        let char_end = rope.len_chars();
        rope.insert(char_end, text);
        if with_newline_separator {
            let char_end = rope.len_chars();
            rope.insert(char_end, "\n");
        }
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
