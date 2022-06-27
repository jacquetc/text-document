use text_document::{text_document::TextDocument, text_cursor::MoveMode, format::BlockFormat};

#[test]
fn create_document() {
    let document = TextDocument::new();
    document.print_debug_elements();
    assert_eq!(document.block_count(), 1);
}

#[test]
fn add_text() {
    let mut document = TextDocument::new();
    document.set_plain_text("aa\na");
    document.print_debug_elements();

    
    assert_eq!(document.block_count(), 2);
}


#[test]
fn get_next_sibling() {
    let mut document = TextDocument::new();

    
    
    
    assert_eq!(document.block_count(), 1);
}


#[test]
fn cursor_insert_block() {
    let document = TextDocument::new();
    document.print_debug_elements();

    let mut cursor = document.create_cursor();
    cursor.set_position(0, MoveMode::KeepAnchor);


    cursor.insert_block(BlockFormat::new()).expect("Testing block insertion");
    document.print_debug_elements();

    assert_eq!(document.block_count(), 2);
}



#[test]
fn cursor_insert_plain_text() {
    let document = TextDocument::new();

    let mut cursor = document.create_cursor();
    cursor.set_position(0, MoveMode::KeepAnchor);
    cursor.insert_plain_text("\nplain_text");
    //cursor.insert_plain_text("\nplain_text\ntest");
    document.print_debug_elements();
 
    assert_eq!(document.block_count(), 3);
}



#[test]
fn cursor_insert_plain_text_into_filled_block() {
    let mut document = TextDocument::new();
    document.set_plain_text("beginningend");
    document.print_debug_elements();
    document.add_cursor_change_callback(|position, removed_characters, added_characters|{ println!("");} );

    let mut cursor = document.create_cursor();
    cursor.set_position(9, MoveMode::KeepAnchor);
    cursor.insert_plain_text("new\nplain_text\ntest");
    document.print_debug_elements();

    assert_eq!(document.block_count(), 3);
}

