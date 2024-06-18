#![cfg(test)]
mod common;
use common::setup_text_document;

#[test]
fn set_plain_text() {
    let mut text_document = setup_text_document();

    text_document.set_plain_text("text");
    assert_eq!(text_document.get_plain_text(), "text");

    text_document.set_plain_text("");
    assert_eq!(text_document.get_plain_text(), "");

    text_document.set_plain_text("line1\nline2");
    assert_eq!(text_document.get_plain_text(), "line1\nline2");

    text_document.set_plain_text("line1\nline2\n");
    assert_eq!(text_document.get_plain_text(), "line1\nline2\n");
}
