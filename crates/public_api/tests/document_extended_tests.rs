use text_document::{
    Alignment, BlockFormat, FindOptions, ResourceType, TextDirection, TextDocument, TextFormat,
    WrapMode,
};

fn new_doc(text: &str) -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text(text).unwrap();
    doc
}

// ── Block format at position ────────────────────────────────────

#[test]
fn block_format_at_position() {
    let doc = new_doc("Hello\nWorld");
    let fmt = doc.block_format_at(0).unwrap();
    let _ = fmt.alignment;
}

#[test]
fn block_format_at_second_block() {
    let doc = new_doc("First\nSecond");
    let fmt = doc.block_format_at(6).unwrap();
    let _ = fmt.alignment;
}

// ── Resources ───────────────────────────────────────────────────

#[test]
fn add_and_retrieve_resource() {
    let doc = new_doc("Hello");
    doc.add_resource(
        ResourceType::Image,
        "test.png",
        "image/png",
        b"fake-png-data",
    )
    .unwrap();
    let data = doc.resource("test.png").unwrap();
    assert!(data.is_some());
    assert_eq!(data.unwrap(), b"fake-png-data");
}

#[test]
fn resource_not_found() {
    let doc = new_doc("Hello");
    let data = doc.resource("nonexistent.png").unwrap();
    assert!(data.is_none());
}

#[test]
fn add_multiple_resources() {
    let doc = new_doc("Hello");
    doc.add_resource(ResourceType::Image, "a.png", "image/png", b"aaa")
        .unwrap();
    doc.add_resource(ResourceType::StyleSheet, "style.css", "text/css", b"body{}")
        .unwrap();
    assert!(doc.resource("a.png").unwrap().is_some());
    assert!(doc.resource("style.css").unwrap().is_some());
}

// ── Title ───────────────────────────────────────────────────────

#[test]
fn set_and_get_title() {
    let doc = new_doc("Hello");
    doc.set_title("My Document").unwrap();
    assert_eq!(doc.title(), "My Document");
}

#[test]
fn default_title_is_empty() {
    let doc = TextDocument::new();
    assert!(doc.title().is_empty());
}

// ── Text direction ──────────────────────────────────────────────

#[test]
fn text_direction_default_ltr() {
    let doc = TextDocument::new();
    assert_eq!(doc.text_direction(), TextDirection::LeftToRight);
}

#[test]
fn set_text_direction_rtl() {
    let doc = new_doc("Hello");
    doc.set_text_direction(TextDirection::RightToLeft).unwrap();
    assert_eq!(doc.text_direction(), TextDirection::RightToLeft);
}

// ── Wrap mode ───────────────────────────────────────────────────

#[test]
fn wrap_mode_default() {
    let doc = TextDocument::new();
    // Default is NoWrap
    assert_eq!(doc.default_wrap_mode(), WrapMode::NoWrap);
}

#[test]
fn set_wrap_mode() {
    let doc = new_doc("Hello");
    doc.set_default_wrap_mode(WrapMode::NoWrap).unwrap();
    assert_eq!(doc.default_wrap_mode(), WrapMode::NoWrap);
}

#[test]
fn set_wrap_mode_all_variants() {
    let doc = new_doc("Hello");
    for mode in [
        WrapMode::NoWrap,
        WrapMode::WordWrap,
        WrapMode::WrapAnywhere,
        WrapMode::WrapAtWordBoundaryOrAnywhere,
    ] {
        doc.set_default_wrap_mode(mode.clone()).unwrap();
        assert_eq!(doc.default_wrap_mode(), mode);
    }
}

// ── Undo / Redo ─────────────────────────────────────────────────

#[test]
fn can_undo_and_redo() {
    let doc = new_doc("Hello");
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());

    let c = doc.cursor_at(5);
    c.insert_text(" world").unwrap();
    assert!(doc.can_undo());
    assert!(!doc.can_redo());

    doc.undo().unwrap();
    assert!(!doc.can_undo());
    assert!(doc.can_redo());

    doc.redo().unwrap();
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
}

#[test]
fn clear_undo_redo() {
    let doc = new_doc("Hello");
    let c = doc.cursor_at(5);
    c.insert_text(" world").unwrap();
    assert!(doc.can_undo());

    doc.clear_undo_redo();
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());
}

// ── Search ──────────────────────────────────────────────────────

#[test]
fn find_basic() {
    let doc = new_doc("Hello world hello");
    let opts = FindOptions::default();
    let m = doc.find("hello", 0, &opts).unwrap();
    assert!(m.is_some());
    let m = m.unwrap();
    assert_eq!(m.position, 0);
    assert_eq!(m.length, 5);
}

#[test]
fn find_from_offset() {
    let doc = new_doc("Hello world hello");
    let opts = FindOptions::default();
    let m = doc.find("hello", 5, &opts).unwrap();
    assert!(m.is_some());
    assert_eq!(m.unwrap().position, 12);
}

#[test]
fn find_not_found() {
    let doc = new_doc("Hello world");
    let opts = FindOptions::default();
    let m = doc.find("xyz", 0, &opts).unwrap();
    assert!(m.is_none());
}

