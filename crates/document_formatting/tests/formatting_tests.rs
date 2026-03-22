extern crate text_document_formatting as document_formatting;
use anyhow::Result;
use common::database::db_context::DbContext;
use common::direct_access::block::BlockRelationshipField;
use common::direct_access::document::DocumentRelationshipField;
use common::direct_access::frame::FrameRelationshipField;
use common::direct_access::root::RootRelationshipField;
use common::event::EventHub;
use common::undo_redo::UndoRedoManager;
use std::sync::Arc;

use direct_access::document::document_controller;
use direct_access::document::dtos::CreateDocumentDto;
use direct_access::root::dtos::CreateRootDto;
use direct_access::root::root_controller;

use direct_access::block::block_controller;
use direct_access::frame::frame_controller;
use direct_access::inline_element::inline_element_controller;

use document_io::ImportPlainTextDto;
use document_io::document_io_controller;

use document_formatting::document_formatting_controller;
use document_formatting::{
    Alignment, CharVerticalAlignment, MarkerType, MergeTextFormatDto, SetBlockFormatDto,
    SetFrameFormatDto, SetTextFormatDto, UnderlineStyle,
};

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

/// Get the first block's ID from the document.
fn get_first_block_id(db_context: &DbContext) -> Result<common::types::EntityId> {
    let root = root_controller::get(db_context, &1)?.unwrap();
    let doc_ids =
        root_controller::get_relationship(db_context, &root.id, &RootRelationshipField::Document)?;
    let doc_id = doc_ids[0];
    let frame_ids = document_controller::get_relationship(
        db_context,
        &doc_id,
        &DocumentRelationshipField::Frames,
    )?;
    let frame_id = frame_ids[0];
    let block_ids =
        frame_controller::get_relationship(db_context, &frame_id, &FrameRelationshipField::Blocks)?;
    Ok(block_ids[0])
}

/// Get all block IDs from the document.
fn get_all_block_ids(db_context: &DbContext) -> Result<Vec<common::types::EntityId>> {
    let root = root_controller::get(db_context, &1)?.unwrap();
    let doc_ids =
        root_controller::get_relationship(db_context, &root.id, &RootRelationshipField::Document)?;
    let doc_id = doc_ids[0];
    let frame_ids = document_controller::get_relationship(
        db_context,
        &doc_id,
        &DocumentRelationshipField::Frames,
    )?;
    let frame_id = frame_ids[0];
    let block_ids =
        frame_controller::get_relationship(db_context, &frame_id, &FrameRelationshipField::Blocks)?;
    Ok(block_ids)
}

/// Get the root frame ID.
fn get_frame_id(db_context: &DbContext) -> Result<common::types::EntityId> {
    let root = root_controller::get(db_context, &1)?.unwrap();
    let doc_ids =
        root_controller::get_relationship(db_context, &root.id, &RootRelationshipField::Document)?;
    let doc_id = doc_ids[0];
    let frame_ids = document_controller::get_relationship(
        db_context,
        &doc_id,
        &DocumentRelationshipField::Frames,
    )?;
    Ok(frame_ids[0])
}

// ==================== SetBlockFormat tests ====================

#[test]
fn test_set_block_format_single_block() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello World")?;

    document_formatting_controller::set_block_format(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &SetBlockFormatDto {
            position: 0,
            anchor: 5,
            alignment: Some(Alignment::Center),
            heading_level: Some(2),
            indent: Some(1),
            marker: Some(MarkerType::Checked),
        },
    )?;

    let block_id = get_first_block_id(&db_context)?;
    let block = block_controller::get(&db_context, &block_id)?.unwrap();

    assert_eq!(
        block.fmt_alignment,
        Some(common::entities::Alignment::Center)
    );
    assert_eq!(block.fmt_heading_level, Some(2));
    assert_eq!(block.fmt_indent, Some(1));
    assert_eq!(
        block.fmt_marker,
        Some(common::entities::MarkerType::Checked)
    );

    Ok(())
}

