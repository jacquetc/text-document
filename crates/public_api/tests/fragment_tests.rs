use text_document::{DocumentFragment, MoveMode, MoveOperation, SelectionType, TextDocument};

fn new_doc_with_text(text: &str) -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text(text).unwrap();
    doc
}

// ── Construction ──────────────────────────────────────────────────

#[test]
fn new_fragment_is_empty() {
    let frag = DocumentFragment::new();
    assert!(frag.is_empty());
    assert_eq!(frag.to_plain_text(), "");
}

#[test]
fn default_fragment_is_empty() {
    let frag = DocumentFragment::default();
    assert!(frag.is_empty());
    assert_eq!(frag.to_plain_text(), "");
}

#[test]
fn from_plain_text() {
    let frag = DocumentFragment::from_plain_text("Hello world");
    assert!(!frag.is_empty());
    assert_eq!(frag.to_plain_text(), "Hello world");
}

#[test]
fn from_plain_text_empty() {
    let frag = DocumentFragment::from_plain_text("");
    assert!(frag.is_empty());
    assert_eq!(frag.to_plain_text(), "");
}

// ── From document ─────────────────────────────────────────────────

#[test]
fn from_document_captures_content() {
    let doc = new_doc_with_text("Hello world");
    let frag = DocumentFragment::from_document(&doc).unwrap();
    assert!(!frag.is_empty());
    assert_eq!(frag.to_plain_text(), "Hello world");
}

#[test]
fn from_empty_document() {
    let doc = TextDocument::new();
    let frag = DocumentFragment::from_document(&doc).unwrap();
    assert!(frag.is_empty());
}

// ── Extract via cursor selection ──────────────────────────────────

#[test]
fn extract_selection_as_fragment() {
    let doc = new_doc_with_text("Hello world");
    let cursor = doc.cursor();
    cursor.select(SelectionType::WordUnderCursor);
    let frag = cursor.selection();
    assert_eq!(frag.to_plain_text(), "Hello");
}

#[test]
fn extract_no_selection_returns_empty() {
    let doc = new_doc_with_text("Hello world");
    let cursor = doc.cursor();
    let frag = cursor.selection();
    assert!(frag.is_empty());
}

#[test]
fn extract_full_document_selection() {
    let doc = new_doc_with_text("Hello world");
    let cursor = doc.cursor();
    cursor.select(SelectionType::Document);
    let frag = cursor.selection();
    assert_eq!(frag.to_plain_text(), "Hello world");
}

// ── Insert fragment ───────────────────────────────────────────────

#[test]
fn insert_fragment_from_document() {
    let doc1 = new_doc_with_text("Source text");
    let frag = DocumentFragment::from_document(&doc1).unwrap();

    let doc2 = TextDocument::new();
    let cursor = doc2.cursor();
    cursor.insert_fragment(&frag).unwrap();
    let text = doc2.to_plain_text().unwrap();
    assert!(text.contains("Source text"));
}

#[test]
fn insert_fragment_at_position() {
    let doc = new_doc_with_text("Hello world");
    let frag = DocumentFragment::from_plain_text("beautiful ");
    let cursor = doc.cursor_at(6);
    cursor.insert_fragment(&frag).unwrap();
    let text = doc.to_plain_text().unwrap();
    assert!(text.contains("Hello"));
    assert!(text.contains("beautiful"));
    assert!(text.contains("world"));
}

#[test]
fn insert_plain_text_fragment() {
    let doc = TextDocument::new();
    let frag = DocumentFragment::from_plain_text("Hello");
    let cursor = doc.cursor();
    cursor.insert_fragment(&frag).unwrap();
    let text = doc.to_plain_text().unwrap();
    assert!(text.contains("Hello"));
}

#[test]
fn insert_multiline_plain_text_fragment() {
    let doc = TextDocument::new();
    let frag = DocumentFragment::from_plain_text("Line 1\nLine 2\nLine 3");
    let cursor = doc.cursor();
    cursor.insert_fragment(&frag).unwrap();
    let text = doc.to_plain_text().unwrap();
    assert!(text.contains("Line 1"));
    assert!(text.contains("Line 2"));
    assert!(text.contains("Line 3"));
}

#[test]
fn insert_empty_fragment() {
    let doc = new_doc_with_text("Hello");
    let frag = DocumentFragment::from_plain_text("");
    let cursor = doc.cursor();
    cursor.insert_fragment(&frag).unwrap();
    let text = doc.to_plain_text().unwrap();
    assert!(text.contains("Hello"));
}

// ── Round-trip: extract then insert ───────────────────────────────

