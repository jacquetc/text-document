use text_document::{text_document::TextDocument, text_cursor::MoveMode};

#[test]
fn create_document() {
    let document = TextDocument::new();
    assert_eq!(document.block_count(), 1);
}

#[test]
fn add_text() {
    let mut document = TextDocument::new();
    document.clear();
    document.clear();
    document.set_plain_text("aa\na");

    //let mut cursor = document.create_cursor();
    // cursor.set_position(0, MoveMode::KeepAnchor);
    // cursor.insert_plain_text("\nplain_text");
    
    assert_eq!(document.block_count(), 1);
}



