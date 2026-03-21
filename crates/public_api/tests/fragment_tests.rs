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
