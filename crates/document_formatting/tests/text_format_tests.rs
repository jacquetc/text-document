//! SetTextFormat — blocks, images, lists, tables, unicode, undo/redo

extern crate text_document_formatting as document_formatting;

use anyhow::Result;
use common::entities::InlineContent;

use document_formatting::document_formatting_controller;
use document_formatting::{CharVerticalAlignment, SetTextFormatDto, UnderlineStyle};

use test_harness::{
    BlockRelationshipField, FrameRelationshipField, UpdateBlockDto, UpdateInlineElementDto,
    block_controller, create_list, frame_controller, get_block_ids, get_sorted_cells,
    inline_element_controller, insert_image, insert_table, setup_with_text,
};

#[test]
fn test_set_text_format_block_with_image() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("ABCD")?;
    let block_ids = get_block_ids(&db)?;
    let block_id = block_ids[0];

    insert_image(&db, &hub, &mut urm, 2, "test.png", 100, 50)?;

    document_formatting_controller::set_text_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetTextFormatDto {
            position: 0,
            anchor: 2,
            font_bold: Some(true),
            ..Default::default()
        },
    )?;

    let elem_ids =
        block_controller::get_relationship(&db, &block_id, &BlockRelationshipField::Elements)?;
    let mut found_bold_text = false;
    let mut image_is_bold = false;
    for eid in &elem_ids {
        let elem = inline_element_controller::get(&db, eid)?.unwrap();
        match &elem.content {
            InlineContent::Text(t) if t == "AB" => {
                assert_eq!(elem.fmt_font_bold, Some(true));
                found_bold_text = true;
            }
            InlineContent::Image { .. } => {
                image_is_bold = elem.fmt_font_bold == Some(true);
            }
            InlineContent::Text(t) if t == "CD" => {
                assert_ne!(elem.fmt_font_bold, Some(true), "CD should not be bold");
            }
            _ => {}
        }
    }
    assert!(found_bold_text, "Should find bold 'AB' element");
    assert!(!image_is_bold, "Image should not be bold");
    Ok(())
}

#[test]
fn test_set_text_format_including_image() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("ABCD")?;
    insert_image(&db, &hub, &mut urm, 2, "pic.png", 64, 64)?;

    document_formatting_controller::set_text_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetTextFormatDto {
            position: 0,
            anchor: 5,
            font_italic: Some(true),
            ..Default::default()
        },
    )?;

    let block_ids = get_block_ids(&db)?;
    let elem_ids =
        block_controller::get_relationship(&db, &block_ids[0], &BlockRelationshipField::Elements)?;
    for eid in &elem_ids {
        let elem = inline_element_controller::get(&db, eid)?.unwrap();
        assert_eq!(
            elem.fmt_font_italic,
            Some(true),
            "All elements (text and image) should be italic"
        );
    }
    Ok(())
}

#[test]
fn test_set_text_format_unicode_split() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("\u{00e9}\u{00e0}\u{00fc}\u{00f6}")?;

    document_formatting_controller::set_text_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetTextFormatDto {
            position: 1,
            anchor: 3,
            font_underline: Some(true),
            underline_style: Some(UnderlineStyle::DashUnderline),
            ..Default::default()
        },
    )?;

    let block_ids = get_block_ids(&db)?;
    let elem_ids =
        block_controller::get_relationship(&db, &block_ids[0], &BlockRelationshipField::Elements)?;
    assert!(
        elem_ids.len() >= 3,
        "Unicode text should be split into at least 3 parts, got {}",
        elem_ids.len()
    );

    let mut found_underlined = false;
    for eid in &elem_ids {
        let elem = inline_element_controller::get(&db, eid)?.unwrap();
        if elem.fmt_font_underline == Some(true)
            && let InlineContent::Text(ref t) = elem.content
        {
            assert_eq!(t, "\u{00e0}\u{00fc}", "Middle chars should be underlined");
            assert_eq!(
                elem.fmt_underline_style,
                Some(common::entities::UnderlineStyle::DashUnderline)
            );
            found_underlined = true;
        }
    }
    assert!(found_underlined, "Should find underlined middle part");
    Ok(())
}

