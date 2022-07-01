#![cfg(test)]
use text_document::{
    text_cursor::MoveMode,
    text_document::{ChangeReason, TextDocument},
    MoveOperation,
};
mod common;

#[test]
fn cursor_insert_block() {
    let document = TextDocument::new();
    document.print_debug_elements();

    let mut cursor = document.create_cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);

    cursor.insert_block().expect("Testing block insertion");
    document.print_debug_elements();

    assert_eq!(document.block_count(), 2);
}

#[test]
fn cursor_insert_plain_text() {
    let document = TextDocument::new();

    let mut cursor = document.create_cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.insert_plain_text("\nplain_text\ntest").unwrap();
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
    cursor.insert_plain_text("\nplain_text\ntest").unwrap();
    document.print_debug_elements();

    assert_eq!(document.block_count(), 3);

    cursor.set_position(2, MoveMode::MoveAnchor);
    cursor.set_position(7, MoveMode::KeepAnchor);
    assert_eq!(cursor.selected_text(), "plain");

    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.set_position(5, MoveMode::KeepAnchor);
    assert_eq!(cursor.selected_text(), "A\npla");
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
    let mut document = TextDocument::new();
    document.set_plain_text("beginningend").unwrap();
    document.print_debug_elements();

    document.add_text_change_callback(|position, removed_characters, added_characters| {
        println!(
            "position: {}, removed_characters: {}, added_characters: {}",
            position, removed_characters, added_characters
        );
        assert_eq!(position, 9);
        assert_eq!(added_characters, 19);
    });

    let mut cursor = document.create_cursor();
    cursor.set_position(9, MoveMode::MoveAnchor);
    cursor.insert_plain_text("new\nplain_text\ntest");
    document.print_debug_elements();

    assert_eq!(document.block_count(), 3);
}

#[test]
fn callbacks() {
    let mut document = TextDocument::new();

    document.add_text_change_callback(|position, removed_characters, added_characters| {
        println!(
            "position: {}, removed_characters: {}, added_characters: {}",
            position, removed_characters, added_characters
        );
        assert_eq!(position, 0);
        assert_eq!(removed_characters, 0);
        assert_eq!(added_characters, 19);
    });

    document.add_element_change_callback(|element, reason| {
        assert_eq!(element.uuid(), 0);
        assert_eq!(reason, ChangeReason::ChildrenChanged);
    });

    document.set_plain_text("new\nplain_text\ntest").unwrap();
}

#[test]
fn remove_in_blocks_at_the_same_level() {
    let mut document = TextDocument::new();
    document.set_plain_text("beginning\nblock\nend").unwrap();
    document.print_debug_elements();

    document.add_text_change_callback(|position, removed_characters, added_characters| {
        println!(
            "position: {}, removed_characters: {}, added_characters: {}",
            position, removed_characters, added_characters
        );
        assert_eq!(position, 3);
        assert_eq!(removed_characters, 14);
    });

    document.add_element_change_callback(|element, reason| {
        assert_eq!(element.uuid(), 0);
        assert_eq!(reason, ChangeReason::ChildrenChanged);
    });

    let mut cursor = document.create_cursor();
    cursor.set_position(3, MoveMode::MoveAnchor);
    cursor.set_position(17, MoveMode::KeepAnchor);
    cursor.remove().unwrap();
    document.print_debug_elements();

    assert_eq!(document.block_count(), 1);
    assert_eq!(document.to_plain_text(), "begnd");
}

#[test]
fn remove_in_blocks_where_top_is_child_of_bottom_block() {
    let document = TextDocument::new();
    //document.set_plain_text("beginning\nblock\nend").unwrap();
    document.print_debug_elements();

    document.add_text_change_callback(|position, removed_characters, added_characters| {
        println!(
            "position: {}, removed_characters: {}, added_characters: {}",
            position, removed_characters, added_characters
        );
        // assert_eq!(position, 9);
        // assert_eq!(removed_characters, 19);
    });

    document.add_element_change_callback(|element, reason| {
        // assert_eq!(element.uuid(), 0);
        // assert_eq!(reason, ChangeReason::ChildrenChanged );
    });

    let mut cursor = document.create_cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.insert_frame().unwrap();
    cursor.insert_plain_text("beginning").unwrap();
    document.print_debug_elements();

    assert_eq!(cursor.position(), 10);

    cursor.insert_block().unwrap();
    document.print_debug_elements();

    cursor.set_position(17, MoveMode::MoveAnchor);
    cursor.insert_plain_text("end").unwrap();

    document.print_debug_elements();

    //position and remove
    cursor.set_position(4, MoveMode::MoveAnchor);
    cursor.set_position(13, MoveMode::KeepAnchor);
    cursor.remove().unwrap();
    document.print_debug_elements();

    assert_eq!(document.block_count(), 2);
    assert_eq!(document.to_plain_text(), "\nnd");
}

