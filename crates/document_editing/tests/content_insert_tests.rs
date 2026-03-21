extern crate text_document_editing as document_editing;
use anyhow::Result;
use common::database::db_context::DbContext;
use common::direct_access::block::block_repository::BlockRelationshipField;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::event::EventHub;
use common::types::EntityId;
use common::undo_redo::UndoRedoManager;
use std::sync::Arc;

use direct_access::block::block_controller;
use direct_access::document::document_controller;
use direct_access::document::dtos::CreateDocumentDto;
use direct_access::frame::frame_controller;
use direct_access::inline_element::inline_element_controller;
use direct_access::root::dtos::CreateRootDto;
use direct_access::root::root_controller;

use document_editing::document_editing_controller;
use document_editing::{InsertFragmentDto, InsertHtmlAtPositionDto, InsertMarkdownAtPositionDto};
use document_io::ImportPlainTextDto;
use document_io::document_io_controller;

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
    let block_ids =
        frame_controller::get_relationship(db_context, &frame_id, &FrameRelationshipField::Blocks)?;
    Ok(block_ids)
}

/// Get element IDs for a block.
fn get_element_ids(db_context: &DbContext, block_id: &EntityId) -> Result<Vec<EntityId>> {
    block_controller::get_relationship(db_context, block_id, &BlockRelationshipField::Elements)
}

// ─── Markdown tests ──────────────────────────────────────────────────

#[test]
fn test_insert_markdown_simple_paragraph() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let result = document_editing_controller::insert_markdown_at_position(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertMarkdownAtPositionDto {
            position: 5,
            anchor: 5,
            markdown: "Hello **world**".to_string(),
        },
    )?;

    assert!(result.blocks_added >= 1);

    // Verify the text was inserted
    let text = export_text(&db_context, &event_hub)?;
    assert!(text.contains("world"));

    // Verify bold formatting on the "world" span
    let block_ids = get_block_ids(&db_context)?;
    let mut found_bold = false;
    for block_id in &block_ids {
        let elem_ids = get_element_ids(&db_context, block_id)?;
        for elem_id in &elem_ids {
            let elem = inline_element_controller::get(&db_context, elem_id)?;
            if let Some(elem) = elem {
                if let common::entities::InlineContent::Text(ref t) = elem.content {
                    if t == "world" {
                        assert_eq!(elem.fmt_font_bold, Some(true));
                        found_bold = true;
                    }
                }
            }
        }
    }
    assert!(found_bold, "Should find a bold 'world' element");

    Ok(())
}

#[test]
fn test_insert_markdown_multiple_paragraphs() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Start")?;

    let result = document_editing_controller::insert_markdown_at_position(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertMarkdownAtPositionDto {
            position: 5,
            anchor: 5,
            markdown: "Para 1\n\nPara 2".to_string(),
        },
    )?;

    assert!(result.blocks_added >= 2, "Should add at least 2 blocks");

    let text = export_text(&db_context, &event_hub)?;
    assert!(text.contains("Para 1"));
    assert!(text.contains("Para 2"));

    Ok(())
}

#[test]
fn test_insert_markdown_heading() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Text")?;

    let _result = document_editing_controller::insert_markdown_at_position(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertMarkdownAtPositionDto {
            position: 4,
            anchor: 4,
            markdown: "# Title".to_string(),
        },
    )?;

    // Verify heading_level=1 on the inserted block
    let block_ids = get_block_ids(&db_context)?;
    let mut found_heading = false;
    for block_id in &block_ids {
        let block = block_controller::get(&db_context, block_id)?;
        if let Some(block) = block {
            if block.fmt_heading_level == Some(1) {
                found_heading = true;
                break;
            }
        }
    }
    assert!(found_heading, "Should find a block with heading_level=1");

    Ok(())
}

#[test]
fn test_insert_markdown_list() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Text")?;

    let _result = document_editing_controller::insert_markdown_at_position(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertMarkdownAtPositionDto {
            position: 4,
            anchor: 4,
            markdown: "- item1\n- item2".to_string(),
        },
    )?;

    // Verify list entities were created by checking block list relationships
    let block_ids = get_block_ids(&db_context)?;
    let mut list_blocks = 0;
    for block_id in &block_ids {
        let block = block_controller::get(&db_context, block_id)?;
        if let Some(block) = block {
            if block.list.is_some() {
                list_blocks += 1;
            }
        }
    }
    assert!(
        list_blocks >= 2,
        "Should have at least 2 blocks with list associations"
    );

    Ok(())
}

