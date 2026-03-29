use text_document::{FlowElement, MoveMode, SelectionKind, TextDocument};

fn new_doc_with_table() -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text("Before").unwrap();
    let cursor = doc.cursor_at(6);
    cursor.insert_table(3, 2).unwrap();
    doc
}

fn find_table(doc: &TextDocument) -> Option<text_document::TextTable> {
    doc.flow().into_iter().find_map(|e| match e {
        FlowElement::Table(t) => Some(t),
        _ => None,
    })
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TextTable
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn table_exists_in_flow() {
    let doc = new_doc_with_table();
    assert!(find_table(&doc).is_some(), "expected a Table in the flow");
}

#[test]
fn table_rows_and_columns() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    assert_eq!(table.rows(), 3);
    assert_eq!(table.columns(), 2);
}

#[test]
fn table_id_is_stable() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let id1 = table.id();
    let id2 = table.id();
    assert_eq!(id1, id2);
    assert!(id1 > 0);
}

#[test]
fn table_cell_at_valid_position() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    assert!(table.cell(0, 0).is_some());
    assert!(table.cell(2, 1).is_some());
}

#[test]
fn table_cell_coordinates() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let cell = table.cell(1, 1).unwrap();
    assert_eq!(cell.row(), 1);
    assert_eq!(cell.column(), 1);
}

#[test]
fn table_cell_out_of_range() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    assert!(table.cell(99, 99).is_none());
}

#[test]
fn table_cell_default_span() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let cell = table.cell(0, 0).unwrap();
    assert_eq!(cell.row_span(), 1);
    assert_eq!(cell.column_span(), 1);
}

#[test]
fn table_format_does_not_panic() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let _fmt = table.format();
}

#[test]
fn table_column_widths() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let _widths = table.column_widths();
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TextTableCell
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn cell_blocks_non_empty() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let cell = table.cell(0, 0).unwrap();
    assert!(
        !cell.blocks().is_empty(),
        "cell should have at least one block"
    );
}

#[test]
fn cell_snapshot_blocks() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let cell = table.cell(0, 0).unwrap();
    let snaps = cell.snapshot_blocks();
    assert!(!snaps.is_empty());
}

#[test]
fn cell_format_default() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let cell = table.cell(0, 0).unwrap();
    let fmt = cell.format();
    assert_eq!(fmt.padding, None);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Table snapshot
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn table_snapshot_structure() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let snap = table.snapshot();
    assert_eq!(snap.rows, 3);
    assert_eq!(snap.columns, 2);
    assert_eq!(snap.cells.len(), 6);
}

#[test]
fn table_snapshot_cell_coordinates() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let snap = table.snapshot();
    for row in 0..3 {
        for col in 0..2 {
            let found = snap.cells.iter().any(|c| c.row == row && c.column == col);
            assert!(found, "cell ({row},{col}) should exist in snapshot");
        }
    }
}

