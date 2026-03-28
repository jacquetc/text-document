//! Tests for editing inside table cells.
//!
//! These tests validate that the snapshot position system and the sequential
//! position computation (`find_block_at_position_sequential`) stay in sync
//! when text is inserted, deleted, or replaced inside table cells.

use text_document::{FlowElementSnapshot, TextDocument};

/// Create a document: "Before" + 2x2 table + "After"
/// Create a document: "Before" + 2x2 empty table + "After" using insert_table
fn doc_with_empty_table() -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text("Before").unwrap();
    let cursor = doc.cursor_at(6);
    cursor.insert_table(2, 2).unwrap();
    let end = doc.character_count();
    let cursor2 = doc.cursor_at(end);
    cursor2.insert_block().unwrap();
    cursor2.insert_text("After").unwrap();
    doc
}

/// Create a document: "Before" + 2x2 table + "After"
fn doc_with_table_and_text() -> TextDocument {
    let doc = TextDocument::new();
    doc.set_markdown("Before\n\n| A | B |\n|---|---|\n| c | d |\n\nAfter")
        .unwrap()
        .wait()
        .unwrap();
    doc
}

/// Collect all block (position, length, text) tuples from a snapshot, in order.
fn all_block_positions(doc: &TextDocument) -> Vec<(usize, usize, String)> {
    let snap = doc.snapshot_flow();
    let mut out = Vec::new();
    collect_from_elements(&snap.elements, &mut out);
    out
}

fn collect_from_elements(elements: &[FlowElementSnapshot], out: &mut Vec<(usize, usize, String)>) {
    for el in elements {
        match el {
            FlowElementSnapshot::Block(bs) => {
                out.push((bs.position, bs.length, bs.text.clone()));
            }
            FlowElementSnapshot::Table(ts) => {
                for cell in &ts.cells {
                    for block in &cell.blocks {
                        out.push((block.position, block.length, block.text.clone()));
                    }
                }
            }
            FlowElementSnapshot::Frame(fs) => {
                collect_from_elements(&fs.elements, out);
            }
        }
    }
}

/// Assert that no two blocks overlap and positions are monotonically increasing.
fn assert_no_overlaps(positions: &[(usize, usize, String)]) {
    let mut sorted = positions.to_vec();
    sorted.sort_by_key(|(pos, _, _)| *pos);
    for i in 1..sorted.len() {
        let (prev_pos, prev_len, ref prev_text) = sorted[i - 1];
        let (cur_pos, _, ref cur_text) = sorted[i];
        let prev_end = prev_pos + prev_len + 1;
        assert!(
            cur_pos >= prev_end,
            "Block {:?} at pos {} (end {}) overlaps with block {:?} at pos {}",
            prev_text,
            prev_pos,
            prev_end,
            cur_text,
            cur_pos
        );
    }
}

