use text_document::text_document::TextDocument;

#[test]
fn create_document() {
    let document = TextDocument::new();
    assert_eq!(document.block_count(), 1);
}

#[test]
fn add_block() {
    let document = TextDocument::new();
    
    assert_eq!(document.block_count(), 1);
}


