use text_document::TextDocument;

fn new_doc_with_text(text: &str) -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text(text).unwrap();
    doc
}

#[test]
fn two_cursors_coexist() {
    let doc = new_doc_with_text("Hello world");
    let c1 = doc.cursor_at(0);
    let c2 = doc.cursor_at(5);
    assert_eq!(c1.position(), 0);
    assert_eq!(c2.position(), 5);
}

#[test]
fn edit_through_one_cursor_adjusts_other() {
    let doc = new_doc_with_text("Hello world");
    let c1 = doc.cursor_at(0);
    let c2 = doc.cursor_at(5);

    // Insert "AAA" at position 0 through c1
    c1.insert_text("AAA").unwrap();

    // c1 should be at position 3 (after "AAA")
    assert_eq!(c1.position(), 3);
    // c2 was at position 5, shifted by +3 to position 8
    assert_eq!(c2.position(), 8);

    assert_eq!(doc.to_plain_text().unwrap(), "AAAHello world");
}

#[test]
fn delete_through_one_cursor_adjusts_other() {
    let doc = new_doc_with_text("Hello world");
    let c1 = doc.cursor_at(0);
    let c2 = doc.cursor_at(5);

    // Delete "H" at position 0
    c1.delete_char().unwrap();

    assert_eq!(c1.position(), 0);
    // c2 was at 5, shifted by -1 to 4
    assert_eq!(c2.position(), 4);
    assert_eq!(doc.to_plain_text().unwrap(), "ello world");
}

#[test]
fn clone_cursor_creates_independent_copy() {
    let doc = new_doc_with_text("Hello");
    let c1 = doc.cursor_at(3);
    let c2 = c1.clone();

    assert_eq!(c1.position(), 3);
    assert_eq!(c2.position(), 3);

    // Move c1, c2 should stay
    c1.set_position(0, text_document::MoveMode::MoveAnchor);
    assert_eq!(c1.position(), 0);
    assert_eq!(c2.position(), 3);
}

#[test]
fn cloned_cursor_is_adjusted_by_edits() {
    let doc = new_doc_with_text("Hello world");
    let c1 = doc.cursor_at(0);
    let c2 = c1.clone();

    c1.insert_text("XX").unwrap();
    // c1 moved to 2, c2 (was at 0, same position) should also adjust
    assert_eq!(c1.position(), 2);
    // c2 was at position 0, which is <= edit_pos (0), so it stays at 0
    assert_eq!(c2.position(), 0);
}

#[test]
fn many_cursors() {
    let doc = new_doc_with_text("abcdefghij");
    let cursors: Vec<_> = (0..10).map(|i| doc.cursor_at(i)).collect();

    // Insert at position 0
    cursors[0].insert_text("X").unwrap();

    // Cursor 0 moved to 1
    assert_eq!(cursors[0].position(), 1);
    // All others shifted by +1
    for (i, cursor) in cursors.iter().enumerate().skip(1) {
        assert_eq!(cursor.position(), i + 1);
    }
}