#[test]
fn test_set_text_format_in_list_blocks() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Item one\nItem two\nItem three")?;
    let list_result = create_list(
        &db,
        &hub,
        &mut urm,
        0,
        27,
        common::entities::ListStyle::Decimal,
    )?;

    document_formatting_controller::set_text_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetTextFormatDto {
            position: 0,
            anchor: 13,
            font_bold: Some(true),
            font_family: Some("Serif".into()),
            ..Default::default()
        },
    )?;

    let block_ids = get_block_ids(&db)?;

    let elem_ids_0 =
        block_controller::get_relationship(&db, &block_ids[0], &BlockRelationshipField::Elements)?;
    for eid in &elem_ids_0 {
        let elem = inline_element_controller::get(&db, eid)?.unwrap();
        if let InlineContent::Text(ref t) = elem.content
            && !t.is_empty()
        {
            assert_eq!(elem.fmt_font_bold, Some(true));
            assert_eq!(elem.fmt_font_family, Some("Serif".into()));
        }
    }

    let elem_ids_1 =
        block_controller::get_relationship(&db, &block_ids[1], &BlockRelationshipField::Elements)?;
    let mut found_bold_item = false;
    for eid in &elem_ids_1 {
        let elem = inline_element_controller::get(&db, eid)?.unwrap();
        if let InlineContent::Text(ref t) = elem.content
            && elem.fmt_font_bold == Some(true)
        {
            assert_eq!(t, "Item");
            found_bold_item = true;
        }
    }
    assert!(found_bold_item, "Should find bold 'Item' in second block");

    let block_0 = block_controller::get(&db, &block_ids[0])?.unwrap();
    assert_eq!(block_0.list, Some(list_result.list_id));
    Ok(())
}

#[test]
fn test_set_text_format_in_table_cell() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Before table")?;
    let table_result = insert_table(&db, &hub, &mut urm, 12, 2, 2)?;

    let cells = get_sorted_cells(&db, &table_result.table_id)?;
    let cell_frame_id = cells[0].cell_frame.unwrap();
    let cell_block_ids =
        frame_controller::get_relationship(&db, &cell_frame_id, &FrameRelationshipField::Blocks)?;
    let cell_block = block_controller::get(&db, &cell_block_ids[0])?.unwrap();
    let cell_block_pos = cell_block.document_position;

    let elem_ids =
        block_controller::get_relationship(&db, &cell_block.id, &BlockRelationshipField::Elements)?;
    let elem = inline_element_controller::get(&db, &elem_ids[0])?.unwrap();
    let mut update_elem: UpdateInlineElementDto = elem.into();
    update_elem.content = InlineContent::Text("Cell text".into());
    inline_element_controller::update(&db, &hub, &mut urm, None, &update_elem)?;

    let mut update_block: UpdateBlockDto = cell_block.into();
    update_block.plain_text = "Cell text".into();
    update_block.text_length = 9;
    block_controller::update(&db, &hub, &mut urm, None, &update_block)?;

    document_formatting_controller::set_text_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetTextFormatDto {
            position: cell_block_pos,
            anchor: cell_block_pos + 4,
            font_bold: Some(true),
            font_italic: Some(true),
            ..Default::default()
        },
    )?;

    let cell_block_ids_after =
        frame_controller::get_relationship(&db, &cell_frame_id, &FrameRelationshipField::Blocks)?;
    let elem_ids_after = block_controller::get_relationship(
        &db,
        &cell_block_ids_after[0],
        &BlockRelationshipField::Elements,
    )?;
    let mut found_bold_cell = false;
    for eid in &elem_ids_after {
        let elem = inline_element_controller::get(&db, eid)?.unwrap();
        if elem.fmt_font_bold == Some(true)
            && elem.fmt_font_italic == Some(true)
            && let InlineContent::Text(ref t) = elem.content
        {
            assert_eq!(t, "Cell");
            found_bold_cell = true;
        }
    }
    assert!(
        found_bold_cell,
        "Should find bold+italic 'Cell' in table cell"
    );
    Ok(())
}

