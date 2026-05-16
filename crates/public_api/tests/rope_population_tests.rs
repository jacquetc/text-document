//! Phase 2 step 5: verify markdown/html importers (which run via the
//! long-operation manager and so live behind the public API) populate
//! the global rope under `rope_backend`. Skipped under the default
//! backend (rope doesn't exist there).

#![cfg(feature = "rope_backend")]

use text_document::TextDocument;

#[test]
fn set_markdown_populates_rope() {
    let doc = TextDocument::new();
    doc.set_markdown("first paragraph\n\nsecond paragraph")
        .expect("set_markdown")
        .wait()
        .expect("wait");

    // Inspect the rope via the document's internal store.
    let store = doc.rope_store_for_test();
    let rope = store.rope.read().unwrap();
    let rope_text = rope.to_string();
    // Two blocks joined by an inter-block `\n`.
    assert_eq!(rope_text, "first paragraph\nsecond paragraph");

    let offsets = store.block_offsets.read().unwrap();
    assert_eq!(offsets.entries.len(), 2);
    assert_eq!(offsets.entries[0].1, 0);
    assert_eq!(offsets.entries[1].1, 16); // "first paragraph\n".len()
    assert_eq!(offsets.total_bytes(), 32);
}

#[test]
fn set_html_populates_rope() {
    let doc = TextDocument::new();
    doc.set_html("<p>alpha</p><p>beta</p><p>gamma</p>")
        .expect("set_html")
        .wait()
        .expect("wait");

    let store = doc.rope_store_for_test();
    let rope = store.rope.read().unwrap();
    assert_eq!(rope.to_string(), "alpha\nbeta\ngamma");

    let offsets = store.block_offsets.read().unwrap();
    assert_eq!(offsets.entries.len(), 3);
    assert_eq!(offsets.entries[0].1, 0);
    assert_eq!(offsets.entries[1].1, 6);  // "alpha\n".len()
    assert_eq!(offsets.entries[2].1, 11); // "alpha\nbeta\n".len()
    assert_eq!(offsets.total_bytes(), 16); // full "alpha\nbeta\ngamma"
}

#[test]
fn insert_text_at_position_mirrors_to_rope() {
    let doc = TextDocument::new();
    doc.set_plain_text("hello world").unwrap();

    let cursor = doc.cursor_at(5);
    cursor.insert_text(",").unwrap();

    let store = doc.rope_store_for_test();
    let rope = store.rope.read().unwrap();
    assert_eq!(rope.to_string(), "hello, world");
}

#[test]
fn insert_text_at_end_mirrors_to_rope() {
    let doc = TextDocument::new();
    doc.set_plain_text("hello").unwrap();

    let cursor = doc.cursor_at(5);
    cursor.insert_text(" world").unwrap();

    let store = doc.rope_store_for_test();
    let rope = store.rope.read().unwrap();
    assert_eq!(rope.to_string(), "hello world");
}

#[test]
fn insert_text_into_block_other_than_first_shifts_offsets() {
    // Multi-block doc; insert into block 2; verify block_offsets
    // for block 3 shift by inserted length.
    let doc = TextDocument::new();
    doc.set_plain_text("aaa\nbbb\nccc").unwrap();

    // block 0 [0..3), block 1 [4..7), block 2 [8..11)
    // Insert "XX" at char position 5 (inside block 1, after "b")
    let cursor = doc.cursor_at(5);
    cursor.insert_text("XX").unwrap();

    let store = doc.rope_store_for_test();
    let rope = store.rope.read().unwrap();
    assert_eq!(rope.to_string(), "aaa\nbXXbb\nccc");

    let offsets = store.block_offsets.read().unwrap();
    assert_eq!(offsets.entries.len(), 3);
    assert_eq!(offsets.entries[0].1, 0);
    assert_eq!(offsets.entries[1].1, 4);
    assert_eq!(offsets.entries[2].1, 10); // was 8, shifted by 2
    assert_eq!(offsets.total_bytes(), 13);
}

#[test]
fn delete_within_block_mirrors_to_rope() {
    let doc = TextDocument::new();
    doc.set_plain_text("hello, world").unwrap();

    // Delete the comma + space at positions 5..7
    let cursor = doc.cursor_at(5);
    cursor.set_position(7, text_document::MoveMode::KeepAnchor);
    cursor.remove_selected_text().unwrap();

    let store = doc.rope_store_for_test();
    let rope = store.rope.read().unwrap();
    assert_eq!(rope.to_string(), "helloworld");
}

