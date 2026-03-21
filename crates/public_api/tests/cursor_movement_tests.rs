use text_document::{MoveMode, MoveOperation, SelectionType, TextDocument};

fn new_doc_with_text(text: &str) -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text(text).unwrap();
    doc
}

#[test]
fn move_to_start() {
    let doc = new_doc_with_text("Hello world");
    let cursor = doc.cursor_at(5);
    cursor.move_position(MoveOperation::Start, MoveMode::MoveAnchor, 1);
    assert_eq!(cursor.position(), 0);
}

#[test]
fn move_to_end() {
    let doc = new_doc_with_text("Hello world");
    let cursor = doc.cursor();
    cursor.move_position(MoveOperation::End, MoveMode::MoveAnchor, 1);
    assert_eq!(cursor.position(), 11);
}

#[test]
fn move_next_character() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor();
    cursor.move_position(MoveOperation::NextCharacter, MoveMode::MoveAnchor, 1);
    assert_eq!(cursor.position(), 1);
}

#[test]
fn move_previous_character() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor_at(3);
    cursor.move_position(MoveOperation::PreviousCharacter, MoveMode::MoveAnchor, 1);
    assert_eq!(cursor.position(), 2);
}

#[test]
fn move_with_keep_anchor_creates_selection() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor();
    cursor.move_position(MoveOperation::NextCharacter, MoveMode::KeepAnchor, 3);
    assert!(cursor.has_selection());
    assert_eq!(cursor.anchor(), 0);
    assert_eq!(cursor.position(), 3);
    assert_eq!(cursor.selected_text().unwrap(), "Hel");
}

#[test]
fn select_document() {
    let doc = new_doc_with_text("Hello world");
    let cursor = doc.cursor();
    cursor.select(SelectionType::Document);
    assert_eq!(cursor.selection_start(), 0);
    assert_eq!(cursor.selection_end(), 11);
    assert_eq!(cursor.selected_text().unwrap(), "Hello world");
}

#[test]
fn select_block_under_cursor() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor_at(3);
    cursor.select(SelectionType::BlockUnderCursor);
    assert_eq!(cursor.selection_start(), 0);
    assert_eq!(cursor.selection_end(), 5);
}

#[test]
fn set_position_clamps_to_document_end() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor();
    cursor.set_position(999, MoveMode::MoveAnchor);
    assert_eq!(cursor.position(), 5);
}

#[test]
fn at_block_start_and_end() {
    let doc = new_doc_with_text("Hello");
    let c_start = doc.cursor_at(0);
    assert!(c_start.at_block_start());
    let c_end = doc.cursor_at(5);
    assert!(c_end.at_block_end());
}

#[test]
fn block_number_and_position_in_block() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor_at(3);
    assert_eq!(cursor.block_number(), 0);
    assert_eq!(cursor.position_in_block(), 3);
}