#[test]
fn test_insert_markdown_undo() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let original_text = export_text(&db_context, &event_hub)?;

    let _result = document_editing_controller::insert_markdown_at_position(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertMarkdownAtPositionDto {
            position: 5,
            anchor: 5,
            markdown: "**bold text**".to_string(),
        },
    )?;

    let after_insert = export_text(&db_context, &event_hub)?;
    assert_ne!(after_insert, original_text);

    // Undo
    undo_redo_manager.undo(None)?;

    let after_undo = export_text(&db_context, &event_hub)?;
    assert_eq!(after_undo, original_text);

    Ok(())
}

// ─── HTML tests ──────────────────────────────────────────────────────

#[test]
fn test_insert_html_simple() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let result = document_editing_controller::insert_html_at_position(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertHtmlAtPositionDto {
            position: 5,
            anchor: 5,
            html: "<p>Hello <b>world</b></p>".to_string(),
        },
    )?;

    assert!(result.blocks_added >= 1);

    let text = export_text(&db_context, &event_hub)?;
    assert!(text.contains("world"));

    // Verify bold formatting
    let block_ids = get_block_ids(&db_context)?;
    let mut found_bold = false;
    for block_id in &block_ids {
        let elem_ids = get_element_ids(&db_context, block_id)?;
        for elem_id in &elem_ids {
            let elem = inline_element_controller::get(&db_context, elem_id)?;
            if let Some(elem) = elem {
                if let common::entities::InlineContent::Text(ref t) = elem.content {
                    if t == "world" {
                        assert_eq!(elem.fmt_font_bold, Some(true));
                        found_bold = true;
                    }
                }
            }
        }
    }
    assert!(found_bold, "Should find a bold 'world' element");

    Ok(())
}

#[test]
fn test_insert_html_multiple_paragraphs() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Start")?;

    let result = document_editing_controller::insert_html_at_position(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertHtmlAtPositionDto {
            position: 5,
            anchor: 5,
            html: "<p>A</p><p>B</p>".to_string(),
        },
    )?;

    assert!(result.blocks_added >= 2, "Should add at least 2 blocks");

    let text = export_text(&db_context, &event_hub)?;
    assert!(text.contains("A"));
    assert!(text.contains("B"));

    Ok(())
}

#[test]
fn test_insert_html_undo() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let original_text = export_text(&db_context, &event_hub)?;

    let _result = document_editing_controller::insert_html_at_position(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertHtmlAtPositionDto {
            position: 5,
            anchor: 5,
            html: "<p><b>bold</b></p>".to_string(),
        },
    )?;

    let after_insert = export_text(&db_context, &event_hub)?;
    assert_ne!(after_insert, original_text);

    // Undo
    undo_redo_manager.undo(None)?;

    let after_undo = export_text(&db_context, &event_hub)?;
    assert_eq!(after_undo, original_text);

    Ok(())
}

// ─── Fragment tests ──────────────────────────────────────────────────

fn make_bold_fragment(text: &str) -> String {
    serde_json::json!({
        "blocks": [{
            "plain_text": text,
            "elements": [{
                "content": {"Text": text},
                "fmt_font_family": null,
                "fmt_font_point_size": null,
                "fmt_font_weight": null,
                "fmt_font_bold": true,
                "fmt_font_italic": null,
                "fmt_font_underline": null,
                "fmt_font_overline": null,
                "fmt_font_strikeout": null,
                "fmt_letter_spacing": null,
                "fmt_word_spacing": null,
                "fmt_anchor_href": null,
                "fmt_anchor_names": [],
                "fmt_is_anchor": null,
                "fmt_tooltip": null,
                "fmt_underline_style": null,
                "fmt_vertical_alignment": null
            }],
            "heading_level": null,
            "list": null,
            "alignment": null,
            "indent": null,
            "text_indent": null,
            "marker": null,
            "top_margin": null,
            "bottom_margin": null,
            "left_margin": null,
            "right_margin": null,
            "tab_positions": []
        }]
    })
    .to_string()
}

