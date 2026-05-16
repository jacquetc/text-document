//! Phase 2 step 5.5e/4: verify document_editing use cases mirror to
//! the global rope under `rope_backend`. Skipped under the default
//! backend.

#![cfg(feature = "rope_backend")]

extern crate text_document_editing as document_editing;

use anyhow::Result;
use document_editing::document_editing_controller;
use document_editing::InsertFrameDto;
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
