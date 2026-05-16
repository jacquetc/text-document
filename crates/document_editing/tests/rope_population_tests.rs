//! Phase 2 step 5.5e/4: verify document_editing use cases mirror to
//! the global rope under `rope_backend`. Skipped under the default
//! backend.

#![cfg(feature = "rope_backend")]

extern crate text_document_editing as document_editing;

use anyhow::Result;
use common::database::block_offset_index::OffsetMarker;
use document_editing::document_editing_controller;
use document_editing::{InsertFrameDto, InsertTableDto};
use document_io::document_io_controller;
use document_io::ImportPlainTextDto;
use test_harness::setup;

/// Plan §1.6: a new top-level frame (created when insert_frame's
/// position falls outside every existing frame) appends an empty
/// block to the rope with a `\n` boundary. The two top-level frames'
/// byte_ranges must be disjoint and recomputed correctly.
#[test]
fn insert_frame_past_end_appends_top_level_frame_to_rope() -> Result<()> {
    let (db_context, event_hub, mut urm) = setup()?;

    // Use the IO importer so the rope gets populated (the structural
    // `setup_with_text` helper bypasses imports).
    document_io_controller::import_plain_text(
        &db_context,
        &event_hub,
        &ImportPlainTextDto {
            plain_text: "Hello".to_string(),
        },
    )?;

    let store = db_context.get_store();
    assert_eq!(store.rope.read().unwrap().to_string(), "Hello");
    assert_eq!(store.block_offsets.read().unwrap().entries.len(), 1);

    // Position 999 falls outside any existing frame → top-level path.
    document_editing_controller::insert_frame(
        &db_context,
        &event_hub,
        &mut urm,
        None,
        &InsertFrameDto {
            position: 999,
            anchor: 999,
        },
    )?;

    // Rope: "Hello\n" — original 5 bytes + `\n` boundary.
    assert_eq!(store.rope.read().unwrap().to_string(), "Hello\n");

    // Offsets: original block at byte 0, new empty block at byte 6.
    let offsets = store.block_offsets.read().unwrap();
    assert_eq!(offsets.entries.len(), 2);
    assert_eq!(offsets.entries[0].1, 0);
    assert_eq!(offsets.entries[1].1, 6);
    drop(offsets);

    // Two top-level frames; byte_ranges cover their content with the
    // inter-frame `\n` boundary owned by the preceding frame (the
    // convention BlockOffsetIndex.range_of follows for any adjacent
    // entries). In half-open [start, end) terms the ranges [0, 6) and
    // [6, 6) are adjacent, not overlapping.
    let frames = store.frames.read().unwrap();
    let mut top_ranges: Vec<(u32, u32)> = frames
        .values()
        .filter(|f| f.parent_frame.is_none())
        .map(|f| f.byte_range)
        .collect();
    top_ranges.sort();
    assert_eq!(top_ranges, vec![(0, 6), (6, 6)]);

    Ok(())
}

/// Plan §1.6 layout: when a table is inserted into the FIRST of two
/// top-level frames, the cell blocks must land at the end of frame 1's
/// range — BEFORE frame 2's content — not at the rope end. This is the
/// core invariant the `top_level_frame_end_byte`/`rope_insert_block_at`
/// helpers were introduced to maintain.
#[test]
fn insert_table_in_first_top_level_frame_places_cells_before_second_frame() -> Result<()> {
    let (db_context, event_hub, mut urm) = setup()?;

    // Frame 1: "ab" (2 bytes via plain import → 1 top-level frame with
    // one block containing "ab").
    document_io_controller::import_plain_text(
        &db_context,
        &event_hub,
        &ImportPlainTextDto {
            plain_text: "ab".to_string(),
        },
    )?;

    // Frame 2: append a second top-level frame past content.
    document_editing_controller::insert_frame(
        &db_context,
        &event_hub,
        &mut urm,
        None,
        &InsertFrameDto {
            position: 999,
            anchor: 999,
        },
    )?;

    let store = db_context.get_store();
    // Sanity: rope is "ab\n" (3 bytes), 2 entries.
    assert_eq!(store.rope.read().unwrap().to_string(), "ab\n");
    assert_eq!(store.block_offsets.read().unwrap().entries.len(), 2);

    // Insert a 1x1 table inside frame 1 at position 0 (which falls
    // inside frame 1's block "ab"). Position 0 → table goes BEFORE the
    // first block of frame 1.
    document_editing_controller::insert_table(
        &db_context,
        &event_hub,
        &mut urm,
        None,
        &InsertTableDto {
            position: 0,
            anchor: 0,
            rows: 1,
            columns: 1,
        },
    )?;

    let rope_text = store.rope.read().unwrap().to_string();
    let offsets = store.block_offsets.read().unwrap();

    // Expected entry layout (vec order):
    //   [TableAnchor, block_ab, block_cell, block_frame2_empty]
    // Plan §1.6 says cells of a table in frame 1 land in the rope BEFORE
    // any following top-level frame's content. So the cell entry's
    // byte_start MUST be less than frame 2's empty block byte_start.
    let cell_pos = offsets
        .entries
        .iter()
        .filter_map(|(m, bs)| {
            // The cell block is the one whose id is NOT the original
            // "ab" block and NOT the frame-2 empty block — i.e. the one
            // physically between them in the rope.
            if matches!(m, OffsetMarker::Block(_)) {
                Some(*bs)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    // Three block entries; the middle one (sorted by byte) is the cell.
    let mut sorted = cell_pos.clone();
    sorted.sort();
    assert_eq!(
        sorted.len(),
        3,
        "expected 3 block entries (ab, cell, frame2-empty), got {:?} in rope {:?}",
        cell_pos,
        rope_text
    );
    // The cell entry is strictly between frame 1's content and frame 2's empty block.
    let (frame1_byte, cell_byte, frame2_byte) = (sorted[0], sorted[1], sorted[2]);
    assert!(
        frame1_byte < cell_byte && cell_byte < frame2_byte,
        "cell byte position must lie between frame1 and frame2 content; \
         got frame1={frame1_byte} cell={cell_byte} frame2={frame2_byte} in rope {rope_text:?}"
    );

    // The cell content (a `\u{FFFC}` is NOT there — that's the anchor,
    // not the cell). The cell is an empty block surrounded by `\n`
    // boundaries. Verify the second top-level frame's empty block did
    // shift forward (its byte_start moved by at least the cell's bytes).

    Ok(())
}
