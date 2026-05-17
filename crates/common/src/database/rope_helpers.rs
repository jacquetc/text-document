//! Helpers for writing the global character rope from use cases.
//!
//! Each helper mutates `store.rope` and `store.block_offsets`
//! together so callers can stay oblivious to the underlying layout.
//! Read helpers (`block_content_via_store`) prefer rope bytes but
//! fall back to `Block.plain_text` when the rope is stale (e.g.
//! after an unmirrored use case). The fallback goes away in step
//! 7c when `Block.plain_text` is removed.

use crate::database::Store;
use crate::database::block_offset_index::OffsetMarker;
use crate::entities::Block;
use crate::types::EntityId;

/// Read a block's content from the global rope via `block_offsets`,
/// stripping the trailing `\n` boundary that `range_of` includes for
/// non-last entries. Returns an empty string if the block isn't
/// registered in the offset index (e.g. a freshly-created block that
/// hasn't been spliced into the rope yet — `setup_with_text` test
/// docs use this path).
pub fn block_content_via_store(block: &Block, store: &Store) -> String {
    let offsets = store.block_offsets.read().unwrap();
    let marker = OffsetMarker::Block(block.id);
    let Some(idx) = offsets.entries.iter().position(|(m, _)| *m == marker) else {
        return String::new();
    };
    let bs = offsets.entries[idx].1;
    let (be, has_successor) = match offsets.entries.get(idx + 1) {
        Some((_, next_bs)) => (*next_bs, true),
        None => (offsets.total_bytes(), false),
    };
    // Drop the trailing inter-block boundary `\n` ONLY when this block
    // has a successor entry — that one byte is the boundary `\n` between
    // this block and the next. The last entry has no trailing boundary;
    // any final `\n` is real content.
    let content_end = if has_successor && be > bs { be - 1 } else { be };
    let rope = store.rope.read().unwrap();
    rope.byte_slice(bs as usize..content_end as usize).to_string()
}

/// Logical character count of a block — what the old
/// `Block.text_length` field used to cache. Computed by counting the
/// chars in the block's rope content. Image anchors are stored as
/// `\u{FFFC}` (one char, three bytes) inside the rope content, so the
/// char count already covers them. Returns 0 for blocks not registered
/// in the offset index.
pub fn block_char_length(block: &Block, store: &Store) -> i64 {
    block_content_via_store(block, store).chars().count() as i64
}

/// Reset the rope to empty and clear `block_offsets`. Called by
/// importers when they replace the entire document content.
pub fn rope_reset(store: &Store) {
    *store.rope.write().unwrap() = ropey::Rope::new();
    *store.block_offsets.write().unwrap() =
        crate::database::block_offset_index::BlockOffsetIndex::new();
}

/// Append `text` to the end of the rope and register `block_id` at
/// the byte position where the text starts. Returns that byte offset.
///
/// Callers are responsible for inserting an inter-block `\n`
/// (`rope_insert_block_boundary`) before each block AFTER the first
/// in a contiguous frame.
pub fn rope_append_block(store: &Store, block_id: EntityId, text: &str) -> u32 {
    let mut rope = store.rope.write().unwrap();
    let byte_start = rope.len_bytes() as u32;
    let char_end = rope.len_chars();
    rope.insert(char_end, text);
    let new_total = rope.len_bytes() as u32;
    drop(rope);

    let mut offsets = store.block_offsets.write().unwrap();
    offsets.push_block(block_id, byte_start);
    offsets.set_total_bytes(new_total);
    byte_start
}