#[test]
fn delete_within_middle_block_shifts_subsequent_offsets() {
    let doc = TextDocument::new();
    doc.set_plain_text("aaa\nbbbbb\nccc").unwrap();

    // block 0 [0..3), block 1 [4..9), block 2 [10..13)
    // Delete chars 5..7 ("bb") inside block 1.
    let cursor = doc.cursor_at(5);
    cursor.set_position(7, text_document::MoveMode::KeepAnchor);
    cursor.remove_selected_text().unwrap();

    let store = doc.rope_store_for_test();
    let rope = store.rope.read().unwrap();
    assert_eq!(rope.to_string(), "aaa\nbbb\nccc");

    let offsets = store.block_offsets.read().unwrap();
    assert_eq!(offsets.entries[0].1, 0);
    assert_eq!(offsets.entries[1].1, 4);
    assert_eq!(offsets.entries[2].1, 8); // was 10, shifted by -2
    assert_eq!(offsets.total_bytes(), 11);
}

#[test]
fn insert_image_inserts_object_replacement_sentinel() {
    let doc = TextDocument::new();
    doc.set_plain_text("ab").unwrap();
    let cursor = doc.cursor_at(1);
    cursor.insert_image("img1", 100, 100).unwrap();

    let store = doc.rope_store_for_test();
    let rope = store.rope.read().unwrap();
    // U+FFFC is 3 bytes in UTF-8. Original "ab" plus sentinel = 5 bytes.
    assert_eq!(rope.to_string(), "a\u{FFFC}b");
    assert_eq!(rope.len_bytes(), 5);

    let offsets = store.block_offsets.read().unwrap();
    assert_eq!(offsets.total_bytes(), 5);
}

#[test]
fn insert_formatted_text_at_position_mirrors_to_rope() {
    use text_document::TextFormat;

    let doc = TextDocument::new();
    doc.set_plain_text("hello world").unwrap();
    let cursor = doc.cursor_at(5);
    let fmt = TextFormat {
        font_bold: Some(true),
        ..Default::default()
    };
    cursor.insert_formatted_text(",", &fmt).unwrap();

    let store = doc.rope_store_for_test();
    let rope = store.rope.read().unwrap();
    assert_eq!(rope.to_string(), "hello, world");
}

#[test]
fn insert_block_splits_existing_block_in_rope() {
    let doc = TextDocument::new();
    doc.set_plain_text("hello world").unwrap();

    // Insert a block boundary at char position 5 (between "hello" and " world")
    let cursor = doc.cursor_at(5);
    cursor.insert_block().unwrap();

    let store = doc.rope_store_for_test();
    let rope = store.rope.read().unwrap();
    assert_eq!(rope.to_string(), "hello\n world");

    let offsets = store.block_offsets.read().unwrap();
    assert_eq!(offsets.entries.len(), 2);
    assert_eq!(offsets.entries[0].1, 0);
    assert_eq!(offsets.entries[1].1, 6); // after "hello\n"
    assert_eq!(offsets.total_bytes(), 12);
}

#[test]
fn cross_block_delete_merges_in_rope() {
    let doc = TextDocument::new();
    doc.set_plain_text("aaa\nbbb\nccc").unwrap();

    // block 0 [0..3), block 1 [4..7), block 2 [8..11) — total 11 bytes
    // Select from char 2 (inside block 0) through char 9 (inside block 2)
    // and delete. Expected merged result: "aacc" — block 0 keeps "aa",
    // block 2's "cc" merges in.
    let cursor = doc.cursor_at(2);
    cursor.set_position(9, text_document::MoveMode::KeepAnchor);
    cursor.remove_selected_text().unwrap();

    let store = doc.rope_store_for_test();
    let rope = store.rope.read().unwrap();
    assert_eq!(rope.to_string(), "aacc");

    let offsets = store.block_offsets.read().unwrap();
    // Two intermediate blocks (block 1 and block 2) removed from
    // index; only the merged block remains.
    assert_eq!(offsets.entries.len(), 1);
    assert_eq!(offsets.entries[0].1, 0);
    assert_eq!(offsets.total_bytes(), 4);
}

#[test]
fn set_html_table_cells_not_in_main_rope() {
    // Step 5.5 will properly add cell content to separate byte ranges.
    // For now, top-level prose is in the rope; cell-internal blocks
    // are NOT.
    let doc = TextDocument::new();
    doc.set_html("<p>before</p><table><tr><td>cell</td></tr></table><p>after</p>")
        .expect("set_html")
        .wait()
        .expect("wait");

    let store = doc.rope_store_for_test();
    let rope = store.rope.read().unwrap();
    // Only the top-level paragraphs are in the rope; the cell content
    // ("cell") lives in the cell's frame but is deferred from the
    // global rope until step 5.5.
    assert_eq!(rope.to_string(), "before\nafter");
}