#[test]
fn test_set_text_format_undo_redo_with_split() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("ABCDEFGH")?;

    document_formatting_controller::set_text_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetTextFormatDto {
            position: 2,
            anchor: 5,
            font_bold: Some(true),
            ..Default::default()
        },
    )?;

    let block_ids = get_block_ids(&db)?;
    let elem_ids_after =
        block_controller::get_relationship(&db, &block_ids[0], &BlockRelationshipField::Elements)?;
    assert!(
        elem_ids_after.len() >= 3,
        "Should have at least 3 elements after split"
    );

    urm.undo(None)?;
    let elem_ids_undo =
        block_controller::get_relationship(&db, &block_ids[0], &BlockRelationshipField::Elements)?;
    assert_eq!(elem_ids_undo.len(), 1, "After undo should have 1 element");
    let elem = inline_element_controller::get(&db, &elem_ids_undo[0])?.unwrap();
    assert_eq!(elem.fmt_font_bold, None);

    urm.redo(None)?;
    let elem_ids_redo =
        block_controller::get_relationship(&db, &block_ids[0], &BlockRelationshipField::Elements)?;
    assert!(
        elem_ids_redo.len() >= 3,
        "After redo should have 3+ elements again"
    );
    let mut found_bold = false;
    for eid in &elem_ids_redo {
        let e = inline_element_controller::get(&db, eid)?.unwrap();
        if e.fmt_font_bold == Some(true) {
            found_bold = true;
        }
    }
    assert!(found_bold, "After redo should have bold element");
    Ok(())
}

#[test]
fn test_set_text_format_empty_range_no_op() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Hello")?;

    document_formatting_controller::set_text_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetTextFormatDto {
            position: 3,
            anchor: 3,
            font_bold: Some(true),
            ..Default::default()
        },
    )?;

    let block_ids = get_block_ids(&db)?;
    let elem_ids =
        block_controller::get_relationship(&db, &block_ids[0], &BlockRelationshipField::Elements)?;
    assert_eq!(elem_ids.len(), 1);
    let elem = inline_element_controller::get(&db, &elem_ids[0])?.unwrap();
    assert_eq!(elem.fmt_font_bold, None);
    Ok(())
}

#[test]
fn test_set_text_format_all_fields() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Test")?;

    document_formatting_controller::set_text_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetTextFormatDto {
            position: 0,
            anchor: 4,
            font_family: Some("Monospace".into()),
            font_point_size: Some(24),
            font_weight: Some(900),
            font_bold: Some(true),
            font_italic: Some(true),
            font_underline: Some(true),
            font_overline: Some(true),
            font_strikeout: Some(true),
            letter_spacing: Some(3),
            word_spacing: Some(6),
            underline_style: Some(UnderlineStyle::WaveUnderline),
            vertical_alignment: Some(CharVerticalAlignment::SuperScript),
        },
    )?;

    let block_ids = get_block_ids(&db)?;
    let elem_ids =
        block_controller::get_relationship(&db, &block_ids[0], &BlockRelationshipField::Elements)?;
    let elem = inline_element_controller::get(&db, &elem_ids[0])?.unwrap();

    assert_eq!(elem.fmt_font_family, Some("Monospace".into()));
    assert_eq!(elem.fmt_font_point_size, Some(24));
    assert_eq!(elem.fmt_font_weight, Some(900));
    assert_eq!(elem.fmt_font_bold, Some(true));
    assert_eq!(elem.fmt_font_italic, Some(true));
    assert_eq!(elem.fmt_font_underline, Some(true));
    assert_eq!(elem.fmt_font_overline, Some(true));
    assert_eq!(elem.fmt_font_strikeout, Some(true));
    assert_eq!(elem.fmt_letter_spacing, Some(3));
    assert_eq!(elem.fmt_word_spacing, Some(6));
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
