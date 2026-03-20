use anyhow::Result;
use common::database::db_context::DbContext;
use common::event::EventHub;
use common::undo_redo::UndoRedoManager;
use std::sync::Arc;

use direct_access::root::dtos::CreateRootDto;
use direct_access::root::root_controller;
use direct_access::document::dtos::CreateDocumentDto;
use direct_access::document::document_controller;

use document_io::document_io_controller;
use document_io::ImportPlainTextDto;

use document_editing::document_editing_controller;
use document_editing::{DeleteTextDto, InsertBlockDto, InsertTextDto};

/// Set up an in-memory database with Root, Document, and imported text content.
fn setup_with_text(text: &str) -> Result<(DbContext, Arc<EventHub>, UndoRedoManager)> {
    let db_context = DbContext::new()?;
    let event_hub = Arc::new(EventHub::new());
    let mut undo_redo_manager = UndoRedoManager::new();

    let root = root_controller::create_orphan(
        &db_context,
        &event_hub,
        &CreateRootDto::default(),
    )?;

    let _doc = document_controller::create(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &CreateDocumentDto::default(),
        root.id,
        -1,
    )?;

    document_io_controller::import_plain_text(
        &db_context,
        &event_hub,
        &ImportPlainTextDto {
            plain_text: text.to_string(),
        },
    )?;

    Ok((db_context, event_hub, undo_redo_manager))
}

/// Helper to export the current document text.
fn export_text(db_context: &DbContext, event_hub: &Arc<EventHub>) -> Result<String> {
    let dto = document_io_controller::export_plain_text(db_context, event_hub)?;
    Ok(dto.plain_text)
}

#[test]
fn test_insert_text_at_beginning() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let result = document_editing_controller::insert_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertTextDto {
            position: 0,
            anchor: 0,
            text: "Say ".to_string(),
        },
    )?;

    assert_eq!(result.new_position, 4);
    assert_eq!(result.blocks_affected, 1);

    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Say Hello");

    Ok(())
}

#[test]
fn test_insert_text_at_end() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let result = document_editing_controller::insert_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertTextDto {
            position: 5,
            anchor: 5,
            text: " World".to_string(),
        },
    )?;

    assert_eq!(result.new_position, 11);

    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello World");

    Ok(())
}

#[test]
fn test_insert_text_in_middle() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Helo")?;

    let result = document_editing_controller::insert_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertTextDto {
            position: 2,
            anchor: 2,
            text: "l".to_string(),
        },
    )?;

    assert_eq!(result.new_position, 3);

    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello");

    Ok(())
}

#[test]
fn test_delete_text_within_block() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello World")?;

    // Delete "World" (positions 6..11)
    let result = document_editing_controller::delete_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &DeleteTextDto {
            position: 6,
            anchor: 11,
        },
    )?;

    assert_eq!(result.new_position, 6);
    assert_eq!(result.deleted_text, "World");

    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello ");

    Ok(())
}

#[test]
fn test_delete_text_noop_same_position() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let result = document_editing_controller::delete_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &DeleteTextDto {
            position: 3,
            anchor: 3,
        },
    )?;

    assert_eq!(result.new_position, 3);
    assert_eq!(result.deleted_text, "");

    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello");

    Ok(())
}

#[test]
fn test_insert_block_creates_new_block() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("HelloWorld")?;

    // Insert a block break at position 5, splitting "HelloWorld" into "Hello" and "World"
    let result = document_editing_controller::insert_block(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertBlockDto {
            position: 5,
            anchor: 5,
        },
    )?;

    // The new block should have been created with a valid ID
    assert!(result.new_block_id > 0);
    // The new position should be at the start of the new block (after "Hello" + block separator)
    assert_eq!(result.new_position, 6);

    // Verify via document stats that block count increased from 1 to 2
    use document_inspection::document_inspection_controller;
    let stats =
        document_inspection_controller::get_document_stats(&db_context, &event_hub)?;
    assert_eq!(stats.block_count, 2);

    // Verify content via export
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello\nWorld");

    Ok(())
}

// --- InsertText: Unicode ---

