//! SetTableFormat and SetTableCellFormat tests

extern crate text_document_formatting as document_formatting;

use anyhow::Result;

use document_formatting::document_formatting_controller;
use document_formatting::{
    Alignment, CellVerticalAlignment, SetTableCellFormatDto, SetTableFormatDto,
};

use test_harness::{
    get_sorted_cells, insert_table, setup_with_text, table_cell_controller, table_controller,
};

#[test]
fn test_set_table_format_all_fields() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Intro")?;
    let table_result = insert_table(&db, &hub, &mut urm, 5, 2, 3)?;
    let table_id = table_result.table_id;

    document_formatting_controller::set_table_format(
        &db, &hub, &mut urm, None,
        &SetTableFormatDto {
            table_id: table_id as i64, border: Some(2), cell_spacing: Some(4),
            cell_padding: Some(8), width: Some(600), alignment: Some(Alignment::Center),
        },
    )?;

    let table = table_controller::get(&db, &table_id)?.unwrap();
    assert_eq!(table.fmt_border, Some(2));
    assert_eq!(table.fmt_cell_spacing, Some(4));
    assert_eq!(table.fmt_cell_padding, Some(8));
    assert_eq!(table.fmt_width, Some(600));
    assert_eq!(table.fmt_alignment, Some(common::entities::Alignment::Center));
    Ok(())
}

#[test]
fn test_set_table_format_preserves_unset_fields() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("X")?;
    let table_result = insert_table(&db, &hub, &mut urm, 1, 1, 1)?;
    let table_id = table_result.table_id;

    document_formatting_controller::set_table_format(
        &db, &hub, &mut urm, None,
        &SetTableFormatDto { table_id: table_id as i64, border: Some(1), width: Some(400), ..Default::default() },
    )?;
    document_formatting_controller::set_table_format(
        &db, &hub, &mut urm, None,
        &SetTableFormatDto { table_id: table_id as i64, alignment: Some(Alignment::Right), ..Default::default() },
    )?;

    let table = table_controller::get(&db, &table_id)?.unwrap();
    assert_eq!(table.fmt_border, Some(1), "Border should be preserved");
    assert_eq!(table.fmt_width, Some(400), "Width should be preserved");
    assert_eq!(table.fmt_alignment, Some(common::entities::Alignment::Right));
    Ok(())
}

#[test]
fn test_set_table_format_undo_redo() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("T")?;
    let table_result = insert_table(&db, &hub, &mut urm, 1, 2, 2)?;
    let table_id = table_result.table_id;

    document_formatting_controller::set_table_format(
        &db, &hub, &mut urm, None,
        &SetTableFormatDto { table_id: table_id as i64, border: Some(3), alignment: Some(Alignment::Justify), ..Default::default() },
    )?;

    assert_eq!(table_controller::get(&db, &table_id)?.unwrap().fmt_border, Some(3));
    urm.undo(None)?;
    assert_eq!(table_controller::get(&db, &table_id)?.unwrap().fmt_border, None);
    urm.redo(None)?;
    assert_eq!(table_controller::get(&db, &table_id)?.unwrap().fmt_border, Some(3));
    Ok(())
}

#[test]
fn test_set_table_cell_format_all_fields() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Data")?;
    let table_result = insert_table(&db, &hub, &mut urm, 4, 2, 2)?;
    let cells = get_sorted_cells(&db, &table_result.table_id)?;
    let cell_id = cells[0].id;

    document_formatting_controller::set_table_cell_format(
        &db, &hub, &mut urm, None,
        &SetTableCellFormatDto {
            cell_id: cell_id as i64, padding: Some(10), border: Some(2),
            vertical_alignment: Some(CellVerticalAlignment::Middle),
            background_color: Some("#e0e0e0".into()),
        },
    )?;

    let cell = table_cell_controller::get(&db, &cell_id)?.unwrap();
    assert_eq!(cell.fmt_padding, Some(10));
    assert_eq!(cell.fmt_border, Some(2));
    assert_eq!(cell.fmt_vertical_alignment, Some(common::entities::CellVerticalAlignment::Middle));
    assert_eq!(cell.fmt_background_color, Some("#e0e0e0".into()));
    Ok(())
}