/// Find the snapshot position of the first cell (0,0) block.
fn cell_block_position(doc: &TextDocument, row: usize, col: usize) -> Option<(usize, usize)> {
    let snap = doc.snapshot_flow();
    for el in &snap.elements {
        if let FlowElementSnapshot::Table(ts) = el {
            for cell in &ts.cells {
                if cell.row == row
                    && cell.column == col
                    && let Some(b) = cell.blocks.first()
                {
                    return Some((b.position, b.length));
                }
            }
        }
    }
    None
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Position consistency
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn positions_no_overlap_fresh_table() {
    let doc = doc_with_table_and_text();
    let positions = all_block_positions(&doc);
    assert_no_overlaps(&positions);
}

#[test]
fn positions_no_overlap_after_insert_in_first_cell() {
    let doc = doc_with_table_and_text();
    let (pos, len) = cell_block_position(&doc, 0, 0).expect("cell (0,0)");
    let cursor = doc.cursor_at(pos + len);
    cursor.insert_text("X").unwrap();
    let positions = all_block_positions(&doc);
    assert_no_overlaps(&positions);
}

#[test]
fn positions_no_overlap_after_insert_in_last_cell() {
    let doc = doc_with_table_and_text();
    let (pos, len) = cell_block_position(&doc, 1, 1).expect("cell (1,1)");
    let cursor = doc.cursor_at(pos + len);
    cursor.insert_text("Z").unwrap();
    let positions = all_block_positions(&doc);
    assert_no_overlaps(&positions);
}

#[test]
fn positions_no_overlap_after_multiple_inserts() {
    let doc = doc_with_table_and_text();

    // Type in cell (0,0)
    let (pos, len) = cell_block_position(&doc, 0, 0).expect("cell (0,0)");
    let cursor = doc.cursor_at(pos + len);
    cursor.insert_text("Hello").unwrap();

    // Type in cell (1,1)
    let (pos2, len2) = cell_block_position(&doc, 1, 1).expect("cell (1,1)");
    let cursor2 = doc.cursor_at(pos2 + len2);
    cursor2.insert_text("World").unwrap();

    let positions = all_block_positions(&doc);
    assert_no_overlaps(&positions);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Text appears in the correct cell
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn insert_text_appears_in_cell() {
    let doc = doc_with_table_and_text();
    let (pos, len) = cell_block_position(&doc, 0, 0).expect("cell (0,0)");
    let cursor = doc.cursor_at(pos + len);
    cursor.insert_text("X").unwrap();

    let snap = doc.snapshot_flow();
    let cell_text: Option<&str> = snap.elements.iter().find_map(|el| {
        if let FlowElementSnapshot::Table(ts) = el {
            ts.cells
                .iter()
                .find(|c| c.row == 0 && c.column == 0)
                .and_then(|c| c.blocks.first())
                .map(|b| b.text.as_str())
        } else {
            None
        }
    });

    let text = cell_text.expect("cell (0,0) should have a block");
    assert!(
        text.contains('X'),
        "cell (0,0) text should contain 'X', got {:?}",
        text
    );
}

#[test]
fn after_block_position_shifts_when_cell_grows() {
    let doc = doc_with_table_and_text();
    let positions_before = all_block_positions(&doc);
    let after_pos_before = positions_before
        .iter()
        .find(|(_, _, t)| t == "After")
        .map(|(p, _, _)| *p)
        .expect("should find 'After'");

    let (pos, len) = cell_block_position(&doc, 0, 0).expect("cell (0,0)");
    let cursor = doc.cursor_at(pos + len);
    cursor.insert_text("XYZ").unwrap();

    let positions_after = all_block_positions(&doc);
    let after_pos_after = positions_after
        .iter()
        .find(|(_, _, t)| t == "After")
        .map(|(p, _, _)| *p)
        .expect("should find 'After'");

    assert_eq!(
        after_pos_after,
        after_pos_before + 3,
        "'After' position should shift by 3 chars"
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Cursor positioning
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn cursor_stays_in_cell_after_insert() {
    let doc = doc_with_table_and_text();
    let (pos, len) = cell_block_position(&doc, 0, 0).expect("cell (0,0)");
    let cursor = doc.cursor_at(pos + len);
    cursor.insert_text("X").unwrap();

    let cursor_pos = cursor.position();
    let (cell_pos, cell_len) = cell_block_position(&doc, 0, 0).expect("cell (0,0) after edit");

    assert!(
        cursor_pos >= cell_pos && cursor_pos <= cell_pos + cell_len,
        "cursor at {} should be within cell (0,0) range [{}, {}]",
        cursor_pos,
        cell_pos,
        cell_pos + cell_len
    );
}

#[test]
fn consecutive_inserts_in_same_cell() {
    let doc = doc_with_table_and_text();
    let (pos, len) = cell_block_position(&doc, 0, 0).expect("cell (0,0)");
    let cursor = doc.cursor_at(pos + len);

    // Type three characters
    cursor.insert_text("a").unwrap();
    cursor.insert_text("b").unwrap();
    cursor.insert_text("c").unwrap();

    // All text should be in cell (0,0)
    let snap = doc.snapshot_flow();
    let cell_text: Option<&str> = snap.elements.iter().find_map(|el| {
        if let FlowElementSnapshot::Table(ts) = el {
            ts.cells
                .iter()
                .find(|c| c.row == 0 && c.column == 0)
                .and_then(|c| c.blocks.first())
                .map(|b| b.text.as_str())
        } else {
            None
        }
    });

    let text = cell_text.expect("cell (0,0) should have a block");
    assert!(
        text.contains("abc"),
        "cell (0,0) should contain 'abc', got {:?}",
        text
    );

    // Positions should still be valid
    assert_no_overlaps(&all_block_positions(&doc));
}

#[test]
fn delete_in_cell_keeps_positions_valid() {
    let doc = doc_with_table_and_text();
    let (pos, len) = cell_block_position(&doc, 0, 0).expect("cell (0,0)");

    // Only delete if cell has content
    if len > 0 {
        let cursor = doc.cursor_at(pos + len);
        cursor.delete_previous_char().unwrap();
        assert_no_overlaps(&all_block_positions(&doc));
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Undo/redo with table edits
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn snapshot_block_at_position_finds_cell_blocks() {
    let doc = doc_with_table_and_text();

    // Get cell (0,0) position from full snapshot
    let (cell_pos, _cell_len) = cell_block_position(&doc, 0, 0).expect("cell (0,0)");

    // snapshot_block_at_position should find this cell block
    let snap = doc
        .snapshot_block_at_position(cell_pos)
        .expect("should find block at cell position");
    assert_eq!(
        snap.position, cell_pos,
        "snapshot position should match cell position"
    );
    assert!(
        snap.table_cell.is_some(),
        "block should have table_cell context"
    );
}

#[test]
fn snapshot_block_at_position_finds_cell_after_edit() {
    let doc = doc_with_table_and_text();
    let (cell_pos, cell_len) = cell_block_position(&doc, 0, 0).expect("cell (0,0)");

    // Insert text
    let cursor = doc.cursor_at(cell_pos + cell_len);
    cursor.insert_text("XYZ").unwrap();

    // snapshot_block_at_position should still find the edited cell block
    let snap = doc
        .snapshot_block_at_position(cell_pos)
        .expect("should find block at cell position after edit");
    assert!(
        snap.text.contains("XYZ"),
        "snapshot text should contain inserted text, got {:?}",
        snap.text
    );
}

#[test]
fn insert_in_empty_cell_positions_stay_valid() {
    let doc = doc_with_empty_table();
    let positions = all_block_positions(&doc);
    assert_no_overlaps(&positions);

    // Find an empty cell and type in it
    if let Some((pos, len)) = cell_block_position(&doc, 0, 0) {
        assert_eq!(len, 0, "empty table cell should have length 0");
        let cursor = doc.cursor_at(pos);
        cursor.insert_text("Hello").unwrap();
        assert_no_overlaps(&all_block_positions(&doc));
    }
}

#[test]
fn undo_insert_in_cell_restores_positions() {
    let doc = doc_with_table_and_text();
    let positions_before = all_block_positions(&doc);

    let (pos, len) = cell_block_position(&doc, 0, 0).expect("cell (0,0)");
    let cursor = doc.cursor_at(pos + len);
    cursor.insert_text("XYZ").unwrap();

    doc.undo().unwrap();

    let positions_after = all_block_positions(&doc);
    assert_eq!(
        positions_before.len(),
        positions_after.len(),
        "block count should match after undo"
    );
    for (before, after) in positions_before.iter().zip(positions_after.iter()) {
        assert_eq!(
            before.0, after.0,
            "position mismatch after undo: {:?} vs {:?}",
            before, after
        );
        assert_eq!(
            before.2, after.2,
            "text mismatch after undo: {:?} vs {:?}",
            before, after
        );
    }
}
