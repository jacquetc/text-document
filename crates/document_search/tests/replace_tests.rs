use anyhow::Result;
use common::database::db_context::DbContext;
use common::event::EventHub;
use common::undo_redo::UndoRedoManager;
use std::sync::Arc;

use direct_access::document::dtos::CreateDocumentDto;
use direct_access::document::document_controller;
use direct_access::root::dtos::CreateRootDto;
use direct_access::root::root_controller;

use document_io::document_io_controller;
use document_io::ImportPlainTextDto;

use document_search::document_search_controller;
use document_search::ReplaceTextDto;

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

fn export_text(db_context: &DbContext, event_hub: &Arc<EventHub>) -> Result<String> {
    let dto = document_io_controller::export_plain_text(db_context, event_hub)?;
    Ok(dto.plain_text)
}

#[test]
fn test_replace_single() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) =
        setup_with_text("hello world hello")?;

    let result = document_search_controller::replace_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &ReplaceTextDto {
            query: "hello".to_string(),
            replacement: "hi".to_string(),
            case_sensitive: true,
            whole_word: false,
            use_regex: false,
            replace_all: false,
        },
    )?;

    assert_eq!(result.replacements_count, 1);
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "hi world hello");

    Ok(())
}

#[test]
fn test_replace_all() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) =
        setup_with_text("hello world hello")?;

    let result = document_search_controller::replace_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &ReplaceTextDto {
            query: "hello".to_string(),
            replacement: "hi".to_string(),
            case_sensitive: true,
            whole_word: false,
            use_regex: false,
            replace_all: true,
        },
    )?;

    assert_eq!(result.replacements_count, 2);
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "hi world hi");

    Ok(())
}

#[test]
fn test_replace_case_insensitive() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) =
        setup_with_text("Hello HELLO hello")?;

    let result = document_search_controller::replace_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &ReplaceTextDto {
            query: "hello".to_string(),
            replacement: "hi".to_string(),
            case_sensitive: false,
            whole_word: false,
            use_regex: false,
            replace_all: true,
        },
    )?;

    assert_eq!(result.replacements_count, 3);
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "hi hi hi");

    Ok(())
}

#[test]
fn test_replace_regex() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) =
        setup_with_text("abc 123 def 456")?;

    let result = document_search_controller::replace_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &ReplaceTextDto {
            query: r"\d+".to_string(),
            replacement: "NUM".to_string(),
            case_sensitive: true,
            whole_word: false,
            use_regex: true,
            replace_all: true,
        },
    )?;

    assert_eq!(result.replacements_count, 2);
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "abc NUM def NUM");

    Ok(())
}

#[test]
fn test_replace_not_found() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) =
        setup_with_text("hello world")?;

    let result = document_search_controller::replace_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &ReplaceTextDto {
            query: "xyz".to_string(),
            replacement: "abc".to_string(),
            case_sensitive: true,
            whole_word: false,
            use_regex: false,
            replace_all: true,
        },
    )?;

    assert_eq!(result.replacements_count, 0);
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "hello world");

    Ok(())
}

#[test]
fn test_replace_empty_query() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) =
        setup_with_text("hello world")?;

    let result = document_search_controller::replace_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &ReplaceTextDto {
            query: "".to_string(),
            replacement: "abc".to_string(),
            case_sensitive: true,
            whole_word: false,
            use_regex: false,
            replace_all: true,
        },
    )?;

    assert_eq!(result.replacements_count, 0);
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "hello world");

    Ok(())
}

#[test]
fn test_replace_undo() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) =
        setup_with_text("hello world hello")?;

    let result = document_search_controller::replace_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &ReplaceTextDto {
            query: "hello".to_string(),
            replacement: "hi".to_string(),
            case_sensitive: true,
            whole_word: false,
            use_regex: false,
            replace_all: true,
        },
    )?;

    assert_eq!(result.replacements_count, 2);
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "hi world hi");

    // Undo
    undo_redo_manager.undo(None)?;
    let text_after_undo = export_text(&db_context, &event_hub)?;
    assert_eq!(text_after_undo, "hello world hello");

    Ok(())
}

#[test]
fn test_replace_all_across_blocks() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) =
        setup_with_text("hello there\nhello again\nhello end")?;

    let result = document_search_controller::replace_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &ReplaceTextDto {
            query: "hello".to_string(),
            replacement: "hi".to_string(),
            case_sensitive: true,
            whole_word: false,
            use_regex: false,
            replace_all: true,
        },
    )?;

    assert_eq!(result.replacements_count, 3);
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "hi there\nhi again\nhi end");

    Ok(())
}

#[test]
fn test_replace_redo() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) =
        setup_with_text("hello world")?;

    document_search_controller::replace_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &ReplaceTextDto {
            query: "hello".to_string(),
            replacement: "hi".to_string(),
            case_sensitive: true,
            whole_word: false,
            use_regex: false,
            replace_all: true,
        },
    )?;

    assert_eq!(export_text(&db_context, &event_hub)?, "hi world");

    undo_redo_manager.undo(None)?;
    assert_eq!(export_text(&db_context, &event_hub)?, "hello world");

    undo_redo_manager.redo(None)?;
    assert_eq!(export_text(&db_context, &event_hub)?, "hi world");

    Ok(())
}

#[test]
fn test_replace_with_empty_deletes() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) =
        setup_with_text("hello beautiful world")?;

    let result = document_search_controller::replace_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &ReplaceTextDto {
            query: "beautiful ".to_string(),
            replacement: "".to_string(),
            case_sensitive: true,
            whole_word: false,
            use_regex: false,
            replace_all: true,
        },
    )?;

    assert_eq!(result.replacements_count, 1);
    assert_eq!(export_text(&db_context, &event_hub)?, "hello world");

    Ok(())
}

#[test]
fn test_replace_with_longer_text() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) =
        setup_with_text("a b a")?;

    let result = document_search_controller::replace_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &ReplaceTextDto {
            query: "a".to_string(),
            replacement: "xxxx".to_string(),
            case_sensitive: true,
            whole_word: false,
            use_regex: false,
            replace_all: true,
        },
    )?;

    assert_eq!(result.replacements_count, 2);
    assert_eq!(export_text(&db_context, &event_hub)?, "xxxx b xxxx");

    Ok(())
}
