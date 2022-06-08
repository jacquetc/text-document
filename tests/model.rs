use text_document::text_document::TextDocument;

#[test]
fn create_document() {
    let document = TextDocument::new_rc_cell();
    assert_eq!(document.as_ref().block_count, 1);
}