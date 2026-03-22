//! Tests for bugs fixed in the review pass:
//! - remove_selected_text emits ContentsChanged event
//! - insert_frame emits ContentsChanged event
//! - create_list / insert_list emit ContentsChanged events
//! - poll_events and on_change are independent delivery paths
//! - plain text cache invalidation
//! - Operation::wait_timeout
//! - join_previous_edit_block behaves as begin_edit_block

use std::sync::{Arc, Mutex};
use text_document::{DocumentEvent, ListStyle, MoveMode, MoveOperation, TextDocument};

fn new_doc(text: &str) -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text(text).unwrap();
    doc.poll_events(); // drain setup events
    doc
}

// ── remove_selected_text now emits events ───────────────────────

#[test]
fn remove_selected_text_emits_contents_changed() {
    let doc = new_doc("Hello world");
    let cursor = doc.cursor();
    cursor.move_position(MoveOperation::NextCharacter, MoveMode::KeepAnchor, 5);

    let deleted = cursor.remove_selected_text().unwrap();
    assert_eq!(deleted, "Hello");

    let events = doc.poll_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DocumentEvent::ContentsChanged { .. })),
        "expected ContentsChanged from remove_selected_text, got: {:?}",
        events
    );
}

#[test]
fn remove_selected_text_fires_callback() {
    let doc = new_doc("Hello world");
    let received = Arc::new(Mutex::new(Vec::new()));
    let r = received.clone();
    let _sub = doc.on_change(move |e| r.lock().unwrap().push(e));

    let cursor = doc.cursor();
    cursor.move_position(MoveOperation::NextCharacter, MoveMode::KeepAnchor, 5);
    cursor.remove_selected_text().unwrap();

    let events = received.lock().unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DocumentEvent::ContentsChanged { .. })),
        "callback should have received ContentsChanged from remove_selected_text, got: {:?}",
        *events
    );
}

// ── insert_frame now emits events ───────────────────────────────

#[test]
fn insert_frame_emits_contents_changed() {
    let doc = new_doc("Hello");
    let cursor = doc.cursor_at(5);
    cursor.insert_frame().unwrap();

    let events = doc.poll_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DocumentEvent::ContentsChanged { .. })),
        "expected ContentsChanged from insert_frame, got: {:?}",
        events
    );
}

// ── create_list / insert_list emit events ───────────────────────

#[test]
fn create_list_emits_contents_changed() {
    let doc = new_doc("Hello");
    let cursor = doc.cursor();
    cursor.create_list(ListStyle::Disc).unwrap();

    let events = doc.poll_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DocumentEvent::ContentsChanged { .. })),
        "expected ContentsChanged from create_list, got: {:?}",
        events
    );
}

#[test]
fn insert_list_emits_contents_changed() {
    let doc = new_doc("Hello");
    let cursor = doc.cursor_at(5);
    cursor.insert_list(ListStyle::Decimal).unwrap();

    let events = doc.poll_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DocumentEvent::ContentsChanged { .. })),
        "expected ContentsChanged from insert_list, got: {:?}",
        events
    );
}

// ── poll_events and on_change are independent (ISSUE-21) ────────

#[test]
fn poll_and_callback_both_receive_events() {
    let doc = new_doc("Hello");
    let received = Arc::new(Mutex::new(Vec::new()));
    let r = received.clone();
    let _sub = doc.on_change(move |e| r.lock().unwrap().push(e));

    let cursor = doc.cursor_at(5);
    cursor.insert_text(" world").unwrap();

    // Callback should have received the event
    let cb_events = received.lock().unwrap();
    assert!(
        cb_events
            .iter()
            .any(|e| matches!(e, DocumentEvent::ContentsChanged { .. })),
        "callback should have received ContentsChanged"
    );
    drop(cb_events);

    // poll_events should ALSO receive the same event (independent path)
    let poll_events = doc.poll_events();
    assert!(
        poll_events
            .iter()
            .any(|e| matches!(e, DocumentEvent::ContentsChanged { .. })),
        "poll_events should also receive ContentsChanged"
    );
}

#[test]
fn poll_events_drains_independently_from_callbacks() {
    let doc = new_doc("Hello");
    let _sub = doc.on_change(|_| {}); // callback that does nothing

    let cursor = doc.cursor_at(5);
    cursor.insert_text(" world").unwrap();

    // First poll drains
    let events1 = doc.poll_events();
    assert!(!events1.is_empty());

    // Second poll is empty
    let events2 = doc.poll_events();
    assert!(events2.is_empty());

    // New edit produces new events for both paths
    cursor.insert_text("!").unwrap();
    let events3 = doc.poll_events();
    assert!(!events3.is_empty());
}

// ── plain text cache ────────────────────────────────────────────

#[test]
fn plain_text_cache_invalidated_on_edit() {
    let doc = new_doc("Hello");
    assert_eq!(doc.to_plain_text().unwrap(), "Hello");

    let cursor = doc.cursor_at(5);
    cursor.insert_text(" world").unwrap();
    // Cache must be invalidated — should return updated text
    assert_eq!(doc.to_plain_text().unwrap(), "Hello world");
}

#[test]
fn plain_text_cache_invalidated_on_undo() {
    let doc = new_doc("Hello");
    let cursor = doc.cursor_at(5);
    cursor.insert_text(" world").unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "Hello world");

    doc.undo().unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "Hello");
}

#[test]
fn plain_text_cache_invalidated_on_clear() {
    let doc = new_doc("Hello");
    assert_eq!(doc.to_plain_text().unwrap(), "Hello");

    doc.clear().unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "");
}

// ── Operation::wait_timeout ─────────────────────────────────────

#[test]
fn wait_timeout_returns_result_for_fast_operation() {
    let doc = new_doc("Hello world");
    let op = doc.set_markdown("# Heading\n\nParagraph").unwrap();

    let result = op.wait_timeout(std::time::Duration::from_secs(5));
    assert!(result.is_some(), "operation should complete within timeout");
    let import_result = result.unwrap().unwrap();
    assert!(import_result.block_count >= 2);
}

// ── join_previous_edit_block is an alias ────────────────────────

#[test]
fn join_previous_edit_block_groups_with_begin() {
    let doc = new_doc("Hello");
    let cursor = doc.cursor_at(5);

    cursor.join_previous_edit_block();
    cursor.insert_text(" ").unwrap();
    cursor.insert_text("world").unwrap();
    cursor.end_edit_block();

    assert_eq!(doc.to_plain_text().unwrap(), "Hello world");

    // One undo reverses the entire group
    doc.undo().unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "Hello");
}

// ── Error paths ─────────────────────────────────────────────────

#[test]
fn remove_selected_text_no_selection_returns_empty() {
    let doc = new_doc("Hello");
    let cursor = doc.cursor_at(3);
    let result = cursor.remove_selected_text().unwrap();
    assert_eq!(result, "");
}

#[test]
fn delete_previous_char_at_start_is_noop() {
    let doc = new_doc("Hello");
    let cursor = doc.cursor();
    // Should not error at position 0
    cursor.delete_previous_char().unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "Hello");
}
