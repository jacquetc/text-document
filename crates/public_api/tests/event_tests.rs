use text_document::{DocumentEvent, TextDocument};

fn new_doc_with_text(text: &str) -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text(text).unwrap();
    // Drain any events from setup
    doc.poll_events();
    doc
}

#[test]
fn poll_events_after_insert() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor_at(5);
    cursor.insert_text(" world").unwrap();

    let events = doc.poll_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DocumentEvent::ContentsChanged { .. })),
        "expected ContentsChanged event, got: {:?}",
        events
    );
}

#[test]
fn poll_events_drains() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor_at(5);
    cursor.insert_text(" world").unwrap();

    let events1 = doc.poll_events();
    assert!(!events1.is_empty());

    let events2 = doc.poll_events();
    assert!(events2.is_empty());
}

#[test]
fn poll_events_after_clear() {
    let doc = new_doc_with_text("Hello");
    doc.poll_events(); // drain setup events
    doc.clear().unwrap();

    let events = doc.poll_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DocumentEvent::DocumentReset)),
        "expected DocumentReset event, got: {:?}",
        events
    );
}

#[test]
fn modified_flag() {
    let doc = new_doc_with_text("Hello");
    assert!(!doc.is_modified());

    let cursor = doc.cursor_at(5);
    cursor.insert_text(" world").unwrap();
    assert!(doc.is_modified());

    doc.set_modified(false);
    assert!(!doc.is_modified());
}

#[test]
fn on_change_callback_fires() {
    use std::sync::{Arc, Mutex};

    let doc = new_doc_with_text("Hello");
    let received = Arc::new(Mutex::new(Vec::new()));
    let received_clone = received.clone();

    let _sub = doc.on_change(move |event| {
        received_clone.lock().unwrap().push(event);
    });

    let cursor = doc.cursor_at(5);
    cursor.insert_text(" world").unwrap();

    let events = received.lock().unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DocumentEvent::ContentsChanged { .. })),
        "callback should have received ContentsChanged, got: {:?}",
        *events
    );
}

#[test]
fn subscription_drop_stops_events() {
    use std::sync::{Arc, Mutex};

    let doc = new_doc_with_text("Hello");
    let received = Arc::new(Mutex::new(Vec::new()));
    let received_clone = received.clone();

    let sub = doc.on_change(move |event| {
        received_clone.lock().unwrap().push(event);
    });

    drop(sub);

    let cursor = doc.cursor_at(5);
    cursor.insert_text(" world").unwrap();

    let events = received.lock().unwrap();
    assert!(
        events.is_empty(),
        "no events should be received after dropping subscription, got: {:?}",
        *events
    );
}
