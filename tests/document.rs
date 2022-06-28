mod common;

use text_document::{text_document::TextDocument, text_cursor::MoveMode, format::BlockFormat};

#[test]
fn create_document() {
    let document = common::setup_text_document();
    document.print_debug_elements();
    assert_eq!(document.block_count(), 1);
}

#[test]
fn add_text() {
    let mut document = TextDocument::new();
    document.set_plain_text("aa\na");
    document.print_debug_elements();

    
    assert_eq!(document.block_count(), 2);
}