#[test]
fn test_set_block_format_multiple_blocks() -> Result<()> {
    // "Hello\nWorld" creates two blocks separated by a block separator
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello\nWorld")?;

    // Format the entire range (position 0 to 11 covers both blocks)
    document_formatting_controller::set_block_format(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &SetBlockFormatDto {
            position: 0,
            anchor: 11,
            alignment: Some(Alignment::Right),
            heading_level: Some(1),
            indent: Some(3),
            marker: Some(MarkerType::Unchecked),
        },
    )?;

    let block_ids = get_all_block_ids(&db_context)?;
    assert!(block_ids.len() >= 2);

    for block_id in &block_ids {
        let block = block_controller::get(&db_context, block_id)?.unwrap();
        assert_eq!(
            block.fmt_alignment,
            Some(common::entities::Alignment::Right)
        );
        assert_eq!(block.fmt_heading_level, Some(1));
        assert_eq!(block.fmt_indent, Some(3));
        assert_eq!(
            block.fmt_marker,
            Some(common::entities::MarkerType::Unchecked)
        );
    }

    Ok(())
}

#[test]
fn test_set_block_format_undo() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello World")?;

    let block_id = get_first_block_id(&db_context)?;

    // Check original format (should be None)
    let block_before = block_controller::get(&db_context, &block_id)?.unwrap();
    assert_eq!(block_before.fmt_alignment, None);
    assert_eq!(block_before.fmt_heading_level, None);

    // Apply block format
    document_formatting_controller::set_block_format(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &SetBlockFormatDto {
            position: 0,
            anchor: 5,
            alignment: Some(Alignment::Justify),
            heading_level: Some(3),
            indent: Some(2),
            marker: Some(MarkerType::NoMarker),
        },
    )?;

    // Verify format was applied
    let block_after = block_controller::get(&db_context, &block_id)?.unwrap();
    assert_eq!(
        block_after.fmt_alignment,
        Some(common::entities::Alignment::Justify)
    );
    assert_eq!(block_after.fmt_heading_level, Some(3));

    // Undo
    undo_redo_manager.undo(None)?;

    // Verify format was reverted
    let block_undone = block_controller::get(&db_context, &block_id)?.unwrap();
    assert_eq!(block_undone.fmt_alignment, None);
    assert_eq!(block_undone.fmt_heading_level, None);

    Ok(())
}

// ==================== SetFrameFormat tests ====================

#[test]
fn test_set_frame_format() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let frame_id = get_frame_id(&db_context)?;

    document_formatting_controller::set_frame_format(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &SetFrameFormatDto {
            position: 0,
            anchor: 0,
            frame_id: frame_id as i64,
            height: Some(100),
            width: Some(200),
            top_margin: Some(10),
            bottom_margin: Some(20),
            left_margin: Some(30),
            right_margin: Some(40),
            padding: Some(5),
            border: Some(2),
        },
    )?;

    let frame = frame_controller::get(&db_context, &frame_id)?.unwrap();
    assert_eq!(frame.fmt_height, Some(100));
    assert_eq!(frame.fmt_width, Some(200));
    assert_eq!(frame.fmt_top_margin, Some(10));
    assert_eq!(frame.fmt_bottom_margin, Some(20));
    assert_eq!(frame.fmt_left_margin, Some(30));
    assert_eq!(frame.fmt_right_margin, Some(40));
    assert_eq!(frame.fmt_padding, Some(5));
    assert_eq!(frame.fmt_border, Some(2));

    Ok(())
}

#[test]
fn test_set_frame_format_undo() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let frame_id = get_frame_id(&db_context)?;

    // Verify original format
    let frame_before = frame_controller::get(&db_context, &frame_id)?.unwrap();
    assert_eq!(frame_before.fmt_height, None);

    // Apply frame format
    document_formatting_controller::set_frame_format(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &SetFrameFormatDto {
            position: 0,
            anchor: 0,
            frame_id: frame_id as i64,
            height: Some(500),
            width: Some(300),
            top_margin: Some(15),
            bottom_margin: Some(25),
            left_margin: Some(35),
            right_margin: Some(45),
            padding: Some(8),
            border: Some(3),
        },
    )?;

    // Verify applied
    let frame_after = frame_controller::get(&db_context, &frame_id)?.unwrap();
    assert_eq!(frame_after.fmt_height, Some(500));

    // Undo
    undo_redo_manager.undo(None)?;

    // Verify reverted
    let frame_undone = frame_controller::get(&db_context, &frame_id)?.unwrap();
    assert_eq!(frame_undone.fmt_height, None);

    Ok(())
}

