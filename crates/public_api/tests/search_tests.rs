use text_document::{FindOptions, TextDocument};

fn new_doc_with_text(text: &str) -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text(text).unwrap();
    doc
}

#[test]
fn find_text_basic() {
    let doc = new_doc_with_text("Hello world Hello");
    let opts = FindOptions::default();
    let result = doc.find("Hello", 0, &opts).unwrap();
    assert!(result.is_some());
    let m = result.unwrap();
    assert_eq!(m.position, 0);
    assert_eq!(m.length, 5);
}

#[test]
fn find_text_from_offset() {
    let doc = new_doc_with_text("Hello world Hello");
    let opts = FindOptions::default();
    let result = doc.find("Hello", 1, &opts).unwrap();
    assert!(result.is_some());
    let m = result.unwrap();
    assert_eq!(m.position, 12);
}

#[test]
fn find_text_not_found() {
    let doc = new_doc_with_text("Hello world");
    let opts = FindOptions::default();
    let result = doc.find("xyz", 0, &opts).unwrap();
    assert!(result.is_none());
}

#[test]
fn find_all() {
    let doc = new_doc_with_text("abcabcabc");
    let opts = FindOptions::default();
    let matches = doc.find_all("abc", &opts).unwrap();
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].position, 0);
    assert_eq!(matches[1].position, 3);
    assert_eq!(matches[2].position, 6);
}

#[test]
fn find_case_sensitive() {
    let doc = new_doc_with_text("Hello hello");
    let opts = FindOptions {
        case_sensitive: true,
        ..Default::default()
    };
    let matches = doc.find_all("Hello", &opts).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].position, 0);
}

#[test]
fn replace_text_all() {
    let doc = new_doc_with_text("foo bar foo");
    let opts = FindOptions::default();
    let count = doc.replace_text("foo", "baz", true, &opts).unwrap();
    assert_eq!(count, 2);
    assert_eq!(doc.to_plain_text().unwrap(), "baz bar baz");
}

#[test]
fn replace_text_is_undoable() {
    let doc = new_doc_with_text("foo bar foo");
    let opts = FindOptions::default();
    doc.replace_text("foo", "baz", true, &opts).unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "baz bar baz");

    doc.undo().unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "foo bar foo");
}