#[test]
fn test_insert_fragment_simple() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let fragment_json = make_bold_fragment("bold text");

    let result = document_editing_controller::insert_fragment(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertFragmentDto {
            position: 5,
            anchor: 5,
            fragment_data: fragment_json,
        },
    )?;

    assert!(result.blocks_added >= 1);

    let block_ids = get_block_ids(&db_context)?;
    let mut found_bold = false;
    for block_id in &block_ids {
        let elem_ids = block_controller::get_relationship(
            &db_context,
            block_id,
            &BlockRelationshipField::Elements,
        )?;
        for elem_id in &elem_ids {
            let elem = inline_element_controller::get(&db_context, elem_id)?.unwrap();
            if let common::entities::InlineContent::Text(ref t) = elem.content {
                if t.contains("bold text") && elem.fmt_font_bold == Some(true) {
                    found_bold = true;
                }
            }
        }
    }
    assert!(found_bold, "Should find a bold 'bold text' element");

    Ok(())
}

#[test]
fn test_insert_fragment_undo() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Hello")?;

    let original_text = export_text(&db_context, &event_hub)?;

    let fragment_json = make_bold_fragment("bold text");

    let _result = document_editing_controller::insert_fragment(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertFragmentDto {
            position: 5,
            anchor: 5,
            fragment_data: fragment_json,
        },
    )?;

    let after_insert = export_text(&db_context, &event_hub)?;
    assert_ne!(after_insert, original_text);

    undo_redo_manager.undo(None)?;

    let after_undo = export_text(&db_context, &event_hub)?;
    assert_eq!(after_undo, original_text);

    Ok(())
}

// --- Additional coverage ---

#[test]
fn test_insert_markdown_code_block() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Before")?;

    let result = document_editing_controller::insert_markdown_at_position(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertMarkdownAtPositionDto {
            position: 6,
            anchor: 6,
            markdown: "```\nfn main() {}\n```".to_string(),
        },
    )?;

    assert!(result.blocks_added >= 1);

    let block_ids = get_block_ids(&db_context)?;
    let mut found_code = false;
    for block_id in &block_ids {
        let elem_ids = block_controller::get_relationship(
            &db_context,
            block_id,
            &BlockRelationshipField::Elements,
        )?;
        for elem_id in &elem_ids {
            let elem = inline_element_controller::get(&db_context, elem_id)?.unwrap();
            if let common::entities::InlineContent::Text(ref t) = elem.content {
                if t.contains("fn main") {
                    assert_eq!(elem.fmt_font_family, Some("monospace".to_string()));
                    found_code = true;
                }
            }
        }
    }
    assert!(found_code, "Should find a code-formatted element");

    Ok(())
}

#[test]
fn test_insert_markdown_nested_bold_italic() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("Start")?;

    let result = document_editing_controller::insert_markdown_at_position(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertMarkdownAtPositionDto {
            position: 5,
            anchor: 5,
            markdown: "***bold italic***".to_string(),
        },
    )?;

    assert!(result.blocks_added >= 1);

    let block_ids = get_block_ids(&db_context)?;
    let mut found_bi = false;
    for block_id in &block_ids {
        let elem_ids = block_controller::get_relationship(
            &db_context,
            block_id,
            &BlockRelationshipField::Elements,
        )?;
        for elem_id in &elem_ids {
            let elem = inline_element_controller::get(&db_context, elem_id)?.unwrap();
            if elem.fmt_font_bold == Some(true) && elem.fmt_font_italic == Some(true) {
                found_bi = true;
            }
        }
    }
    assert!(found_bi, "Should find an element with both bold and italic");

    Ok(())
}

#[test]
fn test_insert_html_link() -> Result<()> {
    let (db_context, event_hub, mut undo_redo_manager) = setup_with_text("See")?;

    document_editing_controller::insert_html_at_position(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &InsertHtmlAtPositionDto {
            position: 3,
            anchor: 3,
            html: "<p><a href=\"http://example.com\">link</a></p>".to_string(),
        },
    )?;

    let block_ids = get_block_ids(&db_context)?;
    let mut found_link = false;
    for block_id in &block_ids {
        let elem_ids = block_controller::get_relationship(
            &db_context,
            block_id,
            &BlockRelationshipField::Elements,
        )?;
        for elem_id in &elem_ids {
            let elem = inline_element_controller::get(&db_context, elem_id)?.unwrap();
            if elem.fmt_anchor_href == Some("http://example.com".to_string()) {
                found_link = true;
            }
        }
    }
    assert!(found_link, "Should find an element with link href");

    Ok(())
}