// ==================== SetTextFormat tests ====================

#[test]
fn test_set_text_format_whole_element() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    // Format the entire text
    document_formatting_controller::set_text_format(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &SetTextFormatDto {
            position: 0,
            anchor: 5,
            font_family: Some("Arial".into()),
            font_point_size: Some(14),
            font_weight: Some(700),
            font_bold: Some(true),
            font_italic: Some(false),
            font_underline: Some(true),
            font_overline: Some(false),
            font_strikeout: Some(false),
            letter_spacing: Some(2),
            word_spacing: Some(4),
            underline_style: Some(UnderlineStyle::SingleUnderline),
            vertical_alignment: Some(CharVerticalAlignment::Normal),
        },
    )?;

    // Get the first block's elements
    let block_id = get_first_block_id(&db_context)?;
    let element_ids = block_controller::get_relationship(
        &db_context,
        &block_id,
        &BlockRelationshipField::Elements,
    )?;

    // Check the first element has formatting
    let elem = inline_element_controller::get(&db_context, &element_ids[0])?.unwrap();
    assert_eq!(elem.fmt_font_family, Some("Arial".to_string()));
    assert_eq!(elem.fmt_font_point_size, Some(14));
    assert_eq!(elem.fmt_font_weight, Some(700));
    assert_eq!(elem.fmt_font_bold, Some(true));
    assert_eq!(elem.fmt_font_underline, Some(true));
    assert_eq!(elem.fmt_letter_spacing, Some(2));
    assert_eq!(elem.fmt_word_spacing, Some(4));
    assert_eq!(
        elem.fmt_underline_style,
        Some(common::entities::UnderlineStyle::SingleUnderline)
    );

    Ok(())
}

#[test]
fn test_set_text_format_partial() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("HelloWorld")?;

    // Format only "lloWo" (positions 2..7) - this should split the element
    document_formatting_controller::set_text_format(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &SetTextFormatDto {
            position: 2,
            anchor: 7,
            font_family: Some("Courier".into()),
            font_point_size: Some(12),
            font_weight: Some(400),
            font_bold: Some(true),
            font_italic: Some(true),
            font_underline: Some(false),
            font_overline: Some(false),
            font_strikeout: Some(false),
            letter_spacing: Some(0),
            word_spacing: Some(0),
            underline_style: Some(UnderlineStyle::NoUnderline),
            vertical_alignment: Some(CharVerticalAlignment::Normal),
        },
    )?;

    // Get the block's elements - should have been split
    let block_id = get_first_block_id(&db_context)?;
    let element_ids = block_controller::get_relationship(
        &db_context,
        &block_id,
        &BlockRelationshipField::Elements,
    )?;

    // Should have 3 elements after splitting: "He", "lloWo", "rld"
    assert!(
        element_ids.len() >= 3,
        "Expected at least 3 elements after split, got {}",
        element_ids.len()
    );

    // Check that at least one element has the bold+italic formatting
    let mut found_formatted = false;
    for elem_id in &element_ids {
        let elem = inline_element_controller::get(&db_context, elem_id)?.unwrap();
        if elem.fmt_font_bold == Some(true) && elem.fmt_font_italic == Some(true) {
            assert_eq!(elem.fmt_font_family, Some("Courier".to_string()));
            found_formatted = true;
        }
    }
    assert!(
        found_formatted,
        "Should find at least one formatted element"
    );

    Ok(())
}

// ==================== MergeTextFormat tests ====================

