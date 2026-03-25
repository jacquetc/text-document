use text_document::{FlowElement, FlowElementSnapshot, TextDocument};

fn new_doc_with_text(text: &str) -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text(text).unwrap();
    doc
}

fn first_block_frame(doc: &TextDocument) -> text_document::TextFrame {
    let flow = doc.flow();
    match &flow[0] {
        FlowElement::Block(b) => b.frame(),
        _ => panic!("expected first flow element to be a Block"),
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TextFrame basics
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn frame_id_is_nonzero() {
    let doc = new_doc_with_text("Hello");
    let frame = first_block_frame(&doc);
    assert!(frame.id() > 0, "frame ID should be nonzero");
}

#[test]
fn frame_format_default() {
    let doc = new_doc_with_text("Hello");
    let frame = first_block_frame(&doc);
    let fmt = frame.format();
    assert_eq!(fmt.height, None);
    assert_eq!(fmt.width, None);
    assert_eq!(fmt.border, None);
}

#[test]
fn frame_flow_returns_same_blocks_as_doc_flow() {
    let doc = new_doc_with_text("A\nB\nC");
    let doc_flow = doc.flow();
    let frame = first_block_frame(&doc);
    let frame_flow = frame.flow();
    assert_eq!(
        frame_flow.len(),
        doc_flow.len(),
        "frame flow length should match doc flow length"
    );
}

#[test]
fn frame_flow_block_ids_match() {
    let doc = new_doc_with_text("X\nY");
    let doc_flow = doc.flow();
    let doc_ids: Vec<usize> = doc_flow
        .iter()
        .filter_map(|e| match e {
            FlowElement::Block(b) => Some(b.id()),
            _ => None,
        })
        .collect();

    let frame = first_block_frame(&doc);
    let frame_flow = frame.flow();
    let frame_ids: Vec<usize> = frame_flow
        .iter()
        .filter_map(|e| match e {
            FlowElement::Block(b) => Some(b.id()),
            _ => None,
        })
        .collect();

    assert_eq!(doc_ids, frame_ids);
}

#[test]
fn frame_is_clone() {
    let doc = new_doc_with_text("Hello");
    let frame = first_block_frame(&doc);
    let cloned = frame.clone();
    assert_eq!(frame.id(), cloned.id());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TextFrame::snapshot()
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn frame_snapshot_captures_id_and_elements() {
    let doc = new_doc_with_text("A\nB\nC");
    let frame = first_block_frame(&doc);
    let snap = frame.snapshot();
    assert_eq!(snap.frame_id, frame.id());
    assert_eq!(
        snap.elements.len(),
        3,
        "frame snapshot should have 3 block elements"
    );
    // All elements should be Block snapshots
    for el in &snap.elements {
        assert!(
            matches!(el, FlowElementSnapshot::Block(_)),
            "expected Block snapshot"
        );
    }
}

#[test]
fn frame_snapshot_captures_format() {
    let doc = new_doc_with_text("Hello");
    let frame = first_block_frame(&doc);
    let snap = frame.snapshot();
    // Default format — all None
    assert_eq!(snap.format.height, None);
    assert_eq!(snap.format.width, None);
    assert_eq!(snap.format.border, None);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// FlowElement::snapshot()
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn flow_element_snapshot_block() {
    let doc = new_doc_with_text("Hello");
    let flow = doc.flow();
    let snap = flow[0].snapshot();
    match snap {
        FlowElementSnapshot::Block(b) => {
            assert_eq!(b.text, "Hello");
        }
        _ => panic!("expected Block snapshot"),
    }
}

#[test]
fn flow_element_snapshot_table() {
    let doc = new_doc_with_text("Before");
    let cursor = doc.cursor_at(6);
    cursor.insert_table(2, 2).unwrap();

    let flow = doc.flow();
    let table_elem = flow
        .iter()
        .find(|e| matches!(e, FlowElement::Table(_)))
        .expect("should have a table");
    let snap = table_elem.snapshot();
    match snap {
        FlowElementSnapshot::Table(t) => {
            assert_eq!(t.rows, 2);
            assert_eq!(t.columns, 2);
        }
        _ => panic!("expected Table snapshot"),
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TextFrame with tables
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn frame_flow_includes_table_after_insert() {
    let doc = new_doc_with_text("Before");
    let cursor = doc.cursor_at(6);
    cursor.insert_table(2, 2).unwrap();

    let flow = doc.flow();
    let has_table = flow.iter().any(|e| matches!(e, FlowElement::Table(_)));
    assert!(has_table);

    let frame = first_block_frame(&doc);
    let frame_flow = frame.flow();
    let frame_has_table = frame_flow
        .iter()
        .any(|e| matches!(e, FlowElement::Table(_)));
    assert!(frame_has_table, "frame flow should also contain the table");
}
