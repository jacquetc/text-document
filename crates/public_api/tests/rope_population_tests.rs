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