#[test]
fn test_merge_text_format_preserves_other_fields() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    // First, apply a full text format
    document_formatting_controller::set_text_format(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &SetTextFormatDto {
            position: 0,
            anchor: 5,
            font_family: Some("Times".into()),
            font_point_size: Some(18),
            font_weight: Some(700),
            font_bold: Some(true),
            font_italic: Some(false),
            font_underline: Some(false),
            font_overline: Some(true),
            font_strikeout: Some(true),
            letter_spacing: Some(5),
            word_spacing: Some(10),
            underline_style: Some(UnderlineStyle::WaveUnderline),
            vertical_alignment: Some(CharVerticalAlignment::SuperScript),
        },
    )?;

    // Now merge - only change font_bold, font_italic, font_underline
    document_formatting_controller::merge_text_format(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &MergeTextFormatDto {
            position: 0,
            anchor: 5,
            font_family: None, // None = don't change
            font_bold: Some(false),
            font_italic: Some(true),
            font_underline: Some(true),
        },
    )?;

    // Verify the merge results
    let block_id = get_first_block_id(&db_context)?;
    let element_ids = block_controller::get_relationship(
        &db_context,
        &block_id,
        &BlockRelationshipField::Elements,
    )?;

    let elem = inline_element_controller::get(&db_context, &element_ids[0])?.unwrap();

    // Merged fields should be updated
    assert_eq!(elem.fmt_font_bold, Some(false));
    assert_eq!(elem.fmt_font_italic, Some(true));
    assert_eq!(elem.fmt_font_underline, Some(true));

    // Font family should be preserved (empty string means don't change)
    assert_eq!(elem.fmt_font_family, Some("Times".to_string()));

    // Other fields should be preserved from set_text_format
    assert_eq!(elem.fmt_font_point_size, Some(18));
    assert_eq!(elem.fmt_font_weight, Some(700));
    assert_eq!(elem.fmt_font_overline, Some(true));
    assert_eq!(elem.fmt_font_strikeout, Some(true));
    assert_eq!(elem.fmt_letter_spacing, Some(5));
    assert_eq!(elem.fmt_word_spacing, Some(10));
    assert_eq!(
        elem.fmt_underline_style,
        Some(common::entities::UnderlineStyle::WaveUnderline)
    );
    assert_eq!(
        elem.fmt_vertical_alignment,
        Some(common::entities::CharVerticalAlignment::SuperScript)
    );

    Ok(())
}

#[test]
fn test_merge_text_format_undo() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    // Apply initial formatting
    document_formatting_controller::set_text_format(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &SetTextFormatDto {
            position: 0,
            anchor: 5,
            font_family: Some("Helvetica".into()),
            font_point_size: Some(16),
            font_weight: Some(400),
            font_bold: Some(false),
            font_italic: Some(false),
            font_underline: Some(false),
            font_overline: Some(false),
            font_strikeout: Some(false),
            letter_spacing: Some(0),
            word_spacing: Some(0),
            underline_style: Some(UnderlineStyle::NoUnderline),
            vertical_alignment: Some(CharVerticalAlignment::Normal),
        },
    )?;

    // Now merge bold+italic
    document_formatting_controller::merge_text_format(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &MergeTextFormatDto {
            position: 0,
            anchor: 5,
            font_family: None,
            font_bold: Some(true),
            font_italic: Some(true),
            font_underline: Some(false),
        },
    )?;

    // Verify merge was applied
    let block_id = get_first_block_id(&db_context)?;
    let element_ids = block_controller::get_relationship(
        &db_context,
        &block_id,
        &BlockRelationshipField::Elements,
    )?;
    let elem = inline_element_controller::get(&db_context, &element_ids[0])?.unwrap();
    assert_eq!(elem.fmt_font_bold, Some(true));
    assert_eq!(elem.fmt_font_italic, Some(true));

    // Undo the merge
    undo_redo_manager.undo(None)?;

    // Verify merge was reverted (back to set_text_format state)
    let element_ids_after = block_controller::get_relationship(
        &db_context,
        &block_id,
        &BlockRelationshipField::Elements,
    )?;
    let elem_after = inline_element_controller::get(&db_context, &element_ids_after[0])?.unwrap();
    assert_eq!(elem_after.fmt_font_bold, Some(false));
    assert_eq!(elem_after.fmt_font_italic, Some(false));
    // Font family should still be from the set_text_format call
    assert_eq!(elem_after.fmt_font_family, Some("Helvetica".to_string()));

    Ok(())
}

