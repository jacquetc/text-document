mod common;

use text_document::TextDocument;

#[test]
fn create_document() {
    let document = common::setup_text_document();
    document.print_debug_elements();
    assert_eq!(document.block_count(), 1);
}

#[test]
fn add_text() {
    let mut document = TextDocument::new();
    document.set_plain_text("aa\na").unwrap();
    document.print_debug_elements();

    assert_eq!(document.block_count(), 2);
}

#[test]
fn set_plain_text_then_export_it() {
    let mut document = TextDocument::new();
    document.set_plain_text("this\nis\na\ntest!\n").unwrap();
    document.print_debug_elements();

    assert_eq!(document.block_count(), 5);

    let plain_text = document.to_plain_text();

    assert_eq!(plain_text, "this\nis\na\ntest!\n".to_string());
}
