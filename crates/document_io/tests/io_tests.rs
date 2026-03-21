extern crate text_document_io as document_io;
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

/// Set up an in-memory database with Root(id=1) and a Document owned by it.
fn setup() -> Result<(DbContext, Arc<EventHub>, UndoRedoManager)> {
    let db_context = DbContext::new()?;
    let event_hub = Arc::new(EventHub::new());
    let mut undo_redo_manager = UndoRedoManager::new();

    // Create Root (non-undoable)
    let root = root_controller::create_orphan(&db_context, &event_hub, &CreateRootDto::default())?;

    // Create Document owned by Root
    let _doc = document_controller::create(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &CreateDocumentDto::default(),
        root.id,
        -1,
    )?;

    Ok((db_context, event_hub, undo_redo_manager))
}

#[test]
fn test_import_empty_text() -> Result<()> {
    let (db_context, event_hub, _undo_redo_manager) = setup()?;

    document_io_controller::import_plain_text(
        &db_context,
        &event_hub,
        &ImportPlainTextDto {
            plain_text: String::new(),
        },
    )?;

    let exported = document_io_controller::export_plain_text(&db_context, &event_hub)?;
    assert_eq!(exported.plain_text, "");

    Ok(())
}

#[test]
fn test_import_single_line() -> Result<()> {
    let (db_context, event_hub, _undo_redo_manager) = setup()?;

    document_io_controller::import_plain_text(
        &db_context,
        &event_hub,
        &ImportPlainTextDto {
            plain_text: "Hello World".to_string(),
        },
    )?;

    let exported = document_io_controller::export_plain_text(&db_context, &event_hub)?;
    assert_eq!(exported.plain_text, "Hello World");

    Ok(())
}

#[test]
fn test_import_multiline() -> Result<()> {
    let (db_context, event_hub, _undo_redo_manager) = setup()?;

    document_io_controller::import_plain_text(
        &db_context,
        &event_hub,
        &ImportPlainTextDto {
            plain_text: "Line 1\nLine 2\nLine 3".to_string(),
        },
    )?;

    let exported = document_io_controller::export_plain_text(&db_context, &event_hub)?;
    assert_eq!(exported.plain_text, "Line 1\nLine 2\nLine 3");

    Ok(())
}

#[test]
fn test_import_then_export_roundtrip() -> Result<()> {
    let (db_context, event_hub, _undo_redo_manager) = setup()?;

    let original = "First paragraph\nSecond paragraph\n\nFourth paragraph after blank";

    document_io_controller::import_plain_text(
        &db_context,
        &event_hub,
        &ImportPlainTextDto {
            plain_text: original.to_string(),
        },
    )?;

    let exported = document_io_controller::export_plain_text(&db_context, &event_hub)?;
    assert_eq!(exported.plain_text, original);

    Ok(())
}

#[test]
fn test_export_empty_document() -> Result<()> {
    let (db_context, event_hub, _undo_redo_manager) = setup()?;

    // The fresh document has no frames/blocks, so export should handle gracefully.
    // After setup, the document exists but has no content frames.
    // export_plain_text traverses Root -> Document -> Frames -> Blocks.
    // A fresh document has no frames, so the result should be empty.
    let exported = document_io_controller::export_plain_text(&db_context, &event_hub)?;
    assert_eq!(exported.plain_text, "");

    Ok(())
}

#[test]
fn test_import_overwrites_previous() -> Result<()> {
    let (db_context, event_hub, _) = setup()?;

    document_io_controller::import_plain_text(
        &db_context,
        &event_hub,
        &ImportPlainTextDto {
            plain_text: "First content".to_string(),
        },
    )?;

    // Import again should replace, not append
    document_io_controller::import_plain_text(
        &db_context,
        &event_hub,
        &ImportPlainTextDto {
            plain_text: "Second content".to_string(),
        },
    )?;

    let exported = document_io_controller::export_plain_text(&db_context, &event_hub)?;
    assert_eq!(exported.plain_text, "Second content");

    Ok(())
}

#[test]
fn test_import_unicode_roundtrip() -> Result<()> {
    let (db_context, event_hub, _) = setup()?;

    let text = "Héllo wörld\n日本語テキスト\nEmoji: 🎉🚀";
    document_io_controller::import_plain_text(
        &db_context,
        &event_hub,
        &ImportPlainTextDto {
            plain_text: text.to_string(),
        },
    )?;

    let exported = document_io_controller::export_plain_text(&db_context, &event_hub)?;
    assert_eq!(exported.plain_text, text);

    Ok(())
}

#[test]
fn test_import_updates_cached_fields() -> Result<()> {
    let (db_context, event_hub, _) = setup()?;

    document_io_controller::import_plain_text(
        &db_context,
        &event_hub,
        &ImportPlainTextDto {
            plain_text: "abc\ndef".to_string(),
        },
    )?;

    use document_inspection::document_inspection_controller;
    let stats = document_inspection_controller::get_document_stats(&db_context, &event_hub)?;
    assert_eq!(stats.block_count, 2);
    assert_eq!(stats.character_count, 6); // "abc" (3) + "def" (3)
    assert_eq!(stats.frame_count, 1);

    Ok(())
}
