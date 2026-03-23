use text_document::{ListStyle, MoveMode, TextDocument};

fn new_doc_with_list() -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text("Alpha\nBeta\nGamma").unwrap();
    let cursor = doc.cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.set_position(16, MoveMode::KeepAnchor); // select all
    cursor.create_list(ListStyle::Decimal).unwrap();
    doc
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TextList basics
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn list_id_is_nonzero() {
    let doc = new_doc_with_list();
    let block = doc.block_by_number(0).unwrap();
    let list = block.list().unwrap();
    assert!(list.id() > 0);
}

#[test]
fn list_style_matches() {
    let doc = new_doc_with_list();
    let block = doc.block_by_number(0).unwrap();
    let list = block.list().unwrap();
    assert_eq!(list.style(), ListStyle::Decimal);
}

#[test]
fn list_count() {
    let doc = new_doc_with_list();
    let block = doc.block_by_number(0).unwrap();
    let list = block.list().unwrap();
    assert_eq!(list.count(), 3);
}

#[test]
fn list_item_returns_correct_block() {
    let doc = new_doc_with_list();
    let block = doc.block_by_number(0).unwrap();
    let list = block.list().unwrap();

    let item0 = list.item(0).unwrap();
    assert_eq!(item0.text(), "Alpha");

    let item1 = list.item(1).unwrap();
    assert_eq!(item1.text(), "Beta");

    let item2 = list.item(2).unwrap();
    assert_eq!(item2.text(), "Gamma");
}

#[test]
fn list_item_out_of_range() {
    let doc = new_doc_with_list();
    let block = doc.block_by_number(0).unwrap();
    let list = block.list().unwrap();
    assert!(list.item(10).is_none());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// item_marker()
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn item_marker_decimal() {
    let doc = new_doc_with_list();
    let block = doc.block_by_number(0).unwrap();
    let list = block.list().unwrap();

    let m0 = list.item_marker(0);
    assert!(
        m0.contains('1'),
        "first decimal marker should contain '1', got: {m0}"
    );

    let m1 = list.item_marker(1);
    assert!(
        m1.contains('2'),
        "second decimal marker should contain '2', got: {m1}"
    );

    let m2 = list.item_marker(2);
    assert!(
        m2.contains('3'),
        "third decimal marker should contain '3', got: {m2}"
    );
}

#[test]
fn item_marker_disc() {
    let doc = TextDocument::new();
    doc.set_plain_text("A\nB").unwrap();
    let cursor = doc.cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.set_position(3, MoveMode::KeepAnchor);
    cursor.create_list(ListStyle::Disc).unwrap();

    let block = doc.block_by_number(0).unwrap();
    let list = block.list().unwrap();
    let m = list.item_marker(0);
    assert!(
        m.contains('\u{2022}'),
        "disc marker should contain bullet, got: {m}"
    );
}

#[test]
fn item_marker_lower_alpha() {
    let doc = TextDocument::new();
    doc.set_plain_text("X\nY\nZ").unwrap();
    let cursor = doc.cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.set_position(5, MoveMode::KeepAnchor);
    cursor.create_list(ListStyle::LowerAlpha).unwrap();

    let block = doc.block_by_number(0).unwrap();
    let list = block.list().unwrap();

    assert!(list.item_marker(0).contains('a'));
    assert!(list.item_marker(1).contains('b'));
    assert!(list.item_marker(2).contains('c'));
}

#[test]
fn item_marker_upper_roman() {
    let doc = TextDocument::new();
    doc.set_plain_text("X\nY\nZ\nW").unwrap();
    let cursor = doc.cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.set_position(7, MoveMode::KeepAnchor);
    cursor.create_list(ListStyle::UpperRoman).unwrap();

    let block = doc.block_by_number(0).unwrap();
    let list = block.list().unwrap();

    assert!(
        list.item_marker(0).contains('I'),
        "got: {}",
        list.item_marker(0)
    );
    assert!(
        list.item_marker(1).contains("II"),
        "got: {}",
        list.item_marker(1)
    );
    assert!(
        list.item_marker(2).contains("III"),
        "got: {}",
        list.item_marker(2)
    );
    assert!(
        list.item_marker(3).contains("IV"),
        "got: {}",
        list.item_marker(3)
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// prefix / suffix
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn list_prefix_and_suffix() {
    let doc = new_doc_with_list();
    let block = doc.block_by_number(0).unwrap();
    let list = block.list().unwrap();
    // prefix and suffix may be empty for default lists
    let _prefix = list.prefix();
    let _suffix = list.suffix();
    // just ensure they don't panic
}

#[test]
fn list_indent() {
    let doc = new_doc_with_list();
    let block = doc.block_by_number(0).unwrap();
    let list = block.list().unwrap();
    let _indent = list.indent();
    // just ensure it doesn't panic
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ListInfo in snapshot
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn snapshot_list_info_all_items() {
    let doc = new_doc_with_list();

    for i in 0..3 {
        let block = doc.block_by_number(i).unwrap();
        let snap = block.snapshot();
        assert!(snap.list_info.is_some(), "block {i} should have list_info");
        let info = snap.list_info.unwrap();
        assert_eq!(info.item_index, i);
        assert_eq!(info.style, ListStyle::Decimal);
    }
}

#[test]
fn snapshot_list_info_markers_sequential() {
    let doc = new_doc_with_list();

    let markers: Vec<String> = (0..3)
        .map(|i| {
            doc.block_by_number(i)
                .unwrap()
                .snapshot()
                .list_info
                .unwrap()
                .marker
        })
        .collect();

    assert!(markers[0].contains('1'));
    assert!(markers[1].contains('2'));
    assert!(markers[2].contains('3'));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Clone
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn list_is_clone() {
    let doc = new_doc_with_list();
    let block = doc.block_by_number(0).unwrap();
    let list = block.list().unwrap();
    let cloned = list.clone();
    assert_eq!(list.id(), cloned.id());
    assert_eq!(list.style(), cloned.style());
    assert_eq!(list.count(), cloned.count());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// All blocks in list share the same list handle
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn all_blocks_share_same_list_id() {
    let doc = new_doc_with_list();
    let id0 = doc.block_by_number(0).unwrap().list().unwrap().id();
    let id1 = doc.block_by_number(1).unwrap().list().unwrap().id();
    let id2 = doc.block_by_number(2).unwrap().list().unwrap().id();
    assert_eq!(id0, id1);
    assert_eq!(id1, id2);
}