#[test]
fn test_insert_text_unicode() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("café")?;

    let result = document_editing_controller::insert_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertTextDto {
            position: 4, // after "café" (4 chars, not 5 bytes)
            anchor: 4,
            text: " latte".to_string(),
        },
    )?;

    assert_eq!(result.new_position, 10); // 4 + 6
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "café latte");

    Ok(())
}

// --- InsertText: with selection (position != anchor) ---

#[test]
fn test_insert_text_replaces_selection() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello World")?;

    // Select "World" (6..11) and replace with "Rust"
    let result = document_editing_controller::insert_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertTextDto {
            position: 6,
            anchor: 11,
            text: "Rust".to_string(),
        },
    )?;

    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello Rust");
    assert_eq!(result.new_position, 10); // 6 + 4

    Ok(())
}

// --- DeleteText: reversed anchor/position ---

#[test]
fn test_delete_text_reversed_range() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello World")?;

    // anchor < position (reversed selection)
    let result = document_editing_controller::delete_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &DeleteTextDto {
            position: 11,
            anchor: 6,
        },
    )?;

    assert_eq!(result.new_position, 6);
    assert_eq!(result.deleted_text, "World");
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello ");

    Ok(())
}

// --- DeleteText: cross-block ---

#[test]
fn test_delete_text_cross_block() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello\nWorld")?;

    // Delete from position 3 to 9: "lo\nWor" -> merges blocks into "Helld"
    // "Hello" pos 0-4, separator at 5, "World" pos 6-10
    // Delete chars 3..9 = "lo" + separator + "Wor"
    let result = document_editing_controller::delete_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &DeleteTextDto {
            position: 3,
            anchor: 9,
        },
    )?;

    assert_eq!(result.new_position, 3);
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Helld");

    Ok(())
}

// --- DeleteText: entire block content ---

#[test]
fn test_delete_text_entire_content() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let result = document_editing_controller::delete_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &DeleteTextDto {
            position: 0,
            anchor: 5,
        },
    )?;

    assert_eq!(result.new_position, 0);
    assert_eq!(result.deleted_text, "Hello");
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "");

    Ok(())
}

// --- InsertBlock: at block boundaries ---

#[test]
fn test_insert_block_at_start() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let result = document_editing_controller::insert_block(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertBlockDto {
            position: 0,
            anchor: 0,
        },
    )?;

    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "\nHello");
    assert!(result.new_block_id > 0);

    Ok(())
}

#[test]
fn test_insert_block_at_end() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let result = document_editing_controller::insert_block(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertBlockDto {
            position: 5,
            anchor: 5,
        },
    )?;

    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello\n");
    assert!(result.new_block_id > 0);

    Ok(())
}

// --- InsertText: updates cached fields ---

#[test]
fn test_insert_text_updates_stats() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hi")?;

    document_editing_controller::insert_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertTextDto {
            position: 2,
            anchor: 2,
            text: " there".to_string(),
        },
    )?;

    use document_inspection::document_inspection_controller;
    let stats = document_inspection_controller::get_document_stats(&db_context, &event_hub)?;
    assert_eq!(stats.character_count, 8); // "Hi there"
    assert_eq!(stats.block_count, 1);

    Ok(())
}

// --- Undo/Redo ---

#[test]
fn test_insert_text_undo() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    document_editing_controller::insert_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertTextDto {
            position: 5,
            anchor: 5,
            text: " World".to_string(),
        },
    )?;

    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello World");

    // Undo
    undo_redo_manager.undo(None)?;
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello");

    // Redo
    undo_redo_manager.redo(None)?;
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello World");

    Ok(())
}

#[test]
fn test_delete_text_undo() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello World")?;

    document_editing_controller::delete_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &DeleteTextDto {
            position: 5,
            anchor: 11,
        },
    )?;

    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello");

    // Undo should restore " World"
    undo_redo_manager.undo(None)?;
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello World");

    Ok(())
}

#[test]
fn test_insert_block_undo() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("HelloWorld")?;

    document_editing_controller::insert_block(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertBlockDto {
            position: 5,
            anchor: 5,
        },
    )?;

    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello\nWorld");

    // Undo should merge back
    undo_redo_manager.undo(None)?;
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "HelloWorld");

    Ok(())
}
