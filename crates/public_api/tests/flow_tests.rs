use text_document::{
    DocumentEvent, FlowElement, FlowElementSnapshot, FormatChangeKind, TextDocument,
};

fn new_doc() -> TextDocument {
    TextDocument::new()
}

fn new_doc_with_text(text: &str) -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text(text).unwrap();
    doc
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TextDocument::flow()
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn flow_empty_doc_has_one_block() {
    let doc = new_doc();
    let flow = doc.flow();
    assert_eq!(flow.len(), 1, "new document should have one block in flow");
    assert!(matches!(flow[0], FlowElement::Block(_)));
}

#[test]
fn flow_single_line_has_one_block() {
    let doc = new_doc_with_text("Hello world");
    let flow = doc.flow();
    assert_eq!(flow.len(), 1);
    if let FlowElement::Block(ref b) = flow[0] {
        assert_eq!(b.text(), "Hello world");
    } else {
        panic!("expected FlowElement::Block");
    }
}

#[test]
fn flow_multiline_has_multiple_blocks() {
    let doc = new_doc_with_text("Line one\nLine two\nLine three");
    let flow = doc.flow();
    assert_eq!(flow.len(), 3);
    for elem in &flow {
        assert!(matches!(elem, FlowElement::Block(_)));
    }
}

