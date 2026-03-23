use text_document::{FlowElement, TextDocument};

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