#[test]
fn table_snapshot_cells_have_blocks() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let snap = table.snapshot();
    for cell in &snap.cells {
        assert!(
            !cell.blocks.is_empty(),
            "cell ({},{}) should have blocks",
            cell.row,
            cell.column
        );
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TextBlock::table_cell() integration
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn block_in_cell_returns_table_cell_ref() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let cell = table.cell(1, 0).unwrap();
    let blocks = cell.blocks();
    assert!(!blocks.is_empty());

    let cell_ref = blocks[0].table_cell();
    assert!(
        cell_ref.is_some(),
        "block in table should return TableCellRef"
    );
    let cell_ref = cell_ref.unwrap();
    assert_eq!(cell_ref.row, 1);
    assert_eq!(cell_ref.column, 0);
    assert_eq!(cell_ref.table.id(), table.id());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// insert_table returns TextTable handle
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn insert_table_returns_handle() {
    let doc = TextDocument::new();
    doc.set_plain_text("Hello").unwrap();
    let cursor = doc.cursor_at(5);
    let table = cursor.insert_table(2, 3).unwrap();
    assert_eq!(table.rows(), 2);
    assert_eq!(table.columns(), 3);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// current_table / current_table_cell
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn current_table_none_in_regular_text() {
    let doc = TextDocument::new();
    doc.set_plain_text("Hello").unwrap();
    let cursor = doc.cursor_at(0);
    assert!(cursor.current_table().is_none());
}

#[test]
fn current_table_cell_none_in_regular_text() {
    let doc = TextDocument::new();
    doc.set_plain_text("Hello").unwrap();
    let cursor = doc.cursor_at(0);
    assert!(cursor.current_table_cell().is_none());
}

/// Bug 1: Cursor at the end of the block *before* a table must NOT
/// be treated as inside the table.
#[test]
fn current_table_cell_none_at_end_of_block_before_table() {
    let doc = new_doc_with_table(); // "Before" + 3x2 table
    // Position 6 = end of "Before" (len 6, positions 0-5, cursor at 6 = after last char)
    let cursor = doc.cursor_at(6);
    assert!(
        cursor.current_table_cell().is_none(),
        "Cursor at end of block before table should NOT be inside a table cell"
    );
}

/// Bug 2: Cursor at end of cell block must report the CURRENT cell, not the next one.
/// Use markdown to create a table with cell content so positions are realistic.
#[test]
fn current_table_cell_at_end_of_cell_block() {
    let doc = TextDocument::new();
    doc.set_markdown("Before\n\n| A | B |\n|---|---|\n| c | d |\n\nAfter")
        .unwrap()
        .wait()
        .unwrap();

    let table = find_table(&doc).unwrap();

    // Find cell (0,0) block with content "A"
    let cell_0_0 = table.cell(0, 0).unwrap();
    let blocks = cell_0_0.snapshot_blocks();
    let block_0_0 = &blocks[0];
    assert_eq!(block_0_0.text, "A");

    // Cursor at end of cell (0,0) block (after "A")
    let end_pos = block_0_0.position + block_0_0.length;
    let cursor = doc.cursor_at(end_pos);
    let cell_ref = cursor
        .current_table_cell()
        .expect("cursor at end of cell block should be in a cell");
    assert_eq!(
        cell_ref.column, 0,
        "cursor at end of cell(0,0) should report column 0, got column {}",
        cell_ref.column
    );
    assert_eq!(cell_ref.row, 0);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Clone
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn table_is_clone() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let cloned = table.clone();
    assert_eq!(table.id(), cloned.id());
    assert_eq!(table.rows(), cloned.rows());
}

#[test]
fn table_cell_is_clone() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let cell = table.cell(0, 0).unwrap();
    let cloned = cell.clone();
    assert_eq!(cell.row(), cloned.row());
    assert_eq!(cell.column(), cloned.column());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Explicit-ID table structure mutations
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn insert_table_row_increases_count() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let cursor = doc.cursor();
    cursor.insert_table_row(table.id(), 1).unwrap();
    assert_eq!(table.rows(), 4); // was 3, now 4
}

#[test]
fn insert_table_column_increases_count() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let cursor = doc.cursor();
    cursor.insert_table_column(table.id(), 0).unwrap();
    assert_eq!(table.columns(), 3); // was 2, now 3
}

#[test]
fn remove_table_row_decreases_count() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let cursor = doc.cursor();
    cursor.remove_table_row(table.id(), 0).unwrap();
    assert_eq!(table.rows(), 2); // was 3, now 2
}

#[test]
fn remove_table_column_decreases_count() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let cursor = doc.cursor();
    cursor.remove_table_column(table.id(), 0).unwrap();
    assert_eq!(table.columns(), 1); // was 2, now 1
}

#[test]
fn remove_table_removes_from_flow() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let cursor = doc.cursor();
    cursor.remove_table(table.id()).unwrap();
    assert!(find_table(&doc).is_none(), "table should be gone from flow");
}

#[test]
fn insert_table_row_is_undoable() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let cursor = doc.cursor();
    cursor.insert_table_row(table.id(), 1).unwrap();
    assert_eq!(table.rows(), 4);
    doc.undo().unwrap();
    assert_eq!(table.rows(), 3);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Table formatting
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn set_table_format_border() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let cursor = doc.cursor();
    let fmt = text_document::TableFormat {
        border: Some(2),
        ..Default::default()
    };
    cursor.set_table_format(table.id(), &fmt).unwrap();
    let read_fmt = table.format();
    assert_eq!(read_fmt.border, Some(2));
}

