//! Snapshot tests for text-document outputs.
//!
//! These tests exercise the serialisation and structural outputs that
//! are hard to assert against by hand (multi-line HTML, deeply nested
//! flow snapshots, markdown round-trips). Each test uses `insta` so
//! the assertion is stored as a `.snap` file alongside the test:
//! running `cargo insta review` after an intentional change lets you
//! eyeball the diff and accept it, rather than hand-editing a multi-
//! line `assert_eq!` literal.
//!
//! The goal isn't 100% snapshot coverage — it's to lock in the shape
//! of things the project already serialises (HTML, markdown,
//! fragments, flow snapshots) so regressions are caught by
//! structural diff rather than by humans noticing a wrong angle
//! bracket.

use insta::{assert_debug_snapshot, assert_snapshot};
use text_document::{MoveMode, MoveOperation, TextDocument, TextFormat};

fn new_doc(plain: &str) -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text(plain).unwrap();
    doc
}

// ── HTML serialisation ──────────────────────────────────────────────

#[test]
fn snapshot_html_simple_paragraph() {
    let doc = new_doc("Hello, world!");
    let c = doc.cursor_at(0);
    c.move_position(MoveOperation::End, MoveMode::KeepAnchor, 1);
    let frag = c.selection();
    assert_snapshot!(frag.to_html());
}

#[test]
fn snapshot_html_mixed_formatting() {
    let doc = new_doc("");
    let c = doc.cursor_at(0);
    c.insert_text("Plain ").unwrap();
    let bold = TextFormat {
        font_bold: Some(true),
        ..Default::default()
    };
    c.insert_formatted_text("bold", &bold).unwrap();
    c.insert_text(" and ").unwrap();
    let italic = TextFormat {
        font_italic: Some(true),
        ..Default::default()
    };
    c.insert_formatted_text("italic", &italic).unwrap();
    c.insert_text(" text.").unwrap();

    let c2 = doc.cursor_at(0);
    c2.move_position(MoveOperation::End, MoveMode::KeepAnchor, 1);
    assert_snapshot!(c2.selection().to_html());
}

#[test]
fn snapshot_html_with_table() {
    let doc = TextDocument::new();
    doc.set_markdown("Before\n\n| A | B |\n|---|---|\n| c | d |\n\nAfter")
        .unwrap()
        .wait()
        .unwrap();
    let c = doc.cursor_at(0);
    c.move_position(MoveOperation::End, MoveMode::KeepAnchor, 1);
    assert_snapshot!(c.selection().to_html());
}

#[test]
fn snapshot_html_with_nested_list() {
    let doc = TextDocument::new();
    doc.set_html(concat!(
        "<ul>",
        "<li>outer one<ul><li>inner a</li><li>inner b</li></ul></li>",
        "<li>outer two</li>",
        "</ul>",
    ))
    .unwrap();
    let c = doc.cursor_at(0);
    c.move_position(MoveOperation::End, MoveMode::KeepAnchor, 1);
    assert_snapshot!(c.selection().to_html());
}

// ── Markdown serialisation ──────────────────────────────────────────

#[test]
fn snapshot_markdown_mixed_blocks() {
    let doc = TextDocument::new();
    doc.set_html(concat!(
        "<h1>Heading</h1>",
        "<p>Intro <b>with</b> <i>formatting</i>.</p>",
        "<ul><li>One</li><li>Two</li></ul>",
        "<p>Outro.</p>",
    ))
    .unwrap();
    let c = doc.cursor_at(0);
    c.move_position(MoveOperation::End, MoveMode::KeepAnchor, 1);
    assert_snapshot!(c.selection().to_markdown());
}

// ── Flow snapshot structure ─────────────────────────────────────────
//
// These replace the hand-rolled `ElementFingerprint` comparison in
// copy_paste_tests.rs — a snapshot of the normalised Debug
// representation covers the same ground with a diff UX.

#[test]
fn snapshot_flow_structure_simple() {
    let doc = TextDocument::new();
    doc.set_html("<p>first</p><p>second</p>").unwrap();
    let snap = doc.snapshot_flow();
    assert_debug_snapshot!(&snap.elements);
}

// ── HTML → HTML round-trip through insert_fragment ──────────────────

#[test]
fn snapshot_paste_roundtrip_preserves_table() {
    // Select-all → copy → paste-on-self must preserve the table
    // structure; snapshot captures the post-paste flow layout so any
    // structural regression (cell count, row/column ordering,
    // paragraph nesting) is one diff away from being visible.
    let doc = TextDocument::new();
    doc.set_markdown("Before\n\n| A | B |\n|---|---|\n| c | d |\n\nAfter")
        .unwrap()
        .wait()
        .unwrap();

    let c = doc.cursor_at(0);
    c.move_position(MoveOperation::End, MoveMode::KeepAnchor, 1);
    let frag = c.selection();

    let c2 = doc.cursor_at(0);
    c2.move_position(MoveOperation::End, MoveMode::KeepAnchor, 1);
    c2.insert_fragment(&frag).unwrap();

    // Shape-only view: element kinds and counts. A structural
    // regression would change this string; ID/position churn would
    // not.
    let shape: Vec<String> = doc
        .snapshot_flow()
        .elements
        .iter()
        .map(|e| match e {
            text_document::FlowElementSnapshot::Block(b) => {
                format!("Block({})", b.text)
            }
            text_document::FlowElementSnapshot::Table(t) => {
                format!("Table({}x{}, cells={})", t.rows, t.columns, t.cells.len())
            }
            text_document::FlowElementSnapshot::Frame(_) => "Frame".to_string(),
        })
        .collect();
    assert_debug_snapshot!(shape);
}

// ── Plain text reconstruction ───────────────────────────────────────

#[test]
fn snapshot_plain_text_survives_complex_insert() {
    let doc = new_doc("start ");
    let c = doc.cursor_at(6);
    c.insert_html(concat!(
        "<h2>Section</h2>",
        "<p>Paragraph with <b>bold</b>.</p>",
        "<ol><li>one</li><li>two</li></ol>",
    ))
    .unwrap();
    assert_snapshot!(doc.to_plain_text().unwrap());
}
