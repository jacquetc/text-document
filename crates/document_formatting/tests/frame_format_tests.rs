//! SetFrameFormat — main frame, sub-frame, table cell frame, blockquote, undo/redo

extern crate text_document_formatting as document_formatting;

use anyhow::Result;

use document_formatting::SetFrameFormatDto;
use document_formatting::document_formatting_controller;

use test_harness::{
    frame_controller, get_frame_id, get_sorted_cells, insert_frame, insert_table, setup_with_text,
};

#[test]
fn test_set_frame_format_blockquote() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Quote text")?;
    let frame_id = get_frame_id(&db)?;

    document_formatting_controller::set_frame_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetFrameFormatDto {
            position: 0,
            anchor: 0,
            frame_id: frame_id as i64,
            is_blockquote: Some(true),
            padding: Some(10),
            border: Some(1),
            ..Default::default()
        },
    )?;

    let frame = frame_controller::get(&db, &frame_id)?.unwrap();
    assert_eq!(frame.fmt_is_blockquote, Some(true));
    assert_eq!(frame.fmt_padding, Some(10));
    assert_eq!(frame.fmt_border, Some(1));
    Ok(())
}

#[test]
fn test_set_frame_format_sub_frame() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Main content")?;

    // Insert a sub-frame
    let frame_result = insert_frame(&db, &hub, &mut urm, 12)?;

    let sub_frame_id = frame_result.frame_id;

    document_formatting_controller::set_frame_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetFrameFormatDto {
            position: 0,
            anchor: 0,
            frame_id: sub_frame_id as i64,
            height: Some(200),
            width: Some(400),
            top_margin: Some(5),
            bottom_margin: Some(5),
            left_margin: Some(10),
            right_margin: Some(10),
            padding: Some(15),
            border: Some(3),
            is_blockquote: Some(false),
        },
    )?;

    let frame = frame_controller::get(&db, &sub_frame_id)?.unwrap();
    assert_eq!(frame.fmt_height, Some(200));
    assert_eq!(frame.fmt_width, Some(400));
    assert_eq!(frame.fmt_top_margin, Some(5));
    assert_eq!(frame.fmt_bottom_margin, Some(5));
    assert_eq!(frame.fmt_left_margin, Some(10));
    assert_eq!(frame.fmt_right_margin, Some(10));
    assert_eq!(frame.fmt_padding, Some(15));
    assert_eq!(frame.fmt_border, Some(3));
    assert_eq!(frame.fmt_is_blockquote, Some(false));
    Ok(())
}

#[test]
fn test_set_frame_format_table_cell_frame() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Text")?;

    let table_result = insert_table(&db, &hub, &mut urm, 4, 1, 2)?;

    let cells = get_sorted_cells(&db, &table_result.table_id)?;
    let cell_frame_id = cells[0].cell_frame.unwrap();

    document_formatting_controller::set_frame_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetFrameFormatDto {
            position: 0,
            anchor: 0,
            frame_id: cell_frame_id as i64,
            padding: Some(8),
            border: Some(2),
            ..Default::default()
        },
    )?;

    let cell_frame = frame_controller::get(&db, &cell_frame_id)?.unwrap();
    assert_eq!(cell_frame.fmt_padding, Some(8));
    assert_eq!(cell_frame.fmt_border, Some(2));
    Ok(())
}

#[test]
fn test_set_frame_format_undo_redo() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Frame")?;
    let frame_id = get_frame_id(&db)?;

    document_formatting_controller::set_frame_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetFrameFormatDto {
            position: 0,
            anchor: 0,
            frame_id: frame_id as i64,
            width: Some(800),
            height: Some(600),
            ..Default::default()
        },
    )?;

    let frame = frame_controller::get(&db, &frame_id)?.unwrap();
    assert_eq!(frame.fmt_width, Some(800));

    urm.undo(None)?;
    let frame = frame_controller::get(&db, &frame_id)?.unwrap();
    assert_eq!(frame.fmt_width, None);

    urm.redo(None)?;
    let frame = frame_controller::get(&db, &frame_id)?.unwrap();
    assert_eq!(frame.fmt_width, Some(800));
    Ok(())
}

#[test]
fn test_set_frame_format_preserves_unset_fields() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Data")?;
    let frame_id = get_frame_id(&db)?;

    // Set height and padding
    document_formatting_controller::set_frame_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetFrameFormatDto {
            position: 0,
            anchor: 0,
            frame_id: frame_id as i64,
            height: Some(300),
            padding: Some(12),
            ..Default::default()
        },
    )?;

    // Set width only — height and padding should be preserved
    document_formatting_controller::set_frame_format(
        &db,
        &hub,
        &mut urm,
        None,
        &SetFrameFormatDto {
            position: 0,
            anchor: 0,
            frame_id: frame_id as i64,
            width: Some(500),
            ..Default::default()
        },
    )?;

    let frame = frame_controller::get(&db, &frame_id)?.unwrap();
    assert_eq!(frame.fmt_height, Some(300), "Height should be preserved");
    assert_eq!(frame.fmt_padding, Some(12), "Padding should be preserved");
    assert_eq!(frame.fmt_width, Some(500), "Width should be set");
    Ok(())
}
