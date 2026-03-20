use text_document::TextDocument;

fn new_doc_with_text(text: &str) -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text(text).unwrap();
    doc
}

#[test]
fn undo_insert_text() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor_at(5);
    cursor.insert_text(" world").unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "Hello world");

    assert!(doc.can_undo());
    doc.undo().unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "Hello");
}

#[test]
fn redo_after_undo() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor_at(5);
    cursor.insert_text(" world").unwrap();

    doc.undo().unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "Hello");

    assert!(doc.can_redo());
    doc.redo().unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "Hello world");
}

#[test]
fn no_undo_after_set_plain_text() {
    let doc = new_doc_with_text("Hello");
    // set_plain_text clears undo history
    assert!(!doc.can_undo());
}

#[test]
fn undo_delete() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor_at(5);
    cursor.delete_previous_char().unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "Hell");

    doc.undo().unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "Hello");
}

#[test]
fn clear_undo_redo() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor_at(5);
    cursor.insert_text(" world").unwrap();
    assert!(doc.can_undo());

    doc.clear_undo_redo();
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());
}

#[test]
fn edit_block_groups_operations() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor_at(5);

    cursor.begin_edit_block();
    cursor.insert_text(" ").unwrap();
    cursor.insert_text("world").unwrap();
    cursor.end_edit_block();

    assert_eq!(doc.to_plain_text().unwrap(), "Hello world");

    // One undo should reverse both inserts
    doc.undo().unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "Hello");
}