/// Insert `text` as a new block at `byte_pos` in the rope, prepending
/// a `\n` boundary. Used by `insert_table_uc` to place cell blocks at
/// the end of their containing top-level frame's range (plan §1.6),
/// rather than always at rope end.
///
/// Total bytes inserted: `1 + text.len()`. The block's content
/// occupies `[byte_pos + 1, byte_pos + 1 + text.len())`. The block
/// entry is registered at `byte_pos + 1` in `block_offsets`.
///
/// Existing entries with `byte_start == byte_pos` (e.g. a previous
/// empty block whose end coincides with this insertion point) are
/// kept BEFORE the new entry in the Vec, since the inserted `\n`
/// boundary belongs after them. Entries strictly past `byte_pos`
/// shift forward by `(1 + text.len())` bytes.
///
/// When `byte_pos == total_bytes`, behaves like
/// `rope_insert_block_boundary` followed by `rope_append_block`.
pub fn rope_insert_block_at(store: &Store, byte_pos: u32, block_id: EntityId, text: &str) {
    let delta = (1 + text.len()) as i32;
    // Vec position: insert AFTER any entry at byte_pos itself
    // (those represent earlier empty blocks whose `\n` boundary
    // we are placing now). Only entries strictly past byte_pos
    // come after our new entry in the Vec.
    let new_entry_vec_pos = {
        let offsets = store.block_offsets.read().unwrap();
        offsets
            .entries
            .iter()
            .position(|(_, bs)| *bs > byte_pos)
            .unwrap_or(offsets.entries.len())
    };
    {
        let mut rope = store.rope.write().unwrap();
        let char_idx = rope.byte_to_char(byte_pos as usize);
        let mut combined = String::with_capacity(1 + text.len());
        combined.push('\n');
        combined.push_str(text);
        rope.insert(char_idx, &combined);
    }
    let mut offsets = store.block_offsets.write().unwrap();
    // Shift entries strictly past byte_pos. Entries AT byte_pos
    // (the prior empty block) stay where they are — the new `\n`
    // is conceptually "after" them.
    offsets.shift_after(byte_pos + 1, delta);
    offsets.insert_at(new_entry_vec_pos, OffsetMarker::Block(block_id), byte_pos + 1);
}

/// Walks up `frame.parent_frame` to find the top-level ancestor of
/// the given frame, then returns the end byte of that top-level
/// frame's current rope range — i.e. the byte position where blocks
/// belonging to that frame's subtree (e.g. table cells per plan §1.6)
/// should be inserted so they land BEFORE any following top-level
/// frame's content.
///
/// Reads `block_offsets`/`frames`/`tables`/`table_cells` directly, so
/// the result is fresh even when `Frame.byte_range` has not yet been
/// recomputed at commit time.
pub fn top_level_frame_end_byte(store: &Store, frame_id: EntityId) -> u32 {
    let top_id = {
        let frames = store.frames.read().unwrap();
        let mut current = frame_id;
        loop {
            let Some(f) = frames.get(&current) else {
                return 0;
            };
            match f.parent_frame {
                None => break current,
                Some(p) => current = p,
            }
        }
    };
    let (_min, max) = compute_frame_byte_range_recursive(store, top_id);
    max
}

/// Append a new empty block to the end of the rope, separating it
/// from any prior content with a `\n` boundary (only if the rope is
/// already non-empty). Registers `block_id` at the resulting byte
/// position. Returns that byte position. Used when `insert_frame_uc`
/// creates a new top-level frame with a single empty block.
pub fn rope_append_empty_block(store: &Store, block_id: EntityId) -> u32 {
    let was_empty = store.rope.read().unwrap().len_bytes() == 0;
    if !was_empty {
        rope_insert_block_boundary(store);
    }
    let pos = store.rope.read().unwrap().len_bytes() as u32;
    let mut offsets = store.block_offsets.write().unwrap();
    offsets.push_block(block_id, pos);
    offsets.set_total_bytes(pos);
    pos
}

/// Append a single `\n` inter-block boundary character to the end of
/// the rope. Does NOT register a block — this is the sentinel between
/// two adjacent blocks within the same frame (plan §1.4).
pub fn rope_insert_block_boundary(store: &Store) {
    let mut rope = store.rope.write().unwrap();
    let char_end = rope.len_chars();
    rope.insert(char_end, "\n");
    let new_total = rope.len_bytes() as u32;
    drop(rope);

    store.block_offsets.write().unwrap().set_total_bytes(new_total);
}