#[test]
fn test_set_text_format_undo() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let block_id = get_first_block_id(&db_context)?;
    let elem_ids_before = block_controller::get_relationship(
        &db_context,
        &block_id,
        &BlockRelationshipField::Elements,
    )?;
    let elem_before = inline_element_controller::get(&db_context, &elem_ids_before[0])?.unwrap();
    assert_eq!(elem_before.fmt_font_bold, None);

    document_formatting_controller::set_text_format(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &SetTextFormatDto {
            position: 0,
            anchor: 5,
            font_family: Some("Mono".into()),
            font_point_size: Some(10),
            font_weight: Some(400),
            font_bold: Some(true),
            font_italic: Some(false),
            font_underline: Some(false),
            font_overline: Some(false),
            font_strikeout: Some(false),
            letter_spacing: Some(0),
            word_spacing: Some(0),
            underline_style: Some(UnderlineStyle::NoUnderline),
            vertical_alignment: Some(CharVerticalAlignment::Normal),
        },
    )?;

    let elem_ids = block_controller::get_relationship(
        &db_context,
        &block_id,
        &BlockRelationshipField::Elements,
    )?;
    let elem = inline_element_controller::get(&db_context, &elem_ids[0])?.unwrap();
    assert_eq!(elem.fmt_font_bold, Some(true));

    undo_redo_manager.undo(None)?;

    let elem_ids_after = block_controller::get_relationship(
        &db_context,
        &block_id,
        &BlockRelationshipField::Elements,
    )?;
    let elem_after = inline_element_controller::get(&db_context, &elem_ids_after[0])?.unwrap();
    assert_eq!(elem_after.fmt_font_bold, None);

    Ok(())
}

#[test]
fn test_set_text_format_cross_block() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello\nWorld")?;

    // Format range spanning both blocks (0..11)
    document_formatting_controller::set_text_format(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &SetTextFormatDto {
            position: 0,
            anchor: 11,
            font_family: Some("Georgia".into()),
            font_point_size: Some(16),
            font_weight: Some(700),
            font_bold: Some(true),
            font_italic: Some(true),
            font_underline: Some(false),
            font_overline: Some(false),
            font_strikeout: Some(false),
            letter_spacing: Some(0),
            word_spacing: Some(0),
            underline_style: Some(UnderlineStyle::NoUnderline),
            vertical_alignment: Some(CharVerticalAlignment::Normal),
        },
    )?;

    let block_ids = get_all_block_ids(&db_context)?;
    for block_id in &block_ids {
        let elem_ids = block_controller::get_relationship(
            &db_context,
            block_id,
            &BlockRelationshipField::Elements,
        )?;
        for elem_id in &elem_ids {
            let elem = inline_element_controller::get(&db_context, elem_id)?.unwrap();
            if let common::entities::InlineContent::Text(ref t) = elem.content {
                if !t.is_empty() {
                    assert_eq!(
                        elem.fmt_font_bold,
                        Some(true),
                        "Element in block {:?} should be bold",
                        block_id
                    );
                    assert_eq!(elem.fmt_font_italic, Some(true));
                }
            }
        }
    }

    Ok(())
}

#[test]
fn test_set_block_format_empty_range() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    // position == anchor: should still format the block containing that position
    document_formatting_controller::set_block_format(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &SetBlockFormatDto {
            position: 2,
            anchor: 2,
            alignment: Some(Alignment::Center),
            heading_level: Some(1),
            indent: Some(0),
            marker: Some(MarkerType::NoMarker),
        },
    )?;

    let block_id = get_first_block_id(&db_context)?;
    let block = block_controller::get(&db_context, &block_id)?.unwrap();
    assert_eq!(
        block.fmt_alignment,
        Some(common::entities::Alignment::Center)
    );
    assert_eq!(block.fmt_heading_level, Some(1));

    Ok(())
}
