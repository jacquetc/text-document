use anyhow::Result;
use common::database::db_context::DbContext;
use common::direct_access::block::block_repository::BlockRelationshipField;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::event::EventHub;
use common::parser_tools::fragment_schema::FragmentData;
use common::types::EntityId;
use common::undo_redo::UndoRedoManager;
use std::sync::Arc;

use direct_access::block::block_controller;
use direct_access::document::dtos::CreateDocumentDto;
use direct_access::document::document_controller;
use direct_access::frame::frame_controller;
use direct_access::inline_element::inline_element_controller;
use direct_access::root::dtos::CreateRootDto;
use direct_access::root::root_controller;

use document_editing::document_editing_controller;
use document_editing::InsertFragmentDto;

use document_inspection::document_inspection_controller;
use document_inspection::ExtractFragmentDto;

use document_io::document_io_controller;
use document_io::ImportPlainTextDto;

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

/// Get the first frame's block IDs.
fn get_block_ids(db_context: &DbContext) -> Result<Vec<EntityId>> {
    let root_rels =
        root_controller::get_relationship(db_context, &1, &RootRelationshipField::Document)?;
    let doc_id = root_rels[0];
    let frame_ids = document_controller::get_relationship(
        db_context,
        &doc_id,
        &DocumentRelationshipField::Frames,
    )?;
    let frame_id = frame_ids[0];
    let block_ids = frame_controller::get_relationship(
        db_context,
        &frame_id,
        &FrameRelationshipField::Blocks,
    )?;
    Ok(block_ids)
}

// ─── Extract Fragment Tests ──────────────────────────────────────────

#[test]
fn test_extract_fragment_full_block() -> Result<()> {
    let (db_context, event_hub, _) = setup_with_text("Hello World")?;

    let result = document_inspection_controller::extract_fragment(
        &db_context,
        &event_hub,
        &ExtractFragmentDto {
            position: 0,
            anchor: 11,
        },
    )?;

    assert_eq!(result.plain_text, "Hello World");

    // Verify fragment_data is valid JSON
    let fragment: FragmentData = serde_json::from_str(&result.fragment_data)?;
    assert_eq!(fragment.blocks.len(), 1);
    assert_eq!(fragment.blocks[0].plain_text, "Hello World");
    assert!(!fragment.blocks[0].elements.is_empty());

    Ok(())
}

#[test]
fn test_extract_fragment_partial_block() -> Result<()> {
    let (db_context, event_hub, _) = setup_with_text("Hello World")?;

    let result = document_inspection_controller::extract_fragment(
        &db_context,
        &event_hub,
        &ExtractFragmentDto {
            position: 2,
            anchor: 7,
        },
    )?;

    assert_eq!(result.plain_text, "llo W");

    let fragment: FragmentData = serde_json::from_str(&result.fragment_data)?;
    assert_eq!(fragment.blocks.len(), 1);
    assert_eq!(fragment.blocks[0].plain_text, "llo W");

    Ok(())
}

#[test]
fn test_extract_fragment_cross_block() -> Result<()> {
    // "First\nSecond\nThird"
    // Block 0: "First" pos 0-4
    // Block 1: "Second" pos 6-11
    // Block 2: "Third" pos 13-17
    let (db_context, event_hub, _) = setup_with_text("First\nSecond\nThird")?;

    // "First" pos 0..5, "Second" pos 6..12, "Third" pos 13..18
    // Positions are half-open: anchor=18 means up to but not including position 18
    let result = document_inspection_controller::extract_fragment(
        &db_context,
        &event_hub,
        &ExtractFragmentDto {
            position: 0,
            anchor: 18,
        },
    )?;

    let fragment: FragmentData = serde_json::from_str(&result.fragment_data)?;
    assert_eq!(fragment.blocks.len(), 3);
    assert_eq!(fragment.blocks[0].plain_text, "First");
    assert_eq!(fragment.blocks[1].plain_text, "Second");
    assert_eq!(fragment.blocks[2].plain_text, "Third");

    // plain_text should join blocks with newline
    assert_eq!(result.plain_text, "First\nSecond\nThird");

    Ok(())
}

