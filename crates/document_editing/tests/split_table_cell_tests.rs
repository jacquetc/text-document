//! Additional tests for the SplitTableCell use case.

extern crate text_document_editing as document_editing;
use anyhow::Result;

use document_editing::document_editing_controller;
use document_editing::{InsertTableDto, MergeTableCellsDto, SplitTableCellDto};

use test_harness::{get_document_stats, get_sorted_cells, setup_with_text, table_cell_controller};

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

// ═══════════════════════════════════════════════════════════════════
// Undo/redo tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_split_cell_undo_restores_merged_cell() -> Result<()> {
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

    let cells_after_merge = get_sorted_cells(&db, &(table_id as u64))?;
    assert_eq!(cells_after_merge.len(), 1);

    // Split into 2x2
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

    let cells_after_split = get_sorted_cells(&db, &(table_id as u64))?;
    assert_eq!(cells_after_split.len(), 4);

    // Undo the split — should restore the merged cell
    urm.undo(None)?;
    let cells_after_undo = get_sorted_cells(&db, &(table_id as u64))?;
    assert_eq!(
        cells_after_undo.len(),
        1,
        "Undo should restore to 1 merged cell"
    );
    assert_eq!(cells_after_undo[0].row_span, 2);
    assert_eq!(cells_after_undo[0].column_span, 2);

    // Redo the split
    urm.redo(None)?;
    let cells_after_redo = get_sorted_cells(&db, &(table_id as u64))?;
    assert_eq!(
        cells_after_redo.len(),
        4,
        "Redo should re-split into 4 cells"
    );
    for c in &cells_after_redo {
        assert_eq!(c.row_span, 1);
        assert_eq!(c.column_span, 1);
    }

    Ok(())
}

#[test]
fn test_split_cell_2x2_grid() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("")?;

    // Create 3x3 table
    let insert_result = document_editing_controller::insert_table(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableDto {
            position: 0,
            anchor: 0,
            rows: 3,
            columns: 3,
        },
    )?;
    let table_id = insert_result.table_id;

    // Merge a 2x2 region (rows 0-1, cols 0-1)
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

    // 9 original - 3 absorbed = 6 cells
    let cells_after_merge = get_sorted_cells(&db, &(table_id as u64))?;
    assert_eq!(cells_after_merge.len(), 6);

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

    // Should be back to 9 cells
    let cells_after_split = get_sorted_cells(&db, &(table_id as u64))?;
    assert_eq!(cells_after_split.len(), 9);

    // Verify all cells are 1x1
    for c in &cells_after_split {
        assert_eq!(
            c.row_span, 1,
            "Cell at ({},{}) should have row_span 1",
            c.row, c.column
        );
        assert_eq!(
            c.column_span, 1,
            "Cell at ({},{}) should have column_span 1",
            c.row, c.column
        );
    }

    Ok(())
}

#[test]
fn test_split_1x1_cell_is_noop_or_error() -> Result<()> {
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

    // Get the first cell (already 1x1)
    let cells = get_sorted_cells(&db, &(table_id as u64))?;
    let cell = &cells[0];
    assert_eq!(cell.row_span, 1);
    assert_eq!(cell.column_span, 1);

    // Splitting a 1x1 cell into 1x1 should either be a no-op or error
    let result = document_editing_controller::split_table_cell(
        &db,
        &hub,
        &mut urm,
        None,
        &SplitTableCellDto {
            cell_id: cell.id as i64,
            split_rows: 1,
            split_columns: 1,
        },
    );

    // Either it errors or produces 1 cell (the same one)
    if let Ok(split_result) = result {
        assert_eq!(
            split_result.new_cell_ids.len(),
            1,
            "Splitting 1x1 into 1x1 should produce 1 cell"
        );
    }
    // If it errors, that's also acceptable behavior

    // Table should still have 4 cells
    let cells_after = get_sorted_cells(&db, &(table_id as u64))?;
    assert_eq!(cells_after.len(), 4);

    Ok(())
}

#[test]
fn test_split_cell_invalid_cell_id_errors() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("")?;

    // Try to split a non-existent cell
    let result = document_editing_controller::split_table_cell(
        &db,
        &hub,
        &mut urm,
        None,
        &SplitTableCellDto {
            cell_id: 999999,
            split_rows: 2,
            split_columns: 2,
        },
    );

    assert!(result.is_err(), "Splitting non-existent cell should fail");

    Ok(())
}
