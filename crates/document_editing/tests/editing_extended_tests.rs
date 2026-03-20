use anyhow::Result;
use common::database::db_context::DbContext;
use common::event::EventHub;
use common::undo_redo::UndoRedoManager;
use std::sync::Arc;

use direct_access::root::dtos::CreateRootDto;
use direct_access::root::root_controller;
use direct_access::document::dtos::CreateDocumentDto;
use direct_access::document::document_controller;
use direct_access::block::block_controller;
use direct_access::inline_element::inline_element_controller;
use direct_access::frame::frame_controller;

use common::direct_access::block::block_repository::BlockRelationshipField;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::types::EntityId;

use document_io::document_io_controller;
use document_io::ImportPlainTextDto;

use document_editing::document_editing_controller;
use document_editing::*;

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

/// Get the first block's element IDs via the entity tree traversal.
fn get_first_block_element_ids(db_context: &DbContext) -> Result<Vec<EntityId>> {
    let root = root_controller::get(db_context, &1)?.expect("Root not found");
    let doc_ids = root_controller::get_relationship(
        db_context,
        &root.id,
        &RootRelationshipField::Document,
    )?;
    let frame_ids = document_controller::get_relationship(
        db_context,
        &doc_ids[0],
        &DocumentRelationshipField::Frames,
    )?;
    let block_ids = frame_controller::get_relationship(
        db_context,
        &frame_ids[0],
        &FrameRelationshipField::Blocks,
    )?;
    let elem_ids = block_controller::get_relationship(
        db_context,
        &block_ids[0],
        &BlockRelationshipField::Elements,
    )?;
    Ok(elem_ids)
}

/// Get the first block DTO.
fn get_first_block(db_context: &DbContext) -> Result<direct_access::block::dtos::BlockDto> {
    let root = root_controller::get(db_context, &1)?.expect("Root not found");
    let doc_ids = root_controller::get_relationship(
        db_context,
        &root.id,
        &RootRelationshipField::Document,
    )?;
    let frame_ids = document_controller::get_relationship(
        db_context,
        &doc_ids[0],
        &DocumentRelationshipField::Frames,
    )?;
    let block_ids = frame_controller::get_relationship(
        db_context,
        &frame_ids[0],
        &FrameRelationshipField::Blocks,
    )?;
    let block = block_controller::get(db_context, &block_ids[0])?.expect("Block not found");
    Ok(block)
}

// --- InsertFormattedText tests ---

#[test]
fn test_insert_formatted_text() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let result = document_editing_controller::insert_formatted_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertFormattedTextDto {
            position: 5,
            anchor: 5,
            text: " World".to_string(),
            font_family: "Arial".to_string(),
            font_point_size: 12,
            font_bold: true,
            font_italic: false,
            font_underline: false,
            font_strikeout: false,
        },
    )?;

    assert_eq!(result.new_position, 11);

    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello World");

    // Verify the new element has formatting
    let elem_ids = get_first_block_element_ids(&db_context)?;
    let mut found_bold = false;
    for elem_id in &elem_ids {
        let elem = inline_element_controller::get(&db_context, elem_id)?
            .expect("Element not found");
        if elem.fmt_font_bold == Some(true) {
            found_bold = true;
            assert_eq!(elem.fmt_font_family, Some("Arial".to_string()));
            assert_eq!(elem.fmt_font_point_size, Some(12));
        }
    }
    assert!(found_bold, "Should find an element with bold formatting");

    Ok(())
}

#[test]
fn test_insert_formatted_text_undo() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    document_editing_controller::insert_formatted_text(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertFormattedTextDto {
            position: 5,
            anchor: 5,
            text: " Bold".to_string(),
            font_family: "Arial".to_string(),
            font_point_size: 14,
            font_bold: true,
            font_italic: false,
            font_underline: false,
            font_strikeout: false,
        },
    )?;

    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello Bold");

    // Undo
    undo_redo_manager.undo(None)?;
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello");

    Ok(())
}

// --- InsertImage tests ---

#[test]
fn test_insert_image() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let result = document_editing_controller::insert_image(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertImageDto {
            position: 5,
            anchor: 5,
            image_name: "test.png".to_string(),
            width: 100,
            height: 50,
        },
    )?;

    // Image occupies 1 character position
    assert_eq!(result.new_position, 6);
    assert!(result.element_id > 0);

    // Verify document stats show increased character count
    use document_inspection::document_inspection_controller;
    let stats = document_inspection_controller::get_document_stats(&db_context, &event_hub)?;
    // Original "Hello" = 5 chars + 1 image = 6
    assert_eq!(stats.character_count, 6);

    Ok(())
}

