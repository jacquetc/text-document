use text_document::{FlowElement, TextDocument};

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