#[test]
fn set_table_cell_format_background() {
    let doc = new_doc_with_table();
    let table = find_table(&doc).unwrap();
    let cell = table.cell(0, 0).unwrap();
    let cursor = doc.cursor();
    let fmt = text_document::CellFormat {
        background_color: Some("#ff0000".to_string()),
        ..Default::default()
    };
    cursor.set_table_cell_format(cell.id(), &fmt).unwrap();
    let read_fmt = cell.format();
    assert_eq!(read_fmt.background_color, Some("#ff0000".to_string()));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Position-based convenience methods
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn convenience_not_in_table_errors() {
    let doc = TextDocument::new();
    doc.set_plain_text("Hello").unwrap();
    let cursor = doc.cursor_at(0);
    assert!(cursor.remove_current_table().is_err());
    assert!(cursor.insert_row_above().is_err());
    assert!(cursor.insert_row_below().is_err());
    assert!(cursor.insert_column_before().is_err());
    assert!(cursor.insert_column_after().is_err());
    assert!(cursor.remove_current_row().is_err());
    assert!(cursor.remove_current_column().is_err());
    assert!(cursor.merge_selected_cells().is_err());
    assert!(cursor.split_current_cell(2, 2).is_err());
    assert!(
        cursor
            .set_current_table_format(&Default::default())
            .is_err()
    );
    assert!(cursor.set_current_cell_format(&Default::default()).is_err());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Mixed selection (text + table) — Word-style behaviour
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Helper: create "Before" + 3×2 table with "CellText" in cell(0,0).
/// Returns (doc, position inside cell(0,0) text).
fn new_doc_with_text_and_table() -> (TextDocument, usize) {
    let doc = TextDocument::new();
    doc.set_plain_text("Before").unwrap();
    // Insert table at end of "Before"
    let cursor = doc.cursor_at(6);
    cursor.insert_table(3, 2).unwrap();

    // Use the snapshot to find cell(0,0) block position
    let snap = doc.snapshot_flow();
    let cell_pos = snap
        .elements
        .iter()
        .find_map(|e| {
            if let text_document::FlowElementSnapshot::Table(ts) = e {
                ts.cells
                    .iter()
                    .find(|c| c.row == 0 && c.column == 0)
                    .map(|c| c.blocks[0].position)
            } else {
                None
            }
        })
        .expect("cell(0,0) block should exist");

    // Type text into cell(0,0)
    let c2 = doc.cursor_at(cell_pos);
    c2.insert_text("CellText").unwrap();

    // Re-read actual position after text insertion
    let snap2 = doc.snapshot_flow();
    let actual_cell_pos = snap2
        .elements
        .iter()
        .find_map(|e| {
            if let text_document::FlowElementSnapshot::Table(ts) = e {
                ts.cells
                    .iter()
                    .find(|c| c.row == 0 && c.column == 0)
                    .map(|c| c.blocks[0].position)
            } else {
                None
            }
        })
        .expect("cell(0,0) block should exist after text insertion");

    (doc, actual_cell_pos)
}

#[test]
fn mixed_selection_text_before_table_selects_full_table() {
    let (doc, cell_pos) = new_doc_with_text_and_table();
    let table = find_table(&doc).unwrap();
    let rows = table.rows();
    let cols = table.columns();

    // Anchor at start of "Before", position inside the first cell
    let cursor = doc.cursor_at(0);
    cursor.set_position(cell_pos + 1, MoveMode::KeepAnchor);

    let kind = cursor.selection_kind();
    match kind {
        SelectionKind::Mixed {
            cell_range,
            text_before,
            text_after,
        } => {
            assert!(text_before, "text is before the table");
            assert!(!text_after);
            assert_eq!(cell_range.start_row, 0);
            assert_eq!(cell_range.start_col, 0);
            assert_eq!(cell_range.end_row, rows - 1);
            assert_eq!(cell_range.end_col, cols - 1);
        }
        other => panic!("expected Mixed, got {:?}", other),
    }
}

#[test]
fn mixed_selection_text_after_table_selects_full_table() {
    // "Before" + table, then select from inside cell to "Before" text
    // (which is "after" the cell from the selection's perspective — anchor
    // in cell, position in the text block before the table)
    let (doc, cell_pos) = new_doc_with_text_and_table();
    let table = find_table(&doc).unwrap();
    let rows = table.rows();
    let cols = table.columns();

    // Anchor at start of "Before" (outside table), position inside cell
    // → text_before=true.  Now flip: anchor inside cell, position at 0.
    let cursor = doc.cursor_at(cell_pos + 1);
    cursor.set_position(0, MoveMode::KeepAnchor);

    let kind = cursor.selection_kind();
    match kind {
        SelectionKind::Mixed {
            cell_range,
            text_before,
            text_after,
        } => {
            // The outside position (0) is BEFORE the inside position (cell)
            assert!(
                text_before || text_after,
                "one of text_before/text_after should be true"
            );
            assert_eq!(cell_range.start_row, 0);
            assert_eq!(cell_range.start_col, 0);
            assert_eq!(cell_range.end_row, rows - 1);
            assert_eq!(cell_range.end_col, cols - 1);
        }
        other => panic!("expected Mixed, got {:?}", other),
    }
}

#[test]
fn mixed_extract_fragment_includes_blocks_and_table() {
    let (doc, cell_pos) = new_doc_with_text_and_table();

    let cursor = doc.cursor_at(0);
    cursor.set_position(cell_pos + 1, MoveMode::KeepAnchor);

    let frag = cursor.selection();
    assert!(!frag.is_empty(), "fragment should not be empty");

    let html = frag.to_html();
    assert!(
        html.contains("<table>"),
        "should contain table in HTML: {}",
        html
    );
    assert!(
        html.contains("Before"),
        "should contain 'Before' text: {}",
        html
    );
    // 3×2 table = 3 rows
    assert!(
        html.matches("<tr>").count() >= 3,
        "table should have 3 rows: {}",
        html
    );
}

#[test]
fn mixed_extract_fragment_plain_text_includes_all_content() {
    let (doc, cell_pos) = new_doc_with_text_and_table();

    let cursor = doc.cursor_at(0);
    cursor.set_position(cell_pos + 1, MoveMode::KeepAnchor);

    let frag = cursor.selection();
    let plain = frag.to_plain_text();
    assert!(
        plain.contains("Before"),
        "plain text should contain 'Before', got: {}",
        plain
    );
    // Table cells are also extracted in full
    assert!(
        plain.contains("CellText"),
        "plain text should contain 'CellText' from cell(0,0), got: {}",
        plain
    );
}

#[test]
fn mixed_fragment_to_html_contains_table() {
    let (doc, cell_pos) = new_doc_with_text_and_table();

    let cursor = doc.cursor_at(0);
    cursor.set_position(cell_pos + 1, MoveMode::KeepAnchor);

    let frag = cursor.selection();
    let html = frag.to_html();

    assert!(
        html.contains("<table>"),
        "HTML should contain <table>, got: {}",
        html
    );
    assert!(
        html.contains("<p>"),
        "HTML should contain <p> for the text block, got: {}",
        html
    );
}

#[test]
fn mixed_fragment_to_markdown_contains_table() {
    let (doc, cell_pos) = new_doc_with_text_and_table();

    let cursor = doc.cursor_at(0);
    cursor.set_position(cell_pos + 1, MoveMode::KeepAnchor);

    let frag = cursor.selection();
    let md = frag.to_markdown();

    assert!(
        md.contains("|"),
        "Markdown should contain pipe for table, got: {}",
        md
    );
}

#[test]
fn mixed_fragment_html_has_text_before_table() {
    let (doc, cell_pos) = new_doc_with_text_and_table();

    let cursor = doc.cursor_at(0);
    cursor.set_position(cell_pos + 1, MoveMode::KeepAnchor);

    let frag = cursor.selection();
    let html = frag.to_html();

    let before_pos = html.find("Before").expect("should contain 'Before'");
    let table_pos = html.find("<table>").expect("should contain '<table>'");
    assert!(
        before_pos < table_pos,
        "'Before' text should appear before <table> in HTML"
    );
}
