use anyhow::Result;
use common::database::db_context::DbContext;
use common::event::EventHub;
use common::undo_redo::UndoRedoManager;
use std::sync::Arc;

use direct_access::document::document_controller;
use direct_access::document::dtos::CreateDocumentDto;
use direct_access::root::dtos::CreateRootDto;
use direct_access::root::root_controller;

use document_io::ImportPlainTextDto;
use document_io::document_io_controller;

use document_inspection::document_inspection_controller;
use document_inspection::{GetBlockAtPositionDto, GetTextAtPositionDto};

/// Set up an in-memory database with Root, Document, and imported text content.
fn setup_with_text(text: &str) -> Result<(DbContext, Arc<EventHub>, UndoRedoManager)> {
    let db_context = DbContext::new()?;
    let event_hub = Arc::new(EventHub::new());
    let mut undo_redo_manager = UndoRedoManager::new();

    let root = root_controller::create_orphan(&db_context, &event_hub, &CreateRootDto::default())?;

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

#[test]
fn test_get_document_stats_empty() -> Result<()> {
    let (db_context, event_hub, _undo_redo_manager) = setup_with_text("")?;

    let stats = document_inspection_controller::get_document_stats(&db_context, &event_hub)?;

    assert_eq!(stats.block_count, 1); // Even empty text creates one block
    assert_eq!(stats.character_count, 0);
    assert_eq!(stats.word_count, 0);
    assert_eq!(stats.frame_count, 1);
    assert_eq!(stats.image_count, 0);
    assert_eq!(stats.list_count, 0);

    Ok(())
}

#[test]
fn test_get_document_stats_after_import() -> Result<()> {
    let (db_context, event_hub, _undo_redo_manager) =
        setup_with_text("Hello World\nSecond line\nThird line")?;

    let stats = document_inspection_controller::get_document_stats(&db_context, &event_hub)?;

    assert_eq!(stats.block_count, 3);
    // character_count = 11 + 11 + 10 = 32
    assert_eq!(stats.character_count, 32);
    // word_count: "Hello" "World" + "Second" "line" + "Third" "line" = 6
    assert_eq!(stats.word_count, 6);
    assert_eq!(stats.frame_count, 1);
    assert_eq!(stats.image_count, 0);
    assert_eq!(stats.list_count, 0);

    Ok(())
}

#[test]
fn test_get_text_at_position() -> Result<()> {
    let (db_context, event_hub, _undo_redo_manager) = setup_with_text("Hello World")?;

    let result = document_inspection_controller::get_text_at_position(
        &db_context,
        &event_hub,
        &GetTextAtPositionDto {
            position: 0,
            length: 5,
        },
    )?;

    assert_eq!(result.text, "Hello");
    assert!(result.block_id > 0);
    assert!(result.element_id > 0);

    Ok(())
}

#[test]
fn test_get_text_at_position_middle() -> Result<()> {
    let (db_context, event_hub, _undo_redo_manager) = setup_with_text("Hello World")?;

    let result = document_inspection_controller::get_text_at_position(
        &db_context,
        &event_hub,
        &GetTextAtPositionDto {
            position: 6,
            length: 5,
        },
    )?;

    assert_eq!(result.text, "World");

    Ok(())
}

#[test]
fn test_get_block_at_position_first_block() -> Result<()> {
    let (db_context, event_hub, _undo_redo_manager) = setup_with_text("First\nSecond\nThird")?;

    let result = document_inspection_controller::get_block_at_position(
        &db_context,
        &event_hub,
        &GetBlockAtPositionDto { position: 0 },
    )?;

    assert_eq!(result.block_number, 0);
    assert_eq!(result.block_start, 0);
    assert_eq!(result.block_length, 5); // "First"
    assert!(result.block_id > 0);

    Ok(())
}

#[test]
fn test_get_block_at_position_second_block() -> Result<()> {
    let (db_context, event_hub, _undo_redo_manager) = setup_with_text("First\nSecond\nThird")?;

    // "First" is positions 0-4, block separator at 5, "Second" starts at 6
    let result = document_inspection_controller::get_block_at_position(
        &db_context,
        &event_hub,
        &GetBlockAtPositionDto { position: 6 },
    )?;

    assert_eq!(result.block_number, 1);
    assert_eq!(result.block_start, 6);
    assert_eq!(result.block_length, 6); // "Second"

    Ok(())
}

#[test]
fn test_get_text_at_position_cross_block() -> Result<()> {
    let (db_context, event_hub, _) = setup_with_text("Hello\nWorld")?;

    // Block 0: "Hello" at pos 0, len 5
    // Block separator at pos 5
    // Block 1: "World" at pos 6, len 5
    // Starting at pos 3, length 6 should give "lo\nWor"
    let result = document_inspection_controller::get_text_at_position(
        &db_context,
        &event_hub,
        &GetTextAtPositionDto {
            position: 3,
            length: 6,
        },
    )?;

    assert_eq!(result.text, "lo\nWor");

    Ok(())
}

#[test]
fn test_get_text_at_position_end_of_document() -> Result<()> {
    let (db_context, event_hub, _) = setup_with_text("Hello")?;

    // Position at very end of document (position 5, length 0)
    let result = document_inspection_controller::get_text_at_position(
        &db_context,
        &event_hub,
        &GetTextAtPositionDto {
            position: 5,
            length: 0,
        },
    )?;

    assert_eq!(result.text, "");

    Ok(())
}

#[test]
fn test_get_document_stats_unicode() -> Result<()> {
    let (db_context, event_hub, _) = setup_with_text("café résumé")?;

    let stats = document_inspection_controller::get_document_stats(&db_context, &event_hub)?;

    // "café résumé" = 11 characters (not 13 bytes)
    assert_eq!(stats.character_count, 11);
    assert_eq!(stats.word_count, 2);

    Ok(())
}

#[test]
fn test_get_block_at_position_separator() -> Result<()> {
    let (db_context, event_hub, _) = setup_with_text("abc\ndef")?;
    // "abc" at pos 0-2 (len 3), separator at pos 3, "def" at pos 4-6 (len 3)

    // Position 2 = last char of first block ('c'). Should be in first block.
    let result = document_inspection_controller::get_block_at_position(
        &db_context,
        &event_hub,
        &GetBlockAtPositionDto { position: 2 },
    )?;
    assert_eq!(result.block_number, 0);
    assert_eq!(result.block_start, 0);

    // Position 3 = block separator between "abc" and "def". Should land in second block.
    let result = document_inspection_controller::get_block_at_position(
        &db_context,
        &event_hub,
        &GetBlockAtPositionDto { position: 3 },
    )?;
    assert_eq!(result.block_number, 1);
    assert_eq!(result.block_start, 4);

    // Position 4 = first char of second block ('d'). Should be in second block.
    let result = document_inspection_controller::get_block_at_position(
        &db_context,
        &event_hub,
        &GetBlockAtPositionDto { position: 4 },
    )?;
    assert_eq!(result.block_number, 1);
    assert_eq!(result.block_start, 4);

    Ok(())
}

#[test]
fn test_get_text_at_position_truncated_at_end() -> Result<()> {
    let (db_context, event_hub, _) = setup_with_text("Hello")?;

    // Request more text than available
    let result = document_inspection_controller::get_text_at_position(
        &db_context,
        &event_hub,
        &GetTextAtPositionDto {
            position: 3,
            length: 100,
        },
    )?;

    assert_eq!(result.text, "lo"); // only 2 chars remaining

    Ok(())
}