#[test]
fn test_extract_fragment_empty_range() -> Result<()> {
    let (db_context, event_hub, _) = setup_with_text("Hello World")?;

    let result = document_inspection_controller::extract_fragment(
        &db_context,
        &event_hub,
        &ExtractFragmentDto {
            position: 5,
            anchor: 5,
        },
    )?;

    assert_eq!(result.plain_text, "");

    let fragment: FragmentData = serde_json::from_str(&result.fragment_data)?;
    assert!(fragment.blocks.is_empty());

    Ok(())
}

// ─── Insert Fragment (via extract roundtrip) Tests ──────────────────

#[test]
fn test_insert_fragment_roundtrip() -> Result<()> {
    // Set up source document
    let (db_context, event_hub, mut undo_redo_manager) =
        setup_with_text("Hello World")?;

    // Extract "World" (positions 6-11)
    let extract_result = document_inspection_controller::extract_fragment(
        &db_context,
        &event_hub,
        &ExtractFragmentDto {
            position: 6,
            anchor: 11,
        },
    )?;

    assert_eq!(extract_result.plain_text, "World");

    // Now set up a second document (reuse same db)
    // Insert at end of current document
    let insert_result = document_editing_controller::insert_fragment(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertFragmentDto {
            position: 11,
            anchor: 11,
            fragment_data: extract_result.fragment_data,
        },
    )?;

    assert!(insert_result.blocks_added >= 1);

    let text = export_text(&db_context, &event_hub)?;
    assert!(text.contains("World"), "Inserted text should contain 'World', got: {}", text);

    Ok(())
}

#[test]
fn test_insert_fragment_preserves_formatting() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Start")?;

    // Insert markdown with bold text
    document_editing_controller::insert_markdown_at_position(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &document_editing::InsertMarkdownAtPositionDto {
            position: 5,
            anchor: 5,
            markdown: "**bold text**".to_string(),
        },
    )?;

    // Find the position range containing "bold text"
    let text = export_text(&db_context, &event_hub)?;
    // text is now something like "Start\nbold text\n"
    // Find where "bold text" starts
    let bold_start = text.find("bold text").expect("should contain 'bold text'");
    let bold_end = bold_start + "bold text".len();

    // Extract the bold fragment
    let extract_result = document_inspection_controller::extract_fragment(
        &db_context,
        &event_hub,
        &ExtractFragmentDto {
            position: bold_start as i64,
            anchor: bold_end as i64,
        },
    )?;

    assert_eq!(extract_result.plain_text, "bold text");

    // Verify the fragment contains bold formatting
    let fragment: FragmentData = serde_json::from_str(&extract_result.fragment_data)?;
    let has_bold = fragment.blocks.iter().any(|b| {
        b.elements.iter().any(|e| e.fmt_font_bold == Some(true))
    });
    assert!(has_bold, "Extracted fragment should contain bold formatting");

    // Now insert this bold fragment at the end of the document
    let current_text = export_text(&db_context, &event_hub)?;
    let _end_pos = current_text.len();
    // Account for block separators in position calculation
    // Get last block position
    let block_ids = get_block_ids(&db_context)?;
    let last_block = block_controller::get(&db_context, block_ids.last().unwrap())?
        .expect("last block should exist");
    let insert_pos = last_block.document_position + last_block.text_length;

    document_editing_controller::insert_fragment(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertFragmentDto {
            position: insert_pos,
            anchor: insert_pos,
            fragment_data: extract_result.fragment_data,
        },
    )?;

    // Verify bold formatting is preserved in the newly inserted elements
    let block_ids = get_block_ids(&db_context)?;
    let mut bold_count = 0;
    for block_id in &block_ids {
        let elem_ids = block_controller::get_relationship(
            &db_context,
            block_id,
            &BlockRelationshipField::Elements,
        )?;
        for elem_id in &elem_ids {
            let elem = inline_element_controller::get(&db_context, elem_id)?;
            if let Some(elem) = elem {
                if let common::entities::InlineContent::Text(ref t) = elem.content {
                    if t.contains("bold") && elem.fmt_font_bold == Some(true) {
                        bold_count += 1;
                    }
                }
            }
        }
    }
    // Should have at least 2 bold elements (original + inserted copy)
    assert!(bold_count >= 2, "Should have at least 2 bold elements (original + copy), got {}", bold_count);

    Ok(())
}