#[test]
fn find_backward() {
    let doc = new_doc("Hello world hello");
    let opts = FindOptions {
        search_backward: true,
        ..Default::default()
    };
    let m = doc.find("hello", 17, &opts).unwrap();
    assert!(m.is_some());
}

#[test]
fn find_case_sensitive() {
    let doc = new_doc("Hello HELLO hello");
    let opts = FindOptions {
        case_sensitive: true,
        ..Default::default()
    };
    let matches = doc.find_all("Hello", &opts).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].position, 0);
}

#[test]
fn find_whole_word() {
    let doc = new_doc("hello helloworld hello");
    let opts = FindOptions {
        whole_word: true,
        ..Default::default()
    };
    let matches = doc.find_all("hello", &opts).unwrap();
    assert_eq!(matches.len(), 2); // first and last, not "helloworld"
}

#[test]
fn find_regex() {
    let doc = new_doc("foo123 bar456");
    let opts = FindOptions {
        use_regex: true,
        ..Default::default()
    };
    let matches = doc.find_all(r"\d+", &opts).unwrap();
    assert_eq!(matches.len(), 2);
}

#[test]
fn replace_text() {
    let doc = new_doc("Hello world");
    let opts = FindOptions::default();
    let count = doc.replace_text("world", "Rust", false, &opts).unwrap();
    assert_eq!(count, 1);
    assert_eq!(doc.to_plain_text().unwrap(), "Hello Rust");
}

#[test]
fn replace_all() {
    let doc = new_doc("foo bar foo baz foo");
    let opts = FindOptions::default();
    let count = doc.replace_text("foo", "X", true, &opts).unwrap();
    assert_eq!(count, 3);
    assert_eq!(doc.to_plain_text().unwrap(), "X bar X baz X");
}

// ── Text format with full fields ────────────────────────────────

#[test]
fn set_text_format_full() {
    let doc = new_doc("Hello world");
    let c = doc.cursor();
    c.set_position(5, text_document::MoveMode::KeepAnchor);
    let fmt = TextFormat {
        font_family: Some("Courier".into()),
        font_point_size: Some(12),
        font_weight: Some(700),
        font_bold: Some(true),
        font_italic: Some(true),
        font_underline: Some(true),
        font_overline: Some(true),
        font_strikeout: Some(true),
        letter_spacing: Some(2),
        word_spacing: Some(3),
        underline_style: Some(text_document::UnderlineStyle::WaveUnderline),
        vertical_alignment: Some(text_document::CharVerticalAlignment::SuperScript),
        anchor_href: None,
        anchor_names: vec![],
        is_anchor: None,
        tooltip: None,
    };
    c.set_char_format(&fmt).unwrap();
}

// ── Block format with full fields ───────────────────────────────

#[test]
fn set_block_format_full() {
    let doc = new_doc("Hello");
    let c = doc.cursor();
    let fmt = BlockFormat {
        alignment: Some(Alignment::Center),
        heading_level: Some(2),
        indent: Some(1),
        marker: Some(text_document::MarkerType::Checked),
        ..Default::default()
    };
    c.set_block_format(&fmt).unwrap();
}

// ── Event subscription ──────────────────────────────────────────

#[test]
fn on_change_callback() {
    use std::sync::{Arc, Mutex};
    let doc = new_doc("Hello");
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();
    let _sub = doc.on_change(move |e| {
        events_clone.lock().unwrap().push(format!("{:?}", e));
    });

    let c = doc.cursor_at(5);
    c.insert_text(" world").unwrap();

    let collected = events.lock().unwrap();
    assert!(!collected.is_empty());
}

#[test]
fn subscription_drop_stops_events() {
    use std::sync::{Arc, Mutex};
    let doc = new_doc("Hello");
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();

    {
        let _sub = doc.on_change(move |e| {
            events_clone.lock().unwrap().push(format!("{:?}", e));
        });
        let c = doc.cursor_at(5);
        c.insert_text("!").unwrap();
    }
    // Sub dropped

    let before = events.lock().unwrap().len();
    let c = doc.cursor_at(6);
    c.insert_text("!").unwrap();
    let after = events.lock().unwrap().len();
    // After dropping sub, no new events should fire
    assert_eq!(before, after);
}

// ── Import/export roundtrips ────────────────────────────────────

#[test]
fn plain_text_roundtrip() {
    let doc = new_doc("Hello\nWorld");
    let text = doc.to_plain_text().unwrap();
    assert_eq!(text, "Hello\nWorld");
}

#[test]
fn html_export() {
    let doc = new_doc("Hello world");
    let html = doc.to_html().unwrap();
    assert!(html.contains("Hello world"));
}

#[test]
fn markdown_export() {
    let doc = new_doc("Hello world");
    let md = doc.to_markdown().unwrap();
    assert!(md.contains("Hello world"));
}

#[test]
fn latex_export_no_preamble() {
    let doc = new_doc("Hello");
    let latex = doc.to_latex("article", false).unwrap();
    assert!(latex.contains("Hello"));
}

#[test]
fn latex_export_with_preamble() {
    let doc = new_doc("Hello");
    let latex = doc.to_latex("book", true).unwrap();
    assert!(latex.contains("\\documentclass"));
    assert!(latex.contains("Hello"));
}