#[test]
fn fragment_round_trip() {
    let doc1 = new_doc_with_text("The quick brown fox");
    let c1 = doc1.cursor_at(4);
    c1.move_position(MoveOperation::EndOfWord, MoveMode::KeepAnchor, 1);
    let frag = c1.selection();
    assert_eq!(frag.to_plain_text(), "quick");

    let doc2 = new_doc_with_text("The  fox");
    let c2 = doc2.cursor_at(4);
    c2.insert_fragment(&frag).unwrap();
    let text = doc2.to_plain_text().unwrap();
    assert!(text.contains("quick"));
}

// ── Clone ─────────────────────────────────────────────────────────

#[test]
fn fragment_clone() {
    let frag = DocumentFragment::from_plain_text("Clone me");
    let cloned = frag.clone();
    assert_eq!(cloned.to_plain_text(), "Clone me");
    assert!(!cloned.is_empty());
}

// ── Debug ─────────────────────────────────────────────────────────

#[test]
fn fragment_debug() {
    let frag = DocumentFragment::from_plain_text("Test");
    let debug = format!("{:?}", frag);
    assert!(debug.contains("DocumentFragment"));
}

// ── to_html inline-only detection ────────────────────────────────

#[test]
fn to_html_inline_only_no_p_wrapper() {
    // A partial (inline-only) selection should not be wrapped in <p>
    let inline_frag = DocumentFragment::from_html("<b>bold</b>");
    let html = inline_frag.to_html();
    assert!(
        html.contains("<strong>bold</strong>"),
        "Expected inline bold tag, got: {}",
        html
    );
    assert!(
        !html.contains("<p>"),
        "Should not wrap inline-only fragment in <p>, got: {}",
        html
    );

    // Also verify via cursor extraction of a partial selection
    let doc = TextDocument::new();
    doc.set_plain_text("Hello world").unwrap();
    let cursor = doc.cursor_at(0);
    cursor.move_position(MoveOperation::Right, MoveMode::KeepAnchor, 5);
    let sel_frag = cursor.selection();
    assert_eq!(sel_frag.to_plain_text(), "Hello");
    let sel_html = sel_frag.to_html();
    assert!(
        !sel_html.contains("<p>"),
        "Partial selection should not have <p>, got: {}",
        sel_html
    );
    assert!(
        sel_html.contains("Hello"),
        "Should contain the selected text, got: {}",
        sel_html
    );
}

#[test]
fn to_html_full_plain_block_no_p_wrapper() {
    // A single plain paragraph (no heading, list, etc.) is inline-only
    let doc = new_doc_with_text("Hello world");
    let cursor = doc.cursor();
    cursor.select(SelectionType::Document);
    let frag = cursor.selection();
    let html = frag.to_html();
    // Single plain paragraph has no block-level formatting → no <p> wrapper
    assert!(
        !html.contains("<p>"),
        "Single plain block should not have <p>, got: {}",
        html
    );
    assert!(html.contains("Hello world"));
}

#[test]
fn to_html_multi_block_keeps_p_wrappers() {
    let frag = DocumentFragment::from_plain_text("Line 1\nLine 2");
    let html = frag.to_html();
    // Multi-block should always use <p> wrappers
    assert!(
        html.contains("<p>Line 1</p>"),
        "Expected <p> for first line, got: {}",
        html
    );
    assert!(
        html.contains("<p>Line 2</p>"),
        "Expected <p> for second line, got: {}",
        html
    );
}

#[test]
fn to_html_from_html_inline_fragment() {
    let frag = DocumentFragment::from_html("<b>bold</b>");
    let html = frag.to_html();
    // Single inline block from HTML should not wrap in <p>
    assert!(
        html.contains("<strong>bold</strong>"),
        "Expected bold tag, got: {}",
        html
    );
    assert!(
        !html.contains("<p>"),
        "Should not wrap in <p>, got: {}",
        html
    );
}

// ── insert_html inline merge ─────────────────────────────────────

#[test]
fn cursor_insert_html_merges_inline() {
    let doc = new_doc_with_text("Hello world");
    let cursor = doc.cursor_at(6); // After "Hello " (position 6 = after the space)

    let block_count_before = doc.stats().block_count;

    cursor.insert_html("<b>beautiful</b>").unwrap();

    let text = doc.to_plain_text().unwrap();
    assert!(
        text.contains("Hello beautiful"),
        "Expected merged text, got: {}",
        text
    );

    // Block count should NOT increase for inline content
    assert_eq!(
        doc.stats().block_count,
        block_count_before,
        "Inline HTML insert should not create new blocks"
    );
}

#[test]
fn cursor_insert_html_multi_paragraph_creates_blocks() {
    let doc = new_doc_with_text("Hello world");
    let block_count_before = doc.stats().block_count;

    let cursor = doc.cursor_at(5);
    cursor.insert_html("<p>A</p><p>B</p>").unwrap();

    assert!(
        doc.stats().block_count > block_count_before,
        "Multi-paragraph HTML should create new blocks"
    );
}
