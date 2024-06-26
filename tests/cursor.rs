#![cfg(test)]
mod common;
use common::setup_text_document;
use text_document::text_cursor::{TextCursor, TextCursorMut};

#[test]
fn cursor_construction() {
    let mut text_document = setup_text_document();

    let cursor_mut = TextCursorMut::new(& mut text_document);
    assert_eq!(cursor_mut.anchor(), 0);

    let cursor  = TextCursor::new(& text_document);
    assert_eq!(cursor.anchor(), 0);

}