#[test]
fn remove_in_blocks_where_bottom_is_child_of_top_block() {
    let document = TextDocument::new();
    //document.set_plain_text("beginning\nblock\nend").unwrap();
    document.print_debug_elements();

    document.add_text_change_callback(|position, removed_characters, added_characters| {
        println!(
            "position: {}, removed_characters: {}, added_characters: {}",
            position, removed_characters, added_characters
        );
        // assert_eq!(position, 9);
        // assert_eq!(removed_characters, 19);
    });

    document.add_element_change_callback(|element, reason| {
        // assert_eq!(element.uuid(), 0);
        // assert_eq!(reason, ChangeReason::ChildrenChanged );
    });

    let mut cursor = document.create_cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.insert_plain_text("beginning").unwrap();
    cursor.insert_block().unwrap();
    cursor.insert_frame().unwrap();
    cursor.insert_block().unwrap();
    cursor.insert_plain_text("end").unwrap();
    cursor.insert_block().unwrap();
    document.print_debug_elements();

    assert_eq!(cursor.position(), 16);

    //position and remove
    cursor.set_position(3, MoveMode::MoveAnchor);
    cursor.set_position(13, MoveMode::KeepAnchor);
    cursor.remove().unwrap();
    document.print_debug_elements();

    assert_eq!(document.block_count(), 2);
    assert_eq!(document.to_plain_text(), "beg\n");
}

#[test]
fn remove_in_blocks_where_bottom_child_and_top_block_are_on_their_own_frame() {
    let document = TextDocument::new();
    //document.set_plain_text("beginning\nblock\nend").unwrap();
    document.print_debug_elements();

    document.add_text_change_callback(|position, removed_characters, added_characters| {
        println!(
            "position: {}, removed_characters: {}, added_characters: {}",
            position, removed_characters, added_characters
        );
        // assert_eq!(position, 9);
        // assert_eq!(removed_characters, 19);
    });

    document.add_element_change_callback(|element, reason| {
        // assert_eq!(element.uuid(), 0);
        // assert_eq!(reason, ChangeReason::ChildrenChanged );
    });

    let mut cursor = document.create_cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.insert_frame().unwrap();
    cursor.insert_plain_text("beginning").unwrap();
    cursor.insert_block().unwrap();
    cursor.move_position(MoveOperation::NextCharacter, MoveMode::MoveAnchor);
    cursor.insert_frame().unwrap();
    cursor.insert_block().unwrap();
    cursor.insert_plain_text("end").unwrap();
    cursor.insert_block().unwrap();
    document.print_debug_elements();

    assert_eq!(cursor.position(), 18);

    //position and remove
    cursor.set_position(3, MoveMode::MoveAnchor);
    cursor.set_position(15, MoveMode::KeepAnchor);
    cursor.remove().unwrap();
    document.print_debug_elements();

    assert_eq!(document.block_count(), 1);
    assert_eq!(document.to_plain_text(), "");
}

#[test]
fn remove_in_blocks_where_bottom_child_and_top_block_are_the_same() {
    let document = TextDocument::new();
    //document.set_plain_text("beginning\nblock\nend").unwrap();
    document.print_debug_elements();

    document.add_text_change_callback(|position, removed_characters, added_characters| {
        println!(
            "position: {}, removed_characters: {}, added_characters: {}",
            position, removed_characters, added_characters
        );
        // assert_eq!(position, 9);
        // assert_eq!(removed_characters, 19);
    });

    document.add_element_change_callback(|element, reason| {
        // assert_eq!(element.uuid(), 0);
        // assert_eq!(reason, ChangeReason::ChildrenChanged );
    });

    let mut cursor = document.create_cursor();
    cursor.set_position(0, MoveMode::MoveAnchor);
    cursor.insert_plain_text("beginning end").unwrap();

    assert_eq!(cursor.position(), 13);

    //position and remove
    cursor.set_position(3, MoveMode::MoveAnchor);
    cursor.set_position(10, MoveMode::KeepAnchor);
    cursor.remove().unwrap();
    document.print_debug_elements();

    assert_eq!(document.block_count(), 1);
    assert_eq!(document.to_plain_text(), "begend");
}

#[test]
fn move_operation() {
    let mut document = TextDocument::new();
    document.set_plain_text("beginning\nblock\nend").unwrap();
    document.print_debug_elements();

    let mut cursor = document.create_cursor();
    cursor.move_position(text_document::MoveOperation::End, MoveMode::MoveAnchor);

    assert_eq!(cursor.position(), 19);
}

#[test]
fn move_cursor() {
    let mut document = TextDocument::new();
    document.set_plain_text("beginning\nblock\nend").unwrap();

    let mut cursor = document.create_cursor();
    cursor.set_position(19, MoveMode::MoveAnchor);

    assert_eq!(cursor.position(), 19);
    assert_eq!(cursor.anchor_position(), 19);

    cursor.set_position(20, MoveMode::MoveAnchor);

    assert_eq!(cursor.position(), 19);
    assert_eq!(cursor.anchor_position(), 19);

    cursor.set_position(10, MoveMode::KeepAnchor);

    assert_eq!(cursor.position(), 10);
    assert_eq!(cursor.anchor_position(), 19);
}

// #[test]
// fn insert_block_
