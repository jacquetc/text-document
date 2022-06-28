use text_document::{text_document::TextDocument, text_cursor::MoveMode, format::BlockFormat};
mod common;

#[test]
fn cursor_insert_block() {
    let document = TextDocument::new();
    document.print_debug_elements();

    let mut cursor = document.create_cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);


    cursor.insert_block(BlockFormat::new()).expect("Testing block insertion");
    document.print_debug_elements();

    assert_eq!(document.block_count(), 2);
}



#[test]
fn cursor_insert_plain_text() {
    let document = TextDocument::new();

    let mut cursor = document.create_cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.insert_plain_text("\nplain_text\ntest");
    document.print_debug_elements();
 
    assert_eq!(document.block_count(), 3);
}

#[test]
fn cursor_insert_plain_text_at_position() {
    let document = TextDocument::new();

    let mut cursor = document.create_cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.insert_plain_text("AB");
    cursor.set_position(1, MoveMode::MoveAnchor);
    cursor.insert_plain_text("\nplain_text\ntest");
    document.print_debug_elements();
 
    assert_eq!(document.block_count(), 3);

    cursor.set_position(2, MoveMode::MoveAnchor);
    cursor.set_position(7, MoveMode::KeepAnchor);
    assert_eq!(cursor.selected_text(), "plain");

    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.set_position(5, MoveMode::KeepAnchor);
    assert_eq!(cursor.selected_text(), "AB\npl");
}

#[test]
fn cursor_insert_single_line_plain_text_at_position() {
    let document = TextDocument::new();

    let mut cursor = document.create_cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.insert_plain_text("AB");
    cursor.set_position(1, MoveMode::MoveAnchor);
    cursor.insert_plain_text("plain_text");
    document.print_debug_elements();
 
    assert_eq!(document.block_count(), 1);
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.set_position(12, MoveMode::KeepAnchor);
    assert_eq!(cursor.selected_text(), "Aplain_textB");
}

#[test]
fn cursor_select_text() {
    let document = TextDocument::new();

    let mut cursor = document.create_cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.insert_plain_text("a\nplain_text\ntest");
    
    document.print_debug_elements();
 
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.set_position(1, MoveMode::KeepAnchor);
    assert_eq!(cursor.selected_text(), "a");

    cursor.set_position(2, MoveMode::MoveAnchor);
    cursor.set_position(7, MoveMode::KeepAnchor);
    assert_eq!(cursor.selected_text(), "plain");
}


#[test]
fn cursor_insert_plain_text_into_filled_block() {
    // let mut document = TextDocument::new();
    // document.set_plain_text("beginningend");
    // document.print_debug_elements();
    // document.add_cursor_change_callback(|position, removed_characters, added_characters|{ println!("");} );

    // let mut cursor = document.create_cursor();
    // cursor.set_position(9, MoveMode::MoveAnchor);
    // cursor.insert_plain_text("new\nplain_text\ntest");
    // document.print_debug_elements();

    // assert_eq!(document.block_count(), 3);
}