#[test]
fn test_set_table_cell_format_multiple_cells_independently() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Grid")?;
    let table_result = insert_table(&db, &hub, &mut urm, 4, 2, 2)?;
    let cells = get_sorted_cells(&db, &table_result.table_id)?;

    document_formatting_controller::set_table_cell_format(&db, &hub, &mut urm, None,
        &SetTableCellFormatDto { cell_id: cells[0].id as i64, background_color: Some("#ff0000".into()), vertical_alignment: Some(CellVerticalAlignment::Top), ..Default::default() },
    )?;
    document_formatting_controller::set_table_cell_format(&db, &hub, &mut urm, None,
        &SetTableCellFormatDto { cell_id: cells[3].id as i64, background_color: Some("#0000ff".into()), vertical_alignment: Some(CellVerticalAlignment::Bottom), ..Default::default() },
    )?;

    let cell_00 = table_cell_controller::get(&db, &cells[0].id)?.unwrap();
    assert_eq!(cell_00.fmt_background_color, Some("#ff0000".into()));
    assert_eq!(cell_00.fmt_vertical_alignment, Some(common::entities::CellVerticalAlignment::Top));

    let cell_11 = table_cell_controller::get(&db, &cells[3].id)?.unwrap();
    assert_eq!(cell_11.fmt_background_color, Some("#0000ff".into()));
    assert_eq!(cell_11.fmt_vertical_alignment, Some(common::entities::CellVerticalAlignment::Bottom));

    let cell_01 = table_cell_controller::get(&db, &cells[1].id)?.unwrap();
    assert_eq!(cell_01.fmt_background_color, None);
    assert_eq!(cell_01.fmt_vertical_alignment, None);
    Ok(())
}

#[test]
fn test_set_table_cell_format_undo_redo() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("C")?;
    let table_result = insert_table(&db, &hub, &mut urm, 1, 1, 1)?;
    let cells = get_sorted_cells(&db, &table_result.table_id)?;
    let cell_id = cells[0].id;

    document_formatting_controller::set_table_cell_format(&db, &hub, &mut urm, None,
        &SetTableCellFormatDto { cell_id: cell_id as i64, padding: Some(5), border: Some(1), ..Default::default() },
    )?;

    assert_eq!(table_cell_controller::get(&db, &cell_id)?.unwrap().fmt_padding, Some(5));
    urm.undo(None)?;
    assert_eq!(table_cell_controller::get(&db, &cell_id)?.unwrap().fmt_padding, None);
    urm.redo(None)?;
    assert_eq!(table_cell_controller::get(&db, &cell_id)?.unwrap().fmt_padding, Some(5));
    Ok(())
}

#[test]
fn test_set_table_cell_format_preserves_unset_fields() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("P")?;
    let table_result = insert_table(&db, &hub, &mut urm, 1, 1, 1)?;
    let cells = get_sorted_cells(&db, &table_result.table_id)?;
    let cell_id = cells[0].id;

    document_formatting_controller::set_table_cell_format(&db, &hub, &mut urm, None,
        &SetTableCellFormatDto { cell_id: cell_id as i64, background_color: Some("#aabbcc".into()), ..Default::default() },
    )?;
    document_formatting_controller::set_table_cell_format(&db, &hub, &mut urm, None,
        &SetTableCellFormatDto { cell_id: cell_id as i64, padding: Some(12), ..Default::default() },
    )?;

    let cell = table_cell_controller::get(&db, &cell_id)?.unwrap();
    assert_eq!(cell.fmt_background_color, Some("#aabbcc".into()), "Background should be preserved");
    assert_eq!(cell.fmt_padding, Some(12));
    Ok(())
}
