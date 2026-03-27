use std::sync::Arc;
use std::thread;

use text_document::TextDocument;

#[test]
fn concurrent_inserts_from_multiple_threads() {
    let doc = Arc::new(TextDocument::new());
    doc.set_plain_text("Hello").unwrap();

    let num_threads = 8;
    let chars_per_insert = 2; // "T0".."T7" are each 2 unicode chars

    let mut handles = Vec::new();
    for i in 0..num_threads {
        let doc = Arc::clone(&doc);
        let text = format!("T{}", i);
        handles.push(thread::spawn(move || {
            let cursor = doc.cursor_at(0);
            cursor.insert_text(&text).unwrap();
        }));
    }

    for h in handles {
        h.join().expect("thread should not panic");
    }

    let stats = doc.stats();
    // Each thread inserts 2 chars, original text is 5 chars.
    let expected = 5 + num_threads * chars_per_insert;
    assert_eq!(
        stats.character_count, expected,
        "expected {} characters, got {}",
        expected, stats.character_count
    );
}

#[test]
fn concurrent_mixed_reads_and_writes() {
    let doc = Arc::new(TextDocument::new());
    doc.set_plain_text("Initial content for testing concurrent access")
        .unwrap();

    let num_threads = 8;
    let mut handles = Vec::new();

    for i in 0..num_threads {
        let doc = Arc::clone(&doc);
        if i % 2 == 0 {
            // Writer threads
            handles.push(thread::spawn(move || {
                let cursor = doc.cursor_at(0);
                cursor.insert_text(&format!("W{}", i)).unwrap();
            }));
        } else {
            // Reader threads
            handles.push(thread::spawn(move || {
                let _ = doc.stats();
                let _ = doc.to_plain_text().unwrap();
                let _ = doc.character_count();
            }));
        }
    }

    for h in handles {
        h.join().expect("thread should not panic");
    }

    // Just verify the document is still accessible and consistent.
    let text = doc.to_plain_text().unwrap();
    let stats = doc.stats();
    assert_eq!(stats.character_count, text.chars().count());
}

#[test]
fn concurrent_cursor_creation_and_editing() {
    let doc = Arc::new(TextDocument::new());
    doc.set_plain_text("ABCDEFGHIJ").unwrap(); // 10 chars

    let num_threads = 8;
    let mut handles = Vec::new();

    for i in 0..num_threads {
        let doc = Arc::clone(&doc);
        handles.push(thread::spawn(move || {
            // Each thread creates a cursor and inserts a single char.
            let cursor = doc.cursor_at(0);
            cursor.insert_text(&format!("{}", i)).unwrap();
        }));
    }

    for h in handles {
        h.join().expect("thread should not panic");
    }

    let stats = doc.stats();
    // 10 original + 8 single-char inserts
    assert_eq!(stats.character_count, 10 + num_threads);
}
