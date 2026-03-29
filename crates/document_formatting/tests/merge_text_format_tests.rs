//! MergeTextFormat — preserving fields, lists, partial split, undo/redo

extern crate text_document_formatting as document_formatting;

use anyhow::Result;
use common::database::db_context::DbContext;
use common::entities::InlineContent;

use document_formatting::document_formatting_controller;
use document_formatting::{
    CharVerticalAlignment, MergeTextFormatDto, SetTextFormatDto, UnderlineStyle,
};

use test_harness::{
    BlockRelationshipField, block_controller, create_list, get_block_ids,
    inline_element_controller, setup_with_text,
};

#[test]
fn test_merge_text_format_empty_family_preserves_existing() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Hello")?;

    // Set initial font family
    document_formatting_controller::set_text_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetTextFormatDto {
            position: 0,
            anchor: 5,
            font_family: Some("Arial".into()),
            font_bold: Some(false),
            ..Default::default()
        },
    )?;

    // Merge with empty string font_family — should NOT overwrite
    document_formatting_controller::merge_text_format(
        &db,
        &hub,
        &mut urm,
        None,
        &MergeTextFormatDto {
            position: 0,
            anchor: 5,
            font_family: Some("".into()),
            font_bold: Some(true),
            ..Default::default()
        },
    )?;

    let block_ids = get_block_ids(&db)?;
    let elem_ids = block_controller::get_relationship(
        &db,
        &block_ids[0],
        &BlockRelationshipField::Elements,
    )?;
    let elem = inline_element_controller::get(&db, &elem_ids[0])?.unwrap();

    assert_eq!(elem.fmt_font_family, Some("Arial".into()), "Empty string should not replace family");
    assert_eq!(elem.fmt_font_bold, Some(true), "Bold should be merged");
    Ok(())
}

#[test]
fn test_merge_text_format_on_list_blocks() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Alpha\nBeta\nGamma")?;

    // Create list
    create_list(&db, &hub, &mut urm, 0, 16, common::entities::ListStyle::Disc)?;

    // Set initial formatting on all text
    document_formatting_controller::set_text_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetTextFormatDto {
            position: 0,
            anchor: 16,
            font_family: Some("Georgia".into()),
            font_point_size: Some(14),
            font_bold: Some(false),
            font_italic: Some(false),
            ..Default::default()
        },
    )?;

    // Merge bold on first two items only (0..10 covers "Alpha\nBeta")
    document_formatting_controller::merge_text_format(
        &db,
        &hub,
        &mut urm,
        None,
        &MergeTextFormatDto {
            position: 0,
            anchor: 10,
            font_family: None,
            font_bold: Some(true),
            font_italic: None,
            font_underline: None,
            font_strikeout: None,
        },
    )?;

    let block_ids = get_block_ids(&db)?;

    // First block "Alpha" should be bold but keep Georgia family
    let elem_ids_0 = block_controller::get_relationship(
        &db,
        &block_ids[0],
        &BlockRelationshipField::Elements,
    )?;
    let elem0 = inline_element_controller::get(&db, &elem_ids_0[0])?.unwrap();
    assert_eq!(elem0.fmt_font_bold, Some(true));
    assert_eq!(elem0.fmt_font_family, Some("Georgia".into()));
    assert_eq!(elem0.fmt_font_point_size, Some(14));

    // Third block "Gamma" should NOT be bold
    let elem_ids_2 = block_controller::get_relationship(
        &db,
        &block_ids[2],
        &BlockRelationshipField::Elements,
    )?;
    let elem2 = inline_element_controller::get(&db, &elem_ids_2[0])?.unwrap();
    assert_eq!(elem2.fmt_font_bold, Some(false), "Gamma should not be bold");
    Ok(())
}

#[test]
fn test_merge_text_format_partial_split_preserves_fields() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("ABCDEF")?;

    // Set initial formatting
    document_formatting_controller::set_text_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetTextFormatDto {
            position: 0,
            anchor: 6,
            font_family: Some("Courier".into()),
            font_point_size: Some(12),
            font_weight: Some(400),
            font_bold: Some(false),
            font_italic: Some(false),
            font_underline: Some(false),
            font_overline: Some(true),
            font_strikeout: Some(false),
            letter_spacing: Some(2),
            word_spacing: Some(4),
            underline_style: Some(UnderlineStyle::NoUnderline),
            vertical_alignment: Some(CharVerticalAlignment::Normal),
        },
    )?;

    // Merge italic only on "CD" (positions 2..4)
    document_formatting_controller::merge_text_format(
        &db,
        &hub,
        &mut urm,
        None,
        &MergeTextFormatDto {
            position: 2,
            anchor: 4,
            font_family: None,
            font_bold: None,
            font_italic: Some(true),
            font_underline: None,
            font_strikeout: None,
        },
    )?;

    let block_ids = get_block_ids(&db)?;
    let elem_ids = block_controller::get_relationship(
        &db,
        &block_ids[0],
        &BlockRelationshipField::Elements,
    )?;

    // Find the "CD" element
    let mut found_cd = false;
    for eid in &elem_ids {
        let elem = inline_element_controller::get(&db, eid)?.unwrap();
        if let InlineContent::Text(ref t) = elem.content {
            if t == "CD" {
                assert_eq!(elem.fmt_font_italic, Some(true), "CD should be italic");
                // All other fields should be preserved from set_text_format
                assert_eq!(elem.fmt_font_family, Some("Courier".into()));
                assert_eq!(elem.fmt_font_point_size, Some(12));
                assert_eq!(elem.fmt_font_overline, Some(true));
                assert_eq!(elem.fmt_letter_spacing, Some(2));
                found_cd = true;
            }
        }
    }
    assert!(found_cd, "Should find 'CD' element with merged italic");
    Ok(())
}

#[test]
fn test_merge_text_format_undo_redo() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Test")?;

    // Initial format
    document_formatting_controller::set_text_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetTextFormatDto {
            position: 0,
            anchor: 4,
            font_bold: Some(false),
            font_italic: Some(false),
            ..Default::default()
        },
    )?;

    // Merge
    document_formatting_controller::merge_text_format(
        &db,
        &hub,
        &mut urm,
        None,
        &MergeTextFormatDto {
            position: 0,
            anchor: 4,
            font_family: None,
            font_bold: Some(true),
            font_italic: None,
            font_underline: None,
            font_strikeout: None,
        },
    )?;

    let block_ids = get_block_ids(&db)?;
    let get_bold = |db: &DbContext| -> anyhow::Result<Option<bool>> {
        let eids = block_controller::get_relationship(
            db,
            &block_ids[0],
            &BlockRelationshipField::Elements,
        )?;
        let elem = inline_element_controller::get(db, &eids[0])?.unwrap();
        Ok(elem.fmt_font_bold)
    };

    assert_eq!(get_bold(&db)?, Some(true));

    urm.undo(None)?;
    assert_eq!(get_bold(&db)?, Some(false), "After undo merge, bold should be false");

    urm.redo(None)?;
    assert_eq!(get_bold(&db)?, Some(true), "After redo merge, bold should be true");
    Ok(())
}