#[test]
fn test_insert_image_undo() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    document_editing_controller::insert_image(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertImageDto {
            position: 3,
            anchor: 3,
            image_name: "photo.jpg".to_string(),
            width: 200,
            height: 150,
        },
    )?;

    use document_inspection::document_inspection_controller;
    let stats = document_inspection_controller::get_document_stats(&db_context, &event_hub)?;
    assert_eq!(stats.character_count, 6); // 5 + 1 image

    // Undo
    undo_redo_manager.undo(None)?;
    let stats = document_inspection_controller::get_document_stats(&db_context, &event_hub)?;
    assert_eq!(stats.character_count, 5); // back to original

    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello");

    Ok(())
}

// --- CreateList tests ---

#[test]
fn test_create_list() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Item one")?;

    let result = document_editing_controller::create_list(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &CreateListDto {
            position: 0,
            anchor: 8,
            style: ListStyle::Disc,
        },
    )?;

    assert!(result.list_id > 0);

    // Verify the block now has a list reference
    let block = get_first_block(&db_context)?;
    assert!(block.list.is_some(), "Block should have a list reference");
    assert_eq!(block.list.unwrap() as i64, result.list_id);

    Ok(())
}

#[test]
fn test_create_list_undo() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Item one")?;

    document_editing_controller::create_list(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &CreateListDto {
            position: 0,
            anchor: 8,
            style: ListStyle::Decimal,
        },
    )?;

    // Verify block has list
    let block = get_first_block(&db_context)?;
    assert!(block.list.is_some());

    // Undo
    undo_redo_manager.undo(None)?;

    let block = get_first_block(&db_context)?;
    assert!(block.list.is_none(), "Block should not have a list after undo");

    Ok(())
}

// --- InsertList tests ---

#[test]
fn test_insert_list() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let result = document_editing_controller::insert_list(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertListDto {
            position: 5,
            anchor: 5,
            style: ListStyle::Disc,
        },
    )?;

    assert!(result.list_id > 0);

    // Verify block count increased
    use document_inspection::document_inspection_controller;
    let stats = document_inspection_controller::get_document_stats(&db_context, &event_hub)?;
    assert_eq!(stats.block_count, 2);

    Ok(())
}

// --- InsertFrame tests ---

#[test]
fn test_insert_frame() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let result = document_editing_controller::insert_frame(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertFrameDto {
            position: 0,
            anchor: 0,
        },
    )?;

    assert!(result.frame_id > 0);

    // Verify the document now has 2 frames
    let root = root_controller::get(&db_context, &1)?.expect("Root not found");
    let doc_ids = root_controller::get_relationship(
        &db_context,
        &root.id,
        &RootRelationshipField::Document,
    )?;
    let frame_ids = document_controller::get_relationship(
        &db_context,
        &doc_ids[0],
        &DocumentRelationshipField::Frames,
    )?;
    assert_eq!(frame_ids.len(), 2);

    // Verify the new frame has parent_frame set to the root frame
    let new_frame = frame_controller::get(&db_context, &(result.frame_id as EntityId))?
        .expect("New frame not found");
    let root_frame_id = frame_ids[0]; // first frame is the root frame
    assert_eq!(
        new_frame.parent_frame,
        Some(root_frame_id),
        "New frame should have the root frame as parent"
    );

    // Verify the parent frame's child_order contains the new frame (as negative ID)
    let parent_frame = frame_controller::get(&db_context, &root_frame_id)?
        .expect("Parent frame not found");
    assert!(
        parent_frame.child_order.contains(&-(result.frame_id)),
        "Parent frame's child_order should contain -(new_frame_id)"
    );

    Ok(())
}

#[test]
fn test_insert_frame_undo() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    document_editing_controller::insert_frame(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertFrameDto {
            position: 0,
            anchor: 0,
        },
    )?;

    // Verify 2 frames
    let root = root_controller::get(&db_context, &1)?.expect("Root not found");
    let doc_ids = root_controller::get_relationship(
        &db_context,
        &root.id,
        &RootRelationshipField::Document,
    )?;
    let frame_ids = document_controller::get_relationship(
        &db_context,
        &doc_ids[0],
        &DocumentRelationshipField::Frames,
    )?;
    assert_eq!(frame_ids.len(), 2);

    // Undo
    undo_redo_manager.undo(None)?;

    let frame_ids = document_controller::get_relationship(
        &db_context,
        &doc_ids[0],
        &DocumentRelationshipField::Frames,
    )?;
    assert_eq!(frame_ids.len(), 1, "Should be back to 1 frame after undo");

    // Text should still be intact
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello");

    Ok(())
}