#[test]
fn test_insert_fragment_undo() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let original_text = export_text(&db_context, &event_hub)?;

    // Extract "Hello" and re-insert at end
    let extract_result = document_inspection_controller::extract_fragment(
        &db_context,
        &event_hub,
        &ExtractFragmentDto {
            position: 0,
            anchor: 5,
        },
    )?;

    document_editing_controller::insert_fragment(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertFragmentDto {
            position: 5,
            anchor: 5,
            fragment_data: extract_result.fragment_data,
        },
    )?;

    let after_insert = export_text(&db_context, &event_hub)?;
    assert_ne!(after_insert, original_text, "Text should have changed after insert");

    // Undo
    undo_redo_manager.undo(None)?;

    let after_undo = export_text(&db_context, &event_hub)?;
    assert_eq!(after_undo, original_text, "Text should be restored after undo");

    Ok(())
}

#[test]
fn test_extract_insert_with_list() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Start")?;

    // Insert markdown with list
    document_editing_controller::insert_markdown_at_position(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &document_editing::InsertMarkdownAtPositionDto {
            position: 5,
            anchor: 5,
            markdown: "- item1\n- item2".to_string(),
        },
    )?;

    // Find blocks with lists
    let block_ids = get_block_ids(&db_context)?;
    let mut list_block_positions: Vec<(i64, i64)> = Vec::new();
    for block_id in &block_ids {
        let block = block_controller::get(&db_context, block_id)?;
        if let Some(block) = block {
            if block.list.is_some() {
                list_block_positions.push((block.document_position, block.text_length));
            }
        }
    }
    assert!(list_block_positions.len() >= 2, "Should have at least 2 list blocks");

    // Extract the range covering the list items
    let first_pos = list_block_positions[0].0;
    let last = list_block_positions.last().unwrap();
    let end_pos = last.0 + last.1;

    let extract_result = document_inspection_controller::extract_fragment(
        &db_context,
        &event_hub,
        &ExtractFragmentDto {
            position: first_pos,
            anchor: end_pos,
        },
    )?;

    // Verify the fragment contains list data
    let fragment: FragmentData = serde_json::from_str(&extract_result.fragment_data)?;
    let has_list = fragment.blocks.iter().any(|b| b.list.is_some());
    assert!(has_list, "Extracted fragment should contain list data");

    // Insert the fragment at the very end
    let all_block_ids = get_block_ids(&db_context)?;
    let last_block = block_controller::get(&db_context, all_block_ids.last().unwrap())?
        .expect("last block");
    let insert_pos = last_block.document_position + last_block.text_length;

    document_editing_controller::insert_fragment(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertFragmentDto {
            position: insert_pos,
            anchor: insert_pos,
            fragment_data: extract_result.fragment_data,
        },
    )?;

    // Verify that the inserted blocks also have list associations
    let final_block_ids = get_block_ids(&db_context)?;
    let mut final_list_count = 0;
    for block_id in &final_block_ids {
        let block = block_controller::get(&db_context, block_id)?;
        if let Some(block) = block {
            if block.list.is_some() {
                final_list_count += 1;
            }
        }
    }
    // Should have more list blocks now (original + inserted copies)
    assert!(
        final_list_count >= list_block_positions.len() * 2,
        "Should have at least {} list blocks (original + copies), got {}",
        list_block_positions.len() * 2,
        final_list_count
    );

    Ok(())
}

#[test]
fn test_insert_empty_fragment_should_error() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let empty_fragment = serde_json::json!({ "blocks": [] }).to_string();

    let result = document_editing_controller::insert_fragment(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertFragmentDto {
            position: 0,
            anchor: 0,
            fragment_data: empty_fragment,
        },
    );

    assert!(result.is_err(), "Inserting an empty fragment should fail");

    // Document should be unchanged
    let text = export_text(&db_context, &event_hub)?;
    assert_eq!(text, "Hello");

    Ok(())
}

