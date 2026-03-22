use text_document::{MoveMode, MoveOperation, TextDocument};

fn new_doc_with_text(text: &str) -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text(text).unwrap();
    doc
}

// Tests that are unique to this file (not covered by cursor_boundary_tests):

#[test]
fn move_next_character_single_step() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor();
    cursor.move_position(MoveOperation::NextCharacter, MoveMode::MoveAnchor, 1);
    assert_eq!(cursor.position(), 1);
}

#[test]
fn move_previous_character_single_step() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor_at(3);
    cursor.move_position(MoveOperation::PreviousCharacter, MoveMode::MoveAnchor, 1);
    assert_eq!(cursor.position(), 2);
}

#[test]
fn select_word_under_cursor_via_movement() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor();
    cursor.move_position(MoveOperation::NextCharacter, MoveMode::KeepAnchor, 3);
    assert!(cursor.has_selection());
    assert_eq!(cursor.anchor(), 0);
    assert_eq!(cursor.position(), 3);
    assert_eq!(cursor.selected_text().unwrap(), "Hel");
}
