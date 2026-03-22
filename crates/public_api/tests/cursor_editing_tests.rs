use text_document::TextDocument;

fn new_doc_with_text(text: &str) -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text(text).unwrap();
    doc
}

#[test]
fn insert_text_at_position_0() {
    let doc = new_doc_with_text("world");
    let cursor = doc.cursor();
    cursor.insert_text("Hello ").unwrap();
    let text = doc.to_plain_text().unwrap();
    assert_eq!(text, "Hello world");
    assert_eq!(cursor.position(), 6);
}

#[test]
fn insert_text_at_end() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor_at(5);
    cursor.insert_text(" world").unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "Hello world");
}

#[test]
fn insert_text_replaces_selection() {
    let doc = new_doc_with_text("Hello world");
    let cursor = doc.cursor();
    cursor.set_position(0, text_document::MoveMode::MoveAnchor);
    cursor.set_position(5, text_document::MoveMode::KeepAnchor);
    assert!(cursor.has_selection());

    cursor.insert_text("Goodbye").unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "Goodbye world");
}

#[test]
fn delete_char_forward() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor();
    cursor.delete_char().unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "ello");
}

#[test]
fn insert_block_creates_new_paragraph() {
    let doc = new_doc_with_text("HelloWorld");
    let cursor = doc.cursor_at(5);
    cursor.insert_block().unwrap();
    assert!(doc.block_count() >= 2);
}

#[test]
fn selected_text() {
    let doc = new_doc_with_text("Hello world");
    let cursor = doc.cursor();
    cursor.set_position(0, text_document::MoveMode::MoveAnchor);
    cursor.set_position(5, text_document::MoveMode::KeepAnchor);
    let text = cursor.selected_text().unwrap();
    assert_eq!(text, "Hello");
}

#[test]
fn cursor_position_tracking() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor();
    assert_eq!(cursor.position(), 0);
    assert_eq!(cursor.anchor(), 0);
    assert!(!cursor.has_selection());
    assert!(cursor.at_start());
}
