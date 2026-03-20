use text_document::{ResourceType, TextDirection, TextDocument, WrapMode};

fn new_doc() -> TextDocument {
    TextDocument::new()
}

fn new_doc_with_text(text: &str) -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text(text).unwrap();
    doc
}

#[test]
fn text_direction_default() {
    let doc = new_doc();
    assert_eq!(doc.text_direction(), TextDirection::LeftToRight);
}

#[test]
fn set_text_direction() {
    let doc = new_doc();
    doc.set_text_direction(TextDirection::RightToLeft).unwrap();
    assert_eq!(doc.text_direction(), TextDirection::RightToLeft);
}

#[test]
fn wrap_mode_default() {
    let doc = new_doc();
    // Default may vary; just check it doesn't panic
    let _mode = doc.default_wrap_mode();
}

#[test]
fn set_wrap_mode() {
    let doc = new_doc();
    doc.set_default_wrap_mode(WrapMode::NoWrap).unwrap();
    assert_eq!(doc.default_wrap_mode(), WrapMode::NoWrap);
}

#[test]
fn add_and_get_resource() {
    let doc = new_doc();
    let data = b"fake image data";
    doc.add_resource(ResourceType::Image, "test.png", "image/png", data)
        .unwrap();

    let retrieved = doc.resource("test.png").unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap(), data);
}

#[test]
fn resource_not_found() {
    let doc = new_doc();
    let result = doc.resource("nonexistent.png").unwrap();
    assert!(result.is_none());
}

#[test]
fn multi_block_operations() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor_at(5);

    // Insert block break
    cursor.insert_block().unwrap();
    assert!(doc.block_count() >= 2);

    // Insert text in second block
    cursor.insert_text("World").unwrap();

    let text = doc.to_plain_text().unwrap();
    assert!(text.contains("Hello"), "should contain Hello: {text:?}");
    assert!(text.contains("World"), "should contain World: {text:?}");
}