#[test]
fn test_extract_insert_fragment_with_image() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Before")?;

    // Insert an image into the document
    use document_editing::InsertImageDto;
    document_editing_controller::insert_image(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertImageDto {
            position: 6,
            anchor: 6,
            image_name: "photo.png".to_string(),
            width: 200,
            height: 100,
        },
    )?;

    // Extract fragment containing the image (position 5..7 spans text end + image)
    let extract_result = document_inspection_controller::extract_fragment(
        &db_context,
        &event_hub,
        &ExtractFragmentDto {
            position: 5,
            anchor: 7,
        },
    )?;

    // Verify fragment_data contains image
    let fragment: common::parser_tools::fragment_schema::FragmentData =
        serde_json::from_str(&extract_result.fragment_data)?;
    let has_image = fragment.blocks.iter().any(|b| {
        b.elements.iter().any(|e| {
            matches!(e.content, common::entities::InlineContent::Image { .. })
        })
    });
    assert!(has_image, "Fragment should contain an image element");

    // Insert the extracted fragment elsewhere
    let result = document_editing_controller::insert_fragment(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertFragmentDto {
            position: 0,
            anchor: 0,
            fragment_data: extract_result.fragment_data,
        },
    )?;

    assert!(result.blocks_added >= 1);

    Ok(())
}

#[test]
fn test_extract_fragment_multiple_formats() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    // Apply bold to "Hel" and italic to "lo"
    use document_formatting::document_formatting_controller;
    use document_formatting::{SetTextFormatDto, UnderlineStyle, CharVerticalAlignment};

    document_formatting_controller::set_text_format(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &SetTextFormatDto {
            position: 0,
            anchor: 3,
            font_family: "Arial".to_string(),
            font_point_size: 12,
            font_weight: 700,
            font_bold: true,
            font_italic: false,
            font_underline: false,
            font_overline: false,
            font_strikeout: false,
            letter_spacing: 0,
            word_spacing: 0,
            underline_style: UnderlineStyle::NoUnderline,
            vertical_alignment: CharVerticalAlignment::Normal,
        },
    )?;

    document_formatting_controller::set_text_format(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &SetTextFormatDto {
            position: 3,
            anchor: 5,
            font_family: "Times".to_string(),
            font_point_size: 14,
            font_weight: 400,
            font_bold: false,
            font_italic: true,
            font_underline: false,
            font_overline: false,
            font_strikeout: false,
            letter_spacing: 0,
            word_spacing: 0,
            underline_style: UnderlineStyle::NoUnderline,
            vertical_alignment: CharVerticalAlignment::Normal,
        },
    )?;

    // Extract the full block
    let extract_result = document_inspection_controller::extract_fragment(
        &db_context,
        &event_hub,
        &ExtractFragmentDto {
            position: 0,
            anchor: 5,
        },
    )?;

    // Verify fragment has multiple elements with different formats
    let fragment: common::parser_tools::fragment_schema::FragmentData =
        serde_json::from_str(&extract_result.fragment_data)?;
    assert!(!fragment.blocks.is_empty());
    let block = &fragment.blocks[0];

    let bold_count = block
        .elements
        .iter()
        .filter(|e| e.fmt_font_bold == Some(true))
        .count();
    let italic_count = block
        .elements
        .iter()
        .filter(|e| e.fmt_font_italic == Some(true))
        .count();

    assert!(bold_count >= 1, "Should have at least one bold element");
    assert!(italic_count >= 1, "Should have at least one italic element");

    // Roundtrip: insert elsewhere and verify formats preserved
    let (db2, eh2, mut urm2) = setup_with_text("Target")?;

    document_editing_controller::insert_fragment(
        &db2,
        &eh2,
        &mut urm2,
        None,
        &InsertFragmentDto {
            position: 6,
            anchor: 6,
            fragment_data: extract_result.fragment_data,
        },
    )?;

    // Verify bold and italic elements exist in target
    let block_ids = get_block_ids(&db2)?;
    let mut found_bold = false;
    let mut found_italic = false;
    for block_id in &block_ids {
        let elem_ids = block_controller::get_relationship(
            &db2,
            block_id,
            &BlockRelationshipField::Elements,
        )?;
        for elem_id in &elem_ids {
            let elem = inline_element_controller::get(&db2, elem_id)?.unwrap();
            if elem.fmt_font_bold == Some(true) {
                found_bold = true;
            }
            if elem.fmt_font_italic == Some(true) {
                found_italic = true;
            }
        }
    }
    assert!(found_bold, "Target should have bold element after fragment insert");
    assert!(found_italic, "Target should have italic element after fragment insert");

    Ok(())
}