/// Insert `text` at `byte_offset_in_block` inside the block identified
/// by `block_id`. Looks up the block's start in the rope via
/// `block_offsets.range_of()`, splices into the rope, and shifts
/// subsequent block offsets by the inserted byte length.
///
/// Silently no-ops if the block is not registered in the offset index
/// (this can happen for blocks whose content lives outside the global
/// rope, e.g. table cells until step 5.5).
pub fn rope_insert_in_block(
    store: &Store,
    block_id: EntityId,
    byte_offset_in_block: u32,
    text: &str,
) {
    let inserted_bytes = text.len() as u32;
    if inserted_bytes == 0 {
        return;
    }
    let block_byte_start = {
        let offsets = store.block_offsets.read().unwrap();
        let Some((start, _end)) = offsets.range_of_block(block_id) else {
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
pub fn rope_split_block(
    store: &Store,
    current_block_id: EntityId,
    byte_offset_in_block: u32,
    new_block_id: EntityId,
) {
    let current_marker = OffsetMarker::Block(current_block_id);
    let (block_start, current_idx) = {
        let offsets = store.block_offsets.read().unwrap();
        let Some((start, _end)) = offsets.range_of(current_marker) else {
            return;
        };
        let idx = offsets
            .entries
            .iter()
            .position(|(m, _)| *m == current_marker)
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
    store.block_offsets.write().unwrap().insert_at(
        current_idx + 1,
        OffsetMarker::Block(new_block_id),
        split_byte + 1,
    );
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
pub fn rope_merge_block_range(
    store: &Store,
    start_block_id: EntityId,
    byte_so_in_start: u32,
    end_block_id: EntityId,
    byte_eo_in_end: u32,
) {
    let start_marker = OffsetMarker::Block(start_block_id);
    let end_marker = OffsetMarker::Block(end_block_id);
    let (start_block_byte, end_block_byte, start_idx, end_idx) = {
        let offsets = store.block_offsets.read().unwrap();
        let Some((sb, _)) = offsets.range_of(start_marker) else {
            return;
        };
        let Some((eb, _)) = offsets.range_of(end_marker) else {
            return;
        };
        let si = offsets
            .entries
            .iter()
            .position(|(m, _)| *m == start_marker)
            .unwrap();
        let ei = offsets
            .entries
            .iter()
            .position(|(m, _)| *m == end_marker)
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

/// Insert a U+FFFC OBJECT REPLACEMENT CHARACTER sentinel in the rope
/// at the table-anchor position, registering a `TableAnchor(table_id)`
/// marker in the offset index (plan §1.6).
///
/// `target_block_id` is the block in the parent frame that the table
/// is adjacent to. `after` controls whether the table goes BEFORE
/// (`after = false`) or AFTER the target block.
///
/// The 3-byte sentinel is paired with an inter-marker `\n`:
/// - `after = false`: inserts `\u{FFFC}\n` at `target.byte_start`
/// - `after = true`, target is NOT the last entry: inserts
///   `\u{FFFC}\n` at `target.byte_end` (between target's trailing
///   `\n` and the next entry)
/// - `after = true`, target IS the last entry: inserts `\n\u{FFFC}`
///   at `target.byte_end` (rope now ends with the sentinel)
///
/// NOTE: cell-internal content is not yet routed through the rope.
/// Cells remain in `Block.plain_text` until a follow-up commit adds
/// the `Frame.byte_range` model from plan §1.6. The rope reflects
/// table *presence* (3-byte sentinel) only.
///
/// No-op if `target_block_id` is not in the index.
pub fn rope_insert_table_anchor(
    store: &Store,
    table_id: EntityId,
    target_block_id: EntityId,
    after: bool,
) {
    const SENTINEL: &str = "\u{FFFC}"; // 3 bytes
    const SENTINEL_BYTES: u32 = 3;

    let (insert_pos, target_idx, target_is_last) = {
        let offsets = store.block_offsets.read().unwrap();
        let target_marker = OffsetMarker::Block(target_block_id);
        let Some((start, end)) = offsets.range_of(target_marker) else {
            return;
        };
        let idx = offsets
            .entries
            .iter()
            .position(|(m, _)| *m == target_marker)
            .unwrap();
        let is_last = idx + 1 == offsets.entries.len();
        let pos = if after { end } else { start };
        (pos, idx, is_last)
    };

    // Insertion strategy:
    let (rope_inserted, marker_byte_start, new_entry_pos, shift_threshold, shift_delta) =
        if !after {
            // Before target: "\u{FFFC}\n" at target.byte_start
            ("\u{FFFC}\n", insert_pos, target_idx, insert_pos, 4i32)
        } else if !target_is_last {
            // After target, with following entries: "\u{FFFC}\n"
            // at target.byte_end. The TableAnchor sits where the
            // next block USED to start; that following entry
            // shifts by 4.
            (
                "\u{FFFC}\n",
                insert_pos,
                target_idx + 1,
                insert_pos,
                4i32,
            )
        } else {
            // After target which is last: "\n\u{FFFC}" appended.
            // TableAnchor's byte_start sits 1 past the original
            // total (after the new `\n`).
            ("\n\u{FFFC}", insert_pos + 1, target_idx + 1, insert_pos, 4i32)
        };

    // 1. Splice the literal bytes into the rope.
    {
        let mut rope = store.rope.write().unwrap();
        let char_idx = rope.byte_to_char(insert_pos as usize);
        rope.insert(char_idx, rope_inserted);
    }

    // 2. Shift entries past the insertion point. Use shift_after
    //    BEFORE inserting our new entry so we don't double-shift.
    store
        .block_offsets
        .write()
        .unwrap()
        .shift_after(shift_threshold, shift_delta);

    // 3. Register the TableAnchor at the resolved byte position
    //    and the resolved Vec position.
    store.block_offsets.write().unwrap().insert_at(
        new_entry_pos,
        OffsetMarker::TableAnchor(table_id),
        marker_byte_start,
    );

    // Note: SENTINEL_BYTES is part of `shift_delta` (3 for the
    // sentinel + 1 for the `\n`).
    let _ = SENTINEL;
    let _ = SENTINEL_BYTES;
}

/// Append a U+FFFC table-anchor sentinel at the end of the rope and
/// register a `TableAnchor(table_id)` marker. If the rope is already
/// non-empty, prepends a `\n` boundary so the sentinel doesn't run
/// into the previous entry's content.
///
/// Used by import paths (`import_html_uc`, `import_markdown_uc`)
/// that process the document linearly and append entities as they
/// encounter them, rather than inserting relative to an existing
/// target block.
pub fn rope_append_table_anchor(store: &Store, table_id: EntityId) {
    let (anchor_byte_start, new_total) = {
        let mut rope = store.rope.write().unwrap();
        let was_empty = rope.len_bytes() == 0;
        let char_end = rope.len_chars();
        let to_insert = if was_empty { "\u{FFFC}" } else { "\n\u{FFFC}" };
        rope.insert(char_end, to_insert);
        let new_total = rope.len_bytes() as u32;
        // Sentinel is 3 bytes; if a `\n` was prepended that's 1 byte
        // before the sentinel.
        let anchor_byte_start = new_total - 3;
        (anchor_byte_start, new_total)
    };

    let mut offsets = store.block_offsets.write().unwrap();
    offsets.entries.push((OffsetMarker::TableAnchor(table_id), anchor_byte_start));
    offsets.set_total_bytes(new_total);
}

/// Remove a TableAnchor sentinel from the rope, undoing the effect
/// of `rope_insert_table_anchor`. Looks up the anchor's byte range
/// (always 3 bytes for the U+FFFC plus 1 byte of inter-marker `\n`
/// either before or after, depending on what's adjacent), removes
/// those 4 bytes from the rope, drops the entry, shifts trailing
/// entries by -4.
///
/// No-op if no TableAnchor for `table_id` exists.
pub fn rope_remove_table_anchor(store: &Store, table_id: EntityId) {
    let anchor_marker = OffsetMarker::TableAnchor(table_id);
    let (anchor_byte_start, anchor_idx, anchor_is_last, has_predecessor) = {
        let offsets = store.block_offsets.read().unwrap();
        let Some((start, _end)) = offsets.range_of(anchor_marker) else {
            return;
        };
        let idx = offsets
            .entries
            .iter()
            .position(|(m, _)| *m == anchor_marker)
            .unwrap();
        let is_last = idx + 1 == offsets.entries.len();
        let has_pred = idx > 0;
        (start, idx, is_last, has_pred)
    };

    // Symmetric to insert_table_anchor. The 4 bytes to remove are:
    // - if anchor is last: [byte_start - 1 .. byte_start + 3) — the
    //   preceding `\n` + the 3-byte sentinel
    // - otherwise: [byte_start .. byte_start + 4) — the sentinel
    //   + the following `\n`
    let (remove_start, remove_end) = if anchor_is_last && has_predecessor {
        (anchor_byte_start - 1, anchor_byte_start + 3)
    } else {
        (anchor_byte_start, anchor_byte_start + 4)
    };

    {
        let mut rope = store.rope.write().unwrap();
        let char_start = rope.byte_to_char(remove_start as usize);
        let char_end = rope.byte_to_char(remove_end as usize);
        rope.remove(char_start..char_end);
    }
    {
        let mut offsets = store.block_offsets.write().unwrap();
        offsets.entries.remove(anchor_idx);
    }
    store
        .block_offsets
        .write()
        .unwrap()
        .shift_after(remove_start, -4);
}

/// Remove a registered block from the rope: drops its content bytes
/// plus one boundary `\n` (the one after, if the block has a
/// successor; the one before, if it's the last entry), removes the
/// entry from the index, and shifts trailing entries by the negative
/// byte delta.
///
/// No-op if `block_id` is not in the index. No-op for the special
/// case of a single-block document being asked to remove its sole
/// block (we'd produce an empty rope but the block itself is being
/// cascaded by the caller).
pub fn rope_remove_block(store: &Store, block_id: EntityId) {
    let block_marker = OffsetMarker::Block(block_id);
    let (block_start, block_end, idx, is_last, has_pred) = {
        let offsets = store.block_offsets.read().unwrap();
        let Some((start, end)) = offsets.range_of(block_marker) else {
            return;
        };
        let idx = offsets
            .entries
            .iter()
            .position(|(m, _)| *m == block_marker)
            .unwrap();
        let is_last = idx + 1 == offsets.entries.len();
        let has_pred = idx > 0;
        (start, end, idx, is_last, has_pred)
    };

    // Determine the byte range to delete:
    // - if there's a successor: [block_start..block_end) — the
    //   block's content INCLUDING its trailing boundary `\n`
    //   (which is the byte at block_end - 1)
    // - if last and has predecessor: [block_start - 1..block_end)
    //   — also delete the LEADING boundary `\n` that the previous
    //   entry placed before us
    // - if last and no predecessor (sole entry): just delete
    //   [block_start..block_end) (no boundary `\n` exists)
    let (remove_start, remove_end) = if is_last && has_pred {
        (block_start.saturating_sub(1), block_end)
    } else {
        (block_start, block_end)
    };
    if remove_end <= remove_start {
        // Drop the entry only; nothing to remove from the rope.
        store.block_offsets.write().unwrap().entries.remove(idx);
        return;
    }
    let deleted_bytes = remove_end - remove_start;

    {
        let mut rope = store.rope.write().unwrap();
        let char_start = rope.byte_to_char(remove_start as usize);
        let char_end = rope.byte_to_char(remove_end as usize);
        rope.remove(char_start..char_end);
    }
    store.block_offsets.write().unwrap().entries.remove(idx);
    // Shift entries STRICTLY PAST the removed range. Using `remove_end` as
    // the threshold (rather than `remove_start`) keeps an empty predecessor
    // whose byte_start equals `remove_start` (the leading boundary `\n` we
    // just deleted) in place — its content position is unchanged, only its
    // trailing boundary is gone. `total_bytes` decreases by `deleted_bytes`
    // regardless of threshold.
    store
        .block_offsets
        .write()
        .unwrap()
        .shift_after(remove_end, -(deleted_bytes as i32));
}

/// Replace the entire content of a registered block in the rope with
/// `new_text`. Preserves the block's `byte_start` and its trailing
/// boundary `\n` (if any); subsequent entries shift by the net
/// length delta.
///
/// Used by use cases that compute a block's final content as a string
/// and want to push that content to the rope in one shot — e.g. the
/// block-splitting branches of `insert_html_at_position_uc` and
/// `insert_markdown_at_position_uc`, where each affected block
/// (head, tail, mid-replacement) gets a single new value.
///
/// No-op if `block_id` is not in the index.
pub fn rope_replace_block_content(store: &Store, block_id: EntityId, new_text: &str) {
    let (block_byte_start, content_bytes) = {
        let offsets = store.block_offsets.read().unwrap();
        let Some((start, end)) = offsets.range_of_block(block_id) else {
            return;
        };
        let total = offsets.total_bytes();
        // `range_of` extends to the next entry's `byte_start` (or to
        // `total_bytes`). If there's a following entry, the byte at
        // `end - 1` is the inter-block boundary `\n` that belongs to
        // the boundary between this block and the next, not to this
        // block's content.
        let has_trailing_boundary = end < total;
        let content_bytes = if has_trailing_boundary {
            end - start - 1
        } else {
            end - start
        };
        (start, content_bytes)
    };

    let new_bytes = new_text.len() as u32;
    if content_bytes == 0 && new_bytes == 0 {
        return;
    }
    let delta = new_bytes as i32 - content_bytes as i32;

    // Splice [block_byte_start..block_byte_start + content_bytes)
    // with `new_text`.
    {
        let mut rope = store.rope.write().unwrap();
        let char_start = rope.byte_to_char(block_byte_start as usize);
        if content_bytes > 0 {
            let char_end = rope.byte_to_char((block_byte_start + content_bytes) as usize);
            rope.remove(char_start..char_end);
        }
        if new_bytes > 0 {
            rope.insert(char_start, new_text);
        }
    }

    if delta != 0 {
        // Shift entries that sit strictly past this block's start
        // (i.e. the trailing boundary and everything after).
        store
            .block_offsets
            .write()
            .unwrap()
            .shift_after(block_byte_start + 1, delta);
    }
}

/// Delete bytes `[byte_start_in_block..byte_end_in_block)` from inside
/// the block identified by `block_id`. Shifts subsequent block offsets
/// by the deleted byte length. No-op for blocks not in the index.
pub fn rope_delete_in_block(
    store: &Store,
    block_id: EntityId,
    byte_start_in_block: u32,
    byte_end_in_block: u32,
) {
    if byte_end_in_block <= byte_start_in_block {
        return;
    }
    let deleted_bytes = byte_end_in_block - byte_start_in_block;
    let block_byte_start = {
        let offsets = store.block_offsets.read().unwrap();
        let Some((start, _end)) = offsets.range_of_block(block_id) else {
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

/// Recompute `Frame.byte_range` for every frame in `store.frames`
/// based on current `block_offsets` and the frame tree structure.
/// Plan §1.6 invariant: each frame's byte_range is the (min_start,
/// max_end) over all its descendant blocks, sub-frames, and table
/// anchors+cells.
///
/// Call this after any mutation that affects rope byte positions.
/// O(F + B) where F = frames, B = blocks in the document.
pub fn recompute_all_frame_byte_ranges(store: &Store) {
    let frame_ids: Vec<EntityId> = {
        let frames = store.frames.read().unwrap();
        frames.keys().copied().collect()
    };
    for fid in frame_ids {
        let new_range = compute_frame_byte_range_recursive(store, fid);
        let mut frames = store.frames.write().unwrap();
        if let Some(f) = frames.get(&fid).cloned() {
            if f.byte_range != new_range {
                let mut updated = f;
                updated.byte_range = new_range;
                frames.insert(fid, updated);
            }
        }
    }
}

fn compute_frame_byte_range_recursive(store: &Store, frame_id: EntityId) -> (u32, u32) {
    let mut bounds: Option<(u32, u32)> = None;
    walk_frame_bounds(store, frame_id, &mut bounds);
    bounds.unwrap_or((0, 0))
}

fn walk_frame_bounds(store: &Store, frame_id: EntityId, bounds: &mut Option<(u32, u32)>) {
    fn merge(bounds: &mut Option<(u32, u32)>, s: u32, e: u32) {
        *bounds = Some(match *bounds {
            None => (s, e),
            Some((min, max)) => (min.min(s), max.max(e)),
        });
    }

    let (blocks, child_order, table_id) = {
        let frames = store.frames.read().unwrap();
        let Some(f) = frames.get(&frame_id) else {
            return;
        };
        (f.blocks.clone(), f.child_order.clone(), f.table)
    };

    {
        let offsets = store.block_offsets.read().unwrap();
        for bid in &blocks {
            if let Some((s, e)) = offsets.range_of_block(*bid) {
                merge(bounds, s, e);
            }
        }
        if let Some(tid) = table_id {
            if let Some((s, e)) = offsets.range_of(OffsetMarker::TableAnchor(tid)) {
                merge(bounds, s, e);
            }
        }
    }

    for entry in &child_order {
        if *entry < 0 {
            walk_frame_bounds(store, (-*entry) as EntityId, bounds);
        }
    }

    if let Some(tid) = table_id {
        let cell_ids: Vec<EntityId> = {
            let tables = store.tables.read().unwrap();
            tables.get(&tid).map(|t| t.cells.clone()).unwrap_or_default()
        };
        for cell_id in &cell_ids {
            let cell_frame_id = {
                let cells = store.table_cells.read().unwrap();
                cells.get(cell_id).and_then(|c| c.cell_frame)
            };
            if let Some(cfid) = cell_frame_id {
                walk_frame_bounds(store, cfid, bounds);
            }
        }
    }
}
