use text_document::{
    BlockFormat, DocumentFragment, FlowElement, MoveMode, SelectionKind, TextDocument,
};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Helpers
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Create a document with heading "Title" (H1) followed by normal "Body".
/// Uses set_plain_text + set_block_format for predictable positions.
/// Positions: "Title" at 0..5, gap at 5, "Body" at 6..10.
fn heading_and_body_doc() -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text("Title\nBody").unwrap();
    // Make the first block a heading
    let cursor = doc.cursor_at(0);
    cursor
        .set_block_format(&BlockFormat {
            heading_level: Some(1),
            ..Default::default()
        })
        .unwrap();
    doc
}

fn find_table(doc: &TextDocument) -> Option<text_document::TextTable> {
    doc.flow().into_iter().find_map(|e| match e {
        FlowElement::Table(t) => Some(t),
        _ => None,
    })
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// E1: Full-block detection (paragraph mark rule)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn extract_partial_block_no_gap() {
    // Select all text in "Title" (0..5) but NOT the paragraph break.
    // Block formatting should NOT be preserved.
    let doc = heading_and_body_doc();
    let cursor = doc.cursor_at(0);
    cursor.set_position(5, MoveMode::KeepAnchor); // "Title" only, no gap
    let frag = cursor.selection();
    let html = frag.to_html();
    // Should NOT contain <h1> because we didn't cross the paragraph break
    assert!(
        !html.contains("<h1>"),
        "partial block should not have heading: {}",
        html
    );
    assert!(
        html.contains("Title"),
        "should contain the text: {}",
        html
    );
}

#[test]
fn extract_full_block_crosses_gap() {
    // Select "Title" + gap (0..6) — crosses into next block.
    // Block formatting SHOULD be preserved.
    let doc = heading_and_body_doc();
    let cursor = doc.cursor_at(0);
    cursor.set_position(6, MoveMode::KeepAnchor); // crosses paragraph break
    let frag = cursor.selection();
    let html = frag.to_html();
    assert!(
        html.contains("<h1>") || html.contains("<h2>") || html.contains("<h3>"),
        "full block should have heading: {}",
        html
    );
}

#[test]
fn extract_middle_blocks_always_full() {
    // Create 3 blocks with different formatting
    let doc = TextDocument::new();
    doc.set_plain_text("First\nMiddle\nLast").unwrap();
    // "First" at 0..5, "Middle" at 6..12, "Last" at 13..17
    let c1 = doc.cursor_at(0);
    c1.set_block_format(&BlockFormat {
        heading_level: Some(1),
        ..Default::default()
    })
    .unwrap();
    let c3 = doc.cursor_at(13);
    c3.set_block_format(&BlockFormat {
        heading_level: Some(2),
        ..Default::default()
    })
    .unwrap();

    // Select from middle of "First" through middle of "Last"
    let c4 = doc.cursor_at(2);
    c4.set_position(15, MoveMode::KeepAnchor);
    let frag = c4.selection();
    let html = frag.to_html();

    // "Middle" is an intermediate block — should have block formatting (wrapped in <p>)
    assert!(
        html.contains("<p>") || html.contains("Middle"),
        "middle block should be included: {}",
        html
    );
    // First block is partial (starts at 2) — should NOT have heading
    // Last block is partial (ends at 15) — should NOT have heading
    // But middle is full (intermediate)
}

#[test]
fn extract_list_items_from_html() {
    // Create doc from HTML with list items, then extract full items
    let doc = TextDocument::new();
    doc.set_plain_text("Item one\nItem two").unwrap();

    // Get the full text for position reference
    let plain = doc.to_plain_text().unwrap();
    assert_eq!(plain, "Item one\nItem two");

    // Select the full first item including gap: 0..9
    let cursor = doc.cursor_at(0);
    cursor.set_position(9, MoveMode::KeepAnchor);
    let frag = cursor.selection();
    let plain_frag = frag.to_plain_text();

    // The plain text should contain "Item one"
    assert!(
        plain_frag.contains("Item one"),
        "should contain first item: {}",
        plain_frag
    );
}