#[test]
fn flow_block_text_matches_lines() {
    let doc = new_doc_with_text("Alpha\nBeta\nGamma");
    let flow = doc.flow();
    let texts: Vec<String> = flow
        .iter()
        .filter_map(|e| match e {
            FlowElement::Block(b) => Some(b.text()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["Alpha", "Beta", "Gamma"]);
}

#[test]
fn flow_order_stable_after_structural_edit() {
    let doc = new_doc_with_text("First\nSecond");
    // Append a third block via set_plain_text then verify order
    doc.set_plain_text("First\nSecond\nThird").unwrap();

    let flow = doc.flow();
    let texts: Vec<String> = flow
        .iter()
        .filter_map(|e| match e {
            FlowElement::Block(b) => Some(b.text()),
            _ => None,
        })
        .collect();
    assert_eq!(texts.len(), 3);
    assert_eq!(texts[0], "First");
    assert_eq!(texts[1], "Second");
    assert_eq!(texts[2], "Third");
}

#[test]
fn flow_with_table_returns_table_element() {
    let doc = new_doc_with_text("Before");
    let cursor = doc.cursor_at(6);
    cursor.insert_table(2, 2).unwrap();
    let flow = doc.flow();
    let has_table = flow.iter().any(|e| matches!(e, FlowElement::Table(_)));
    assert!(
        has_table,
        "flow should contain a Table element after insert_table"
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TextDocument::block_by_id()
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn block_by_id_returns_valid_block() {
    let doc = new_doc_with_text("Hello");
    let flow = doc.flow();
    let block_id = match &flow[0] {
        FlowElement::Block(b) => b.id(),
        _ => panic!("expected Block"),
    };

    let block = doc.block_by_id(block_id);
    assert!(block.is_some());
    assert_eq!(block.unwrap().text(), "Hello");
}

#[test]
fn block_by_id_returns_none_for_invalid() {
    let doc = new_doc_with_text("Hello");
    assert!(doc.block_by_id(999999).is_none());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TextDocument::block_at_position()
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn block_at_position_first_block() {
    let doc = new_doc_with_text("Hello\nWorld");
    let block = doc.block_at_position(0).unwrap();
    assert_eq!(block.text(), "Hello");
}

#[test]
fn block_at_position_second_block() {
    let doc = new_doc_with_text("Hello\nWorld");
    let block = doc.block_at_position(6).unwrap();
    assert_eq!(block.text(), "World");
}

#[test]
fn block_at_position_middle_of_block() {
    let doc = new_doc_with_text("Hello\nWorld");
    // Position 3 is inside "Hello"
    let block = doc.block_at_position(3).unwrap();
    assert_eq!(block.text(), "Hello");
    // Position 8 is inside "World"
    let block2 = doc.block_at_position(8).unwrap();
    assert_eq!(block2.text(), "World");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TextDocument::block_by_number()
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn block_by_number_zero() {
    let doc = new_doc_with_text("First\nSecond");
    let block = doc.block_by_number(0).unwrap();
    assert_eq!(block.text(), "First");
}

#[test]
fn block_by_number_one() {
    let doc = new_doc_with_text("First\nSecond");
    let block = doc.block_by_number(1).unwrap();
    assert_eq!(block.text(), "Second");
}

#[test]
fn block_by_number_out_of_range() {
    let doc = new_doc_with_text("Hello");
    assert!(doc.block_by_number(5).is_none());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TextDocument::snapshot_flow()
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn snapshot_flow_captures_all_blocks() {
    let doc = new_doc_with_text("A\nB\nC");
    let snap = doc.snapshot_flow();
    assert_eq!(snap.elements.len(), 3);
    for elem in &snap.elements {
        assert!(matches!(elem, FlowElementSnapshot::Block(_)));
    }
}

#[test]
fn snapshot_flow_block_text_matches() {
    let doc = new_doc_with_text("Hello\nWorld");
    let snap = doc.snapshot_flow();
    if let FlowElementSnapshot::Block(ref bs) = snap.elements[0] {
        assert_eq!(bs.text, "Hello");
        assert_eq!(bs.block_id, doc.block_by_number(0).unwrap().id());
    } else {
        panic!("expected Block snapshot");
    }
    if let FlowElementSnapshot::Block(ref bs) = snap.elements[1] {
        assert_eq!(bs.text, "World");
    } else {
        panic!("expected Block snapshot");
    }
}

#[test]
fn snapshot_flow_position_and_length() {
    let doc = new_doc_with_text("Hello\nWorld");
    let snap = doc.snapshot_flow();
    if let FlowElementSnapshot::Block(ref bs) = snap.elements[0] {
        assert_eq!(bs.position, 0);
        assert_eq!(bs.length, 5);
    } else {
        panic!("expected Block");
    }
    if let FlowElementSnapshot::Block(ref bs) = snap.elements[1] {
        assert_eq!(bs.position, 6);
        assert_eq!(bs.length, 5);
    } else {
        panic!("expected Block");
    }
}

#[test]
fn snapshot_flow_consistent_with_flow() {
    let doc = new_doc_with_text("One\nTwo\nThree");
    let flow = doc.flow();
    let snap = doc.snapshot_flow();
    assert_eq!(flow.len(), snap.elements.len());
    for (fe, fes) in flow.iter().zip(snap.elements.iter()) {
        match (fe, fes) {
            (FlowElement::Block(b), FlowElementSnapshot::Block(bs)) => {
                assert_eq!(b.id(), bs.block_id);
                assert_eq!(b.text(), bs.text);
            }
            _ => panic!("flow and snapshot should have matching element types"),
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// FormatChangeKind on events
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn format_changed_char_kind() {
    let doc = new_doc_with_text("Hello world");
    doc.poll_events(); // drain setup events

    let cursor = doc.cursor_at(0);
    cursor.set_position(5, text_document::MoveMode::KeepAnchor);

    let fmt = text_document::TextFormat {
        font_bold: Some(true),
        ..Default::default()
    };
    cursor.set_char_format(&fmt).unwrap();

    let events = doc.poll_events();
    let has_char_format = events.iter().any(|e| {
        matches!(
            e,
            DocumentEvent::FormatChanged {
                kind: FormatChangeKind::Character,
                ..
            }
        )
    });
    assert!(
        has_char_format,
        "expected FormatChanged with Character kind, got: {:?}",
        events
    );
}

#[test]
fn format_changed_block_kind() {
    let doc = new_doc_with_text("Hello world");
    doc.poll_events();

    let cursor = doc.cursor_at(0);
    let fmt = text_document::BlockFormat {
        alignment: Some(text_document::Alignment::Center),
        ..Default::default()
    };
    cursor.set_block_format(&fmt).unwrap();

    let events = doc.poll_events();
    let has_block_format = events.iter().any(|e| {
        matches!(
            e,
            DocumentEvent::FormatChanged {
                kind: FormatChangeKind::Block,
                ..
            }
        )
    });
    assert!(
        has_block_format,
        "expected FormatChanged with Block kind, got: {:?}",
        events
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// FlowElementsInserted / FlowElementsRemoved events
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn flow_elements_inserted_on_insert_block() {
    let doc = new_doc_with_text("Hello");
    doc.poll_events(); // drain

    let cursor = doc.cursor_at(5);
    cursor.insert_block().unwrap();
    cursor.insert_text("World").unwrap();

    let events = doc.poll_events();
    let has_inserted = events
        .iter()
        .any(|e| matches!(e, DocumentEvent::FlowElementsInserted { count: 1, .. }));
    assert!(
        has_inserted,
        "expected FlowElementsInserted event, got: {:?}",
        events
    );
}

#[test]
fn flow_elements_removed_on_delete_block() {
    let doc = new_doc_with_text("First\nSecond");
    doc.poll_events(); // drain

    // Delete the newline to merge blocks
    let cursor = doc.cursor_at(5);
    cursor.delete_char().unwrap();

    let events = doc.poll_events();
    let has_removed = events
        .iter()
        .any(|e| matches!(e, DocumentEvent::FlowElementsRemoved { .. }));
    assert!(
        has_removed,
        "expected FlowElementsRemoved event after merging blocks, got: {:?}",
        events
    );
}

#[test]
fn no_flow_events_on_text_only_edit() {
    let doc = new_doc_with_text("Hello");
    doc.poll_events(); // drain

    let cursor = doc.cursor_at(5);
    cursor.insert_text(" world").unwrap();

    let events = doc.poll_events();
    let has_flow = events.iter().any(|e| {
        matches!(
            e,
            DocumentEvent::FlowElementsInserted { .. } | DocumentEvent::FlowElementsRemoved { .. }
        )
    });
    assert!(
        !has_flow,
        "text-only edits should NOT produce flow events, got: {:?}",
        events
    );
}

#[test]
fn flow_cache_reset_on_document_reset() {
    let doc = new_doc_with_text("First\nSecond\nThird");
    doc.poll_events();

    doc.set_plain_text("New content").unwrap();

    let events = doc.poll_events();
    // After DocumentReset, no flow insert/remove events should be emitted
    // (the layout engine does a full rebuild)
    let has_flow = events.iter().any(|e| {
        matches!(
            e,
            DocumentEvent::FlowElementsInserted { .. } | DocumentEvent::FlowElementsRemoved { .. }
        )
    });
    assert!(
        !has_flow,
        "DocumentReset should not emit flow events, got: {:?}",
        events
    );

    // After reset, flow should reflect new content
    let flow = doc.flow();
    assert_eq!(flow.len(), 1);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TextDocument::blocks()
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn blocks_returns_all_blocks_sorted() {
    let doc = new_doc_with_text("A\nB\nC");
    let blocks = doc.blocks();
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0].text(), "A");
    assert_eq!(blocks[1].text(), "B");
    assert_eq!(blocks[2].text(), "C");
}

#[test]
fn blocks_includes_table_cell_blocks() {
    let doc = new_doc_with_text("Before");
    let cursor = doc.cursor_at(6);
    cursor.insert_table(2, 2).unwrap();

    let blocks = doc.blocks();
    // Should have more than just "Before" — table cells have blocks too
    assert!(
        blocks.len() > 1,
        "blocks() should include table cell blocks, got {} blocks",
        blocks.len()
    );
}

#[test]
fn blocks_empty_doc() {
    let doc = new_doc();
    let blocks = doc.blocks();
    assert_eq!(blocks.len(), 1, "empty doc should have one block");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TextDocument::blocks_in_range()
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn blocks_in_range_single_block() {
    let doc = new_doc_with_text("Hello world");
    let blocks = doc.blocks_in_range(0, 5);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].text(), "Hello world");
}

#[test]
fn blocks_in_range_multiple_blocks() {
    let doc = new_doc_with_text("AAA\nBBB\nCCC");
    // "AAA" is at position 0, length 3
    // "BBB" is at position 4, length 3
    // "CCC" is at position 8, length 3
    // Range [0, 8) should intersect "AAA" and "BBB"
    let blocks = doc.blocks_in_range(0, 8);
    assert!(
        blocks.len() >= 2,
        "range [0..8) should intersect at least 2 blocks, got {}",
        blocks.len()
    );
}

#[test]
fn blocks_in_range_point_query() {
    let doc = new_doc_with_text("AAA\nBBB\nCCC");
    // Point query at position 5 (inside "BBB")
    let blocks = doc.blocks_in_range(5, 0);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].text(), "BBB");
}

#[test]
fn blocks_in_range_out_of_bounds() {
    let doc = new_doc_with_text("Hello");
    let blocks = doc.blocks_in_range(100, 10);
    assert!(
        blocks.is_empty(),
        "should return empty for out-of-bounds range"
    );
}

#[test]
fn snapshot_flow_position_after_table_does_not_overlap() {
    // Regression: snapshot_from_child_order must advance running_pos past table
    // content so that the block after the table gets a correct position.
    let doc = new_doc();
    doc.set_markdown("Before\n\n| A | B |\n|---|---|\n| c | d |\n\nAfter")
        .unwrap()
        .wait()
        .unwrap();

    let snap = doc.snapshot_flow();

    // Find positions of the "Before" block, the table, and the "After" block
    let mut table_max_pos = 0;
    let mut after_pos = None;

    for el in &snap.elements {
        match el {
            FlowElementSnapshot::Table(ts) => {
                for cell in &ts.cells {
                    for block in &cell.blocks {
                        let end = block.position + block.length + 1;
                        if end > table_max_pos {
                            table_max_pos = end;
                        }
                    }
                }
            }
            FlowElementSnapshot::Block(bs) if bs.text == "After" => {
                after_pos = Some(bs.position);
            }
            _ => {}
        }
    }

    let after_pos = after_pos.expect("should find 'After' block in snapshot");

    assert!(
        table_max_pos > 0,
        "table should have cell blocks with positions"
    );
    assert!(
        after_pos >= table_max_pos,
        "'After' block position ({after_pos}) must not overlap table content (max end {table_max_pos})"
    );
}

#[test]
fn snapshot_table_cell_positions_correct_after_edit() {
    // Regression: table cell block positions must be computed from running_pos,
    // not from stale document_position in the DB, so they stay consistent
    // with insert_text's find_block_at_position_sequential after edits.
    let doc = new_doc();
    doc.set_markdown("Before\n\n| A | B |\n|---|---|\n| c | d |\n\nAfter")
        .unwrap()
        .wait()
        .unwrap();

    // Snapshot before edit: collect all cell block positions
    let snap_before = doc.snapshot_flow();
    let cell_positions_before: Vec<(usize, usize, usize)> = snap_before
        .elements
        .iter()
        .filter_map(|el| {
            if let FlowElementSnapshot::Table(ts) = el {
                Some(ts)
            } else {
                None
            }
        })
        .flat_map(|ts| &ts.cells)
        .flat_map(|cell| {
            cell.blocks
                .iter()
                .map(|b| (b.position, b.length, b.block_id))
        })
        .collect();
    assert!(!cell_positions_before.is_empty());

    // Type into the first cell (cell 0,0)
    let first_cell_pos = cell_positions_before[0].0;
    let first_cell_len = cell_positions_before[0].1;
    let cursor = doc.cursor_at(first_cell_pos + first_cell_len); // end of first cell text
    cursor.insert_text("X").unwrap();

    // Snapshot after edit
    let snap_after = doc.snapshot_flow();

    // Collect all block positions (flow blocks + cell blocks) and verify no overlaps
    let mut all_positions: Vec<(usize, usize, String)> = Vec::new();
    fn collect_positions(elements: &[FlowElementSnapshot], out: &mut Vec<(usize, usize, String)>) {
        for el in elements {
            match el {
                FlowElementSnapshot::Block(bs) => {
                    out.push((bs.position, bs.length, bs.text.clone()));
                }
                FlowElementSnapshot::Table(ts) => {
                    for cell in &ts.cells {
                        for block in &cell.blocks {
                            out.push((block.position, block.length, block.text.clone()));
                        }
                    }
                }
                FlowElementSnapshot::Frame(fs) => {
                    collect_positions(&fs.elements, out);
                }
            }
        }
    }
    collect_positions(&snap_after.elements, &mut all_positions);
    all_positions.sort_by_key(|(pos, _, _)| *pos);

    // Verify no two blocks overlap and positions are monotonically increasing
    for i in 1..all_positions.len() {
        let (prev_pos, prev_len, ref prev_text) = all_positions[i - 1];
        let (cur_pos, _, ref cur_text) = all_positions[i];
        let prev_end = prev_pos + prev_len + 1; // +1 for block separator
        assert!(
            cur_pos >= prev_end,
            "Block {:?} at pos {} (end {}) overlaps with block {:?} at pos {}",
            prev_text,
            prev_pos,
            prev_end,
            cur_text,
            cur_pos
        );
    }
}
