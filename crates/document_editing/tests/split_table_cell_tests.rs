//! Additional tests for the SplitTableCell use case.

extern crate text_document_editing as document_editing;
use anyhow::Result;

use document_editing::document_editing_controller;
use document_editing::{InsertTableDto, MergeTableCellsDto, SplitTableCellDto};

use test_harness::{
    get_document_stats, get_sorted_cells, setup_with_text, table_cell_controller,
};

// ═══════════════════════════════════════════════════════════════════
// SplitTableCell additional tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_split_cell_horizontal_only() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("")?;

    // Create table and merge 1x3 (one row, three columns)
    let insert_result = document_editing_controller::insert_table(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableDto {
            position: 0,
            anchor: 0,
            rows: 2,
            columns: 3,
        },
    )?;
    let table_id = insert_result.table_id;

    // Merge row 0 columns 0-2
    let merge_result = document_editing_controller::merge_table_cells(
        &db,
        &hub,
        &mut urm,
        None,
        &MergeTableCellsDto {
            table_id,
            start_row: 0,
            start_column: 0,
            end_row: 0,
            end_column: 2,
        },
    )?;

    let merged = table_cell_controller::get(&db, &(merge_result.merged_cell_id as u64))?.unwrap();
    assert_eq!(merged.row_span, 1);
    assert_eq!(merged.column_span, 3);

    // Split horizontally into 3 columns
    let split_result = document_editing_controller::split_table_cell(
        &db,
        &hub,
        &mut urm,
        None,
        &SplitTableCellDto {
            cell_id: merge_result.merged_cell_id,
            split_rows: 1,
            split_columns: 3,
        },
    )?;

    assert_eq!(split_result.new_cell_ids.len(), 3);

    // All cells in row 0 should be 1x1 again
    let cells = get_sorted_cells(&db, &(table_id as u64))?;
    let row0: Vec<_> = cells.iter().filter(|c| c.row == 0).collect();
    assert_eq!(row0.len(), 3);
    for c in &row0 {
        assert_eq!(c.row_span, 1);
        assert_eq!(c.column_span, 1);
    }

    Ok(())
}

#[test]
fn test_split_cell_vertical_only() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("")?;

    // Create table and merge 3x1 (three rows, one column)
    let insert_result = document_editing_controller::insert_table(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableDto {
            position: 0,
            anchor: 0,
            rows: 3,
            columns: 2,
        },
    )?;
    let table_id = insert_result.table_id;

    // Merge column 0, rows 0-2
    let merge_result = document_editing_controller::merge_table_cells(
        &db,
        &hub,
        &mut urm,
        None,
        &MergeTableCellsDto {
            table_id,
            start_row: 0,
            start_column: 0,
            end_row: 2,
            end_column: 0,
        },
    )?;

    let merged = table_cell_controller::get(&db, &(merge_result.merged_cell_id as u64))?.unwrap();
    assert_eq!(merged.row_span, 3);
    assert_eq!(merged.column_span, 1);

    // Split vertically into 3 rows
    let split_result = document_editing_controller::split_table_cell(
        &db,
        &hub,
        &mut urm,
        None,
        &SplitTableCellDto {
            cell_id: merge_result.merged_cell_id,
            split_rows: 3,
            split_columns: 1,
        },
    )?;

    assert_eq!(split_result.new_cell_ids.len(), 3);

    // All cells in column 0 should be back to 1x1
    let cells = get_sorted_cells(&db, &(table_id as u64))?;
    let col0: Vec<_> = cells.iter().filter(|c| c.column == 0).collect();
    assert_eq!(col0.len(), 3);
    for c in &col0 {
        assert_eq!(c.row_span, 1);
        assert_eq!(c.column_span, 1);
    }

    Ok(())
}

#[test]
fn test_split_cell_exceeding_span_fails() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("")?;

    let insert_result = document_editing_controller::insert_table(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableDto {
            position: 0,
            anchor: 0,
            rows: 2,
            columns: 2,
        },
    )?;
    let table_id = insert_result.table_id;

    // Merge 2x2
    let merge_result = document_editing_controller::merge_table_cells(
        &db,
        &hub,
        &mut urm,
        None,
        &MergeTableCellsDto {
            table_id,
            start_row: 0,
            start_column: 0,
            end_row: 1,
            end_column: 1,
        },
    )?;

    // Try to split into 3 rows when span is only 2 — should fail
    let result = document_editing_controller::split_table_cell(
        &db,
        &hub,
        &mut urm,
        None,
        &SplitTableCellDto {
            cell_id: merge_result.merged_cell_id,
            split_rows: 3,
            split_columns: 1,
        },
    );
    assert!(result.is_err());

    // Try to split into 3 columns when span is only 2 — should fail
    let result = document_editing_controller::split_table_cell(
        &db,
        &hub,
        &mut urm,
        None,
        &SplitTableCellDto {
            cell_id: merge_result.merged_cell_id,
            split_rows: 1,
            split_columns: 3,
        },
    );
    assert!(result.is_err());

    Ok(())
}

#[test]
fn test_split_cell_updates_block_count() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("")?;

    let insert_result = document_editing_controller::insert_table(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableDto {
            position: 0,
            anchor: 0,
            rows: 2,
            columns: 2,
        },
    )?;
    let table_id = insert_result.table_id;

    // Merge all 4 cells into 1
    let merge_result = document_editing_controller::merge_table_cells(
        &db,
        &hub,
        &mut urm,
        None,
        &MergeTableCellsDto {
            table_id,
            start_row: 0,
            start_column: 0,
            end_row: 1,
            end_column: 1,
        },
    )?;

    let stats_merged = get_document_stats(&db)?;
    // 1 original empty block + 1 merged cell block (the other 3 were removed)
    assert_eq!(stats_merged.block_count, 2);

    // Split back into 2x2
    document_editing_controller::split_table_cell(
        &db,
        &hub,
        &mut urm,
        None,
        &SplitTableCellDto {
            cell_id: merge_result.merged_cell_id,
            split_rows: 2,
            split_columns: 2,
        },
    )?;

    // Should have 1 original + 4 cell blocks = 5
    let stats_split = get_document_stats(&db)?;
    assert_eq!(stats_split.block_count, 5);

    Ok(())
}