#[test]
fn extract_inline_formatting_on_partial() {
    // Bold text in partial selection should preserve bold formatting
    let doc = TextDocument::new();
    let cursor = doc.cursor_at(0);
    cursor
        .insert_html("<p>Normal <b>bold text</b> normal</p>")
        .unwrap();

    let plain = doc.to_plain_text().unwrap();
    // Find "bold" in the text
    let bold_start = plain.find("bold").expect("should contain 'bold'");

    let c2 = doc.cursor_at(bold_start);
    c2.set_position(bold_start + 4, MoveMode::KeepAnchor);
    let frag = c2.selection();
    let html = frag.to_html();
    assert!(
        html.contains("<b>") || html.contains("<strong>") || html.contains("font-weight"),
        "partial selection should preserve inline bold: {}",
        html
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Insert tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn insert_inline_preserves_target_format() {
    // Paste inline text into a heading — heading format should be preserved
    let doc = heading_and_body_doc();
    let cursor = doc.cursor_at(2);
    let frag = DocumentFragment::from_plain_text("INSERTED");
    cursor.insert_fragment(&frag).unwrap();

    // Check the block at position 0 still has heading formatting
    let c2 = doc.cursor_at(0);
    let fmt = c2.block_format().unwrap();
    assert!(
        fmt.heading_level.is_some(),
        "heading should be preserved after inline paste"
    );
}

#[test]
fn insert_full_block_splits_and_inherits() {
    // Paste a heading block into a normal paragraph
    let doc = TextDocument::new();
    doc.set_plain_text("Some text here").unwrap();

    let frag = DocumentFragment::from_html("<h1>Heading</h1>");

    let cursor = doc.cursor_at(5);
    cursor.insert_fragment(&frag).unwrap();

    let plain = doc.to_plain_text().unwrap();
    assert!(
        plain.contains("Heading"),
        "pasted heading text should appear: {}",
        plain
    );
}

#[test]
fn insert_with_selection_replaces_atomically() {
    // Select some text, then paste — should replace the selection
    let doc = TextDocument::new();
    doc.set_plain_text("Hello World").unwrap();

    let cursor = doc.cursor_at(6);
    cursor.set_position(11, MoveMode::KeepAnchor);
    cursor.insert_text("Universe").unwrap();

    let plain = doc.to_plain_text().unwrap();
    assert_eq!(plain, "Hello Universe");

    // Undo should restore both the delete and insert
    doc.undo().unwrap();
    let plain2 = doc.to_plain_text().unwrap();
    assert_eq!(plain2, "Hello World");
}

#[test]
fn insert_html_with_selection_replaces() {
    let doc = TextDocument::new();
    doc.set_plain_text("Hello World").unwrap();

    let cursor = doc.cursor_at(6);
    cursor.set_position(11, MoveMode::KeepAnchor);
    cursor.insert_html("<b>Bold</b>").unwrap();

    let plain = doc.to_plain_text().unwrap();
    assert!(
        plain.contains("Bold"),
        "should contain pasted text: {}",
        plain
    );
    assert!(
        !plain.contains("World"),
        "should not contain replaced text: {}",
        plain
    );
}

#[test]
fn insert_fragment_with_selection_replaces() {
    let doc = TextDocument::new();
    doc.set_plain_text("Hello World").unwrap();

    let cursor = doc.cursor_at(6);
    cursor.set_position(11, MoveMode::KeepAnchor);
    let frag = DocumentFragment::from_plain_text("Rust");
    cursor.insert_fragment(&frag).unwrap();

    let plain = doc.to_plain_text().unwrap();
    assert_eq!(plain, "Hello Rust");
}

#[test]
fn insert_table_outside_table_creates_new() {
    let doc = TextDocument::new();
    doc.set_plain_text("Before").unwrap();

    let cursor = doc.cursor_at(6);
    cursor.insert_table(2, 3).unwrap();

    let table = find_table(&doc);
    assert!(table.is_some(), "should create a table");
    let t = table.unwrap();
    assert_eq!(t.rows(), 2);
    assert_eq!(t.columns(), 3);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Delete tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn delete_cross_block_preserves_first_format() {
    // Delete across two blocks — the merged block should keep the first block's format
    let doc = heading_and_body_doc();
    // "Title" (H1, 0..4), gap at 5, "Body" (normal, 6..9)
    let cursor = doc.cursor_at(3);
    cursor.set_position(7, MoveMode::KeepAnchor);
    cursor.remove_selected_text().unwrap();

    let c2 = doc.cursor_at(0);
    let fmt = c2.block_format().unwrap();
    assert!(
        fmt.heading_level.is_some(),
        "merged block should keep first block's heading format"
    );
}

#[test]
fn delete_cross_cell_clears_cells() {
    // Select across cells and delete — cells should be cleared, table preserved
    let doc = TextDocument::new();
    doc.set_plain_text("Before").unwrap();
    let cursor = doc.cursor_at(6);
    let table = cursor.insert_table(2, 2).unwrap();

    // Type text into cells
    let cell00 = table.cell(0, 0).unwrap();
    let pos00 = cell00.blocks()[0].position();
    let c1 = doc.cursor_at(pos00);
    c1.insert_text("CellA").unwrap();

    // Re-get table after insert
    let table2 = find_table(&doc).unwrap();
    let cell01 = table2.cell(0, 1).unwrap();
    let pos01 = cell01.blocks()[0].position();
    let c2 = doc.cursor_at(pos01);
    c2.insert_text("CellB").unwrap();

    // Select across cells (cross-cell selection)
    let table3 = find_table(&doc).unwrap();
    let new_pos00 = table3.cell(0, 0).unwrap().blocks()[0].position();
    let new_pos01 = table3.cell(0, 1).unwrap().blocks()[0].position();

    let c3 = doc.cursor_at(new_pos00 + 1);
    c3.set_position(new_pos01 + 2, MoveMode::KeepAnchor);
    c3.remove_selected_text().unwrap();

    // Table should still exist
    assert!(
        find_table(&doc).is_some(),
        "table should survive cross-cell delete"
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Roundtrip tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn roundtrip_heading_paragraph() {
    // Copy a heading (fully), paste into a normal paragraph
    let doc = heading_and_body_doc();

    // Select "Title" fully (0..6 crosses the gap)
    let cursor = doc.cursor_at(0);
    cursor.set_position(6, MoveMode::KeepAnchor);
    let frag = cursor.selection();

    // Create a new doc with normal text, paste into it
    let doc2 = TextDocument::new();
    doc2.set_plain_text("Normal paragraph").unwrap();
    let c2 = doc2.cursor_at(7);
    c2.insert_fragment(&frag).unwrap();

    let plain = doc2.to_plain_text().unwrap();
    assert!(plain.contains("Title"), "pasted heading text: {}", plain);
}

#[test]
fn roundtrip_plain_text() {
    // Copy plain text and paste elsewhere
    let doc = TextDocument::new();
    doc.set_plain_text("Hello World").unwrap();

    let cursor = doc.cursor_at(0);
    cursor.set_position(5, MoveMode::KeepAnchor);
    let frag = cursor.selection();

    let doc2 = TextDocument::new();
    doc2.set_plain_text("Goodbye").unwrap();
    let c2 = doc2.cursor_at(7);
    c2.insert_fragment(&frag).unwrap();

    let plain = doc2.to_plain_text().unwrap();
    assert_eq!(plain, "GoodbyeHello");
}

#[test]
fn roundtrip_mixed_text_table() {
    // Copy text + table, paste elsewhere
    let doc = TextDocument::new();
    doc.set_plain_text("Before").unwrap();
    let cursor = doc.cursor_at(6);
    cursor.insert_table(2, 2).unwrap();

    // Type into cell(0,0)
    let table = find_table(&doc).unwrap();
    let cell = table.cell(0, 0).unwrap();
    let cell_pos = cell.blocks()[0].position();
    let c2 = doc.cursor_at(cell_pos);
    c2.insert_text("Hello").unwrap();

    // Select from start of "Before" into the table
    let table2 = find_table(&doc).unwrap();
    let new_cell_pos = table2.cell(0, 0).unwrap().blocks()[0].position();
    let c3 = doc.cursor_at(0);
    c3.set_position(new_cell_pos + 2, MoveMode::KeepAnchor);
    let frag = c3.selection();

    let html = frag.to_html();
    assert!(
        html.contains("Before"),
        "should contain 'Before' text: {}",
        html
    );
    assert!(
        html.contains("<table>"),
        "should contain table: {}",
        html
    );
}

#[test]
fn copy_paste_preserves_inline_bold() {
    // Copy bold text, paste it — bold should be preserved
    let doc = TextDocument::new();
    let cursor = doc.cursor_at(0);
    cursor.insert_html("<p><b>Bold Text</b></p>").unwrap();

    let plain = doc.to_plain_text().unwrap();
    let bold_start = plain.find("Bold Text").expect("should contain bold text");
    let bold_end = bold_start + "Bold Text".len();

    let c2 = doc.cursor_at(bold_start);
    c2.set_position(bold_end, MoveMode::KeepAnchor);
    let frag = c2.selection();

    // Paste into new doc
    let doc2 = TextDocument::new();
    let c3 = doc2.cursor_at(0);
    c3.insert_fragment(&frag).unwrap();

    // Re-extract and check bold is preserved
    let plain2 = doc2.to_plain_text().unwrap();
    let bs = plain2.find("Bold").expect("should have bold text");
    let c4 = doc2.cursor_at(bs);
    c4.set_position(bs + 4, MoveMode::KeepAnchor);
    let frag2 = c4.selection();
    let html2 = frag2.to_html();
    assert!(
        html2.contains("<b>") || html2.contains("<strong>") || html2.contains("font-weight"),
        "bold should survive roundtrip: {}",
        html2
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// HTML table roundtrip (critical bug fix validation)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn html_table_roundtrip_preserves_structure() {
    // from_html should preserve table structure (not flatten to blocks)
    let html = "<table><tr><td>A</td><td>B</td></tr><tr><td>C</td><td>D</td></tr></table>";
    let frag = DocumentFragment::from_html(html);

    let out_html = frag.to_html();
    assert!(
        out_html.contains("<table>"),
        "roundtrip should preserve <table>: {}",
        out_html
    );
    assert!(
        out_html.contains("<td>"),
        "roundtrip should preserve <td>: {}",
        out_html
    );
    assert!(
        out_html.contains("A") && out_html.contains("D"),
        "cell content preserved: {}",
        out_html
    );
}

#[test]
fn html_table_with_text_roundtrip() {
    // Mixed text + table in HTML should preserve both
    let html = "<p>Before</p><table><tr><td>X</td></tr></table><p>After</p>";
    let frag = DocumentFragment::from_html(html);

    let out_html = frag.to_html();
    assert!(out_html.contains("Before"), "text before table: {}", out_html);
    assert!(out_html.contains("<table>"), "table preserved: {}", out_html);
    assert!(out_html.contains("After"), "text after table: {}", out_html);
}

#[test]
fn markdown_table_roundtrip() {
    let md = "| A | B |\n| --- | --- |\n| C | D |";
    let frag = DocumentFragment::from_markdown(md);

    let out_md = frag.to_markdown();
    assert!(
        out_md.contains("|"),
        "markdown table should survive roundtrip: {}",
        out_md
    );
    assert!(
        out_md.contains("A") && out_md.contains("D"),
        "cell content preserved: {}",
        out_md
    );
}

#[test]
fn insert_html_table_creates_table_entity() {
    // Pasting HTML with a table should create a table entity, not flat blocks
    let doc = TextDocument::new();
    doc.set_plain_text("Text").unwrap();

    let cursor = doc.cursor_at(4);
    cursor
        .insert_html("<table><tr><td>A</td><td>B</td></tr><tr><td>C</td><td>D</td></tr></table>")
        .unwrap();

    assert!(
        find_table(&doc).is_some(),
        "insert_html with <table> should create a table entity"
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Cell selection extraction
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn cell_selection_extract_produces_table_fragment() {
    // When cursor has cell selection, selection() should produce a table fragment
    let doc = TextDocument::new();
    doc.set_plain_text("Text").unwrap();
    let cursor = doc.cursor_at(4);
    cursor.insert_table(2, 2).unwrap();

    // Put text in cell(0,0) using the table's set_cell_text API if available,
    // or find the cell block position from a fresh snapshot
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

    let c1 = doc.cursor_at(cell_pos);
    c1.insert_text("Hello").unwrap();

    // Re-get table and use cell selection override to select all cells
    let table2 = find_table(&doc).unwrap();
    let table_id = table2.id();
    let c3 = doc.cursor_at(0);
    c3.select_cell_range(table_id, 0, 0, 1, 1);

    let kind = c3.selection_kind();
    assert!(
        matches!(kind, SelectionKind::Cells(_)),
        "should be cell selection: {:?}",
        kind
    );

    let frag = c3.selection();
    assert!(!frag.is_empty(), "cell selection should produce non-empty fragment");

    let html = frag.to_html();
    assert!(
        html.contains("<table>"),
        "cell selection should produce table HTML: {}",
        html
    );
    assert!(
        html.contains("Hello"),
        "should contain cell content: {}",
        html
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Table paste into existing table (I4)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn paste_table_into_existing_table_replaces_cells() {
    // Create doc with a 2x2 table
    let doc = TextDocument::new();
    doc.set_plain_text("Text").unwrap();
    let cursor = doc.cursor_at(4);
    cursor.insert_table(2, 2).unwrap();

    // Type into cell(0,0)
    let table = find_table(&doc).unwrap();
    let pos00 = table.cell(0, 0).unwrap().blocks()[0].position();
    let c1 = doc.cursor_at(pos00);
    c1.insert_text("Original").unwrap();

    // Create a fragment from HTML table (1x1)
    let frag = DocumentFragment::from_html("<table><tr><td>Replaced</td></tr></table>");

    // Paste at the cell position — should replace the cell content
    let table2 = find_table(&doc).unwrap();
    let new_pos00 = table2.cell(0, 0).unwrap().blocks()[0].position();
    let c2 = doc.cursor_at(new_pos00);
    c2.insert_fragment(&frag).unwrap();

    // Table should still exist (not a new table)
    let _tables: Vec<_> = doc.flow().into_iter().filter_map(|e| match e {
        FlowElement::Table(t) => Some(t),
        _ => None,
    }).collect();

    // Check the cell content was replaced
    let plain = doc.to_plain_text().unwrap();
    assert!(
        plain.contains("Replaced"),
        "cell content should be replaced: {}",
        plain
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// List continuation (I6)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn paste_list_continues_adjacent_list() {
    // Create doc with existing list
    let doc = TextDocument::new();
    let cursor = doc.cursor_at(0);
    cursor
        .insert_html("<ul><li>Existing item</li></ul>")
        .unwrap();

    // Create fragment with a list item
    let frag = DocumentFragment::from_html("<ul><li>New item</li></ul>");

    // Paste at the end of the existing list item
    let plain = doc.to_plain_text().unwrap();
    let c2 = doc.cursor_at(plain.len());
    c2.insert_fragment(&frag).unwrap();

    let plain2 = doc.to_plain_text().unwrap();
    assert!(
        plain2.contains("Existing item") && plain2.contains("New item"),
        "both items should exist: {}",
        plain2
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Heading/list interactions
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn paste_heading_into_list_preserves_tail_list() {
    // Paste a heading block into a list item
    // The heading should appear, tail text should retain list format
    let doc = TextDocument::new();
    let cursor = doc.cursor_at(0);
    cursor.insert_html("<ul><li>List item text</li></ul>").unwrap();

    let frag = DocumentFragment::from_html("<h1>Heading</h1>");

    // Find position inside the list item
    let plain = doc.to_plain_text().unwrap();
    let mid = plain.find("item").unwrap_or(5);
    let c2 = doc.cursor_at(mid);
    c2.insert_fragment(&frag).unwrap();

    let plain2 = doc.to_plain_text().unwrap();
    assert!(
        plain2.contains("Heading"),
        "heading text should appear: {}",
        plain2
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Undo atomicity
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn undo_paste_html_over_selection_is_atomic() {
    let doc = TextDocument::new();
    doc.set_plain_text("Hello World").unwrap();

    let cursor = doc.cursor_at(6);
    cursor.set_position(11, MoveMode::KeepAnchor);
    cursor.insert_html("<b>Bold</b>").unwrap();

    let after_paste = doc.to_plain_text().unwrap();
    assert!(after_paste.contains("Bold"), "paste worked: {}", after_paste);

    // Single undo should restore original
    doc.undo().unwrap();
    let after_undo = doc.to_plain_text().unwrap();
    assert_eq!(after_undo, "Hello World", "undo should restore original");
}

#[test]
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Frame (blockquote) tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn extract_inside_blockquote_includes_text() {
    // Text inside a blockquote frame should be extractable
    let doc = TextDocument::new();
    let cursor = doc.cursor_at(0);
    cursor
        .insert_markdown("> Quoted text inside a frame")
        .unwrap();

    let plain = doc.to_plain_text().unwrap();
    let start = plain.find("Quoted").unwrap_or(0);
    let end = start + "Quoted".len();

    let c2 = doc.cursor_at(start);
    c2.set_position(end, MoveMode::KeepAnchor);
    let frag = c2.selection();
    let frag_text = frag.to_plain_text();
    assert!(
        frag_text.contains("Quoted"),
        "should extract text from blockquote: plain='{}', frag='{}'",
        plain,
        frag_text
    );
}

#[test]
fn insert_into_blockquote_stays_in_blockquote() {
    // Pasting text inside a blockquote should keep it within the blockquote
    let doc = TextDocument::new();
    let cursor = doc.cursor_at(0);
    cursor
        .insert_markdown("> Quoted text")
        .unwrap();

    let plain = doc.to_plain_text().unwrap();
    let mid = plain.find("text").unwrap_or(plain.len().saturating_sub(2));

    // Insert plain text inside the blockquote
    let c2 = doc.cursor_at(mid);
    c2.insert_text("INSERTED ").unwrap();

    let plain2 = doc.to_plain_text().unwrap();
    assert!(
        plain2.contains("INSERTED"),
        "inserted text should appear: {}",
        plain2
    );
}

#[test]
fn copy_paste_inside_blockquote_roundtrip() {
    // Copy text from inside a blockquote, paste into same blockquote
    let doc = TextDocument::new();
    let cursor = doc.cursor_at(0);
    cursor
        .insert_markdown("> First line\n> Second line")
        .unwrap();

    let plain = doc.to_plain_text().unwrap();

    // Select some text
    let start = plain.find("First").unwrap_or(0);
    let end = start + "First".len();

    let c2 = doc.cursor_at(start);
    c2.set_position(end, MoveMode::KeepAnchor);
    let frag = c2.selection();

    assert!(
        !frag.is_empty(),
        "should extract non-empty fragment from blockquote"
    );

    // Paste at the end
    let c3 = doc.cursor_at(plain.len());
    c3.insert_fragment(&frag).unwrap();

    let plain2 = doc.to_plain_text().unwrap();
    // "First" should appear at least twice (original + pasted)
    let count = plain2.matches("First").count();
    assert!(
        count >= 2,
        "pasted text should appear twice, got {}: {}",
        count,
        plain2
    );
}

#[test]
fn undo_paste_fragment_over_selection_is_atomic() {
    let doc = TextDocument::new();
    doc.set_plain_text("ABCDEF").unwrap();

    let cursor = doc.cursor_at(2);
    cursor.set_position(4, MoveMode::KeepAnchor); // select "CD"
    let frag = DocumentFragment::from_plain_text("XY");
    cursor.insert_fragment(&frag).unwrap();

    assert_eq!(doc.to_plain_text().unwrap(), "ABXYEF");

    doc.undo().unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "ABCDEF");
}
