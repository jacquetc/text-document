use text_document::{Alignment, BlockFormat, MoveMode, TextDocument, TextFormat};

fn new_doc_with_text(text: &str) -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text(text).unwrap();
    doc
}

#[test]
fn char_format_at_position() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor();
    let fmt = cursor.char_format().unwrap();
    // Default format: all None
    assert_eq!(fmt.font_bold, None);
    assert_eq!(fmt.font_italic, None);
}

#[test]
fn set_char_format_bold() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor();
    // Select all text
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.set_position(5, MoveMode::KeepAnchor);

    let fmt = TextFormat {
        font_bold: Some(true),
        ..Default::default()
    };
    cursor.set_char_format(&fmt).unwrap();

    // Check format at position 0
    let read_cursor = doc.cursor_at(0);
    let result_fmt = read_cursor.char_format().unwrap();
    assert_eq!(result_fmt.font_bold, Some(true));
}

#[test]
fn merge_char_format_preserves_existing() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.set_position(5, MoveMode::KeepAnchor);

    // First set bold
    let bold_fmt = TextFormat {
        font_bold: Some(true),
        ..Default::default()
    };
    cursor.set_char_format(&bold_fmt).unwrap();

    // Then merge italic only — bold should be preserved (None = don't touch)
    let italic_fmt = TextFormat {
        font_italic: Some(true),
        ..Default::default()
    };
    cursor.merge_char_format(&italic_fmt).unwrap();

    let read_cursor = doc.cursor_at(0);
    let result_fmt = read_cursor.char_format().unwrap();
    assert_eq!(result_fmt.font_bold, Some(true));
    assert_eq!(result_fmt.font_italic, Some(true));
}

#[test]
fn block_format_at_position() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor();
    let fmt = cursor.block_format().unwrap();
    // Default: no alignment set
    assert_eq!(fmt.alignment, None);
}

#[test]
fn set_block_format_alignment() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.set_position(5, MoveMode::KeepAnchor);

    let fmt = BlockFormat {
        alignment: Some(Alignment::Center),
        ..Default::default()
    };
    cursor.set_block_format(&fmt).unwrap();

    let read_cursor = doc.cursor_at(0);
    let result_fmt = read_cursor.block_format().unwrap();
    assert_eq!(result_fmt.alignment, Some(Alignment::Center));
}

#[test]
fn set_char_format_is_undoable() {
    let doc = new_doc_with_text("Hello");
    let cursor = doc.cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.set_position(5, MoveMode::KeepAnchor);

    let fmt = TextFormat {
        font_bold: Some(true),
        ..Default::default()
    };
    cursor.set_char_format(&fmt).unwrap();
    assert!(doc.can_undo());

    doc.undo().unwrap();
    let read_cursor = doc.cursor_at(0);
    let result_fmt = read_cursor.char_format().unwrap();
    // After undo, bold should be reverted
    assert_ne!(result_fmt.font_bold, Some(true));
}
