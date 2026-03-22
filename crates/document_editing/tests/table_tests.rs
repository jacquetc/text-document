extern crate text_document_editing as document_editing;
use anyhow::Result;

use document_editing::document_editing_controller;
use document_editing::{
    InsertTableColumnDto, InsertTableDto, InsertTableRowDto, MergeTableCellsDto,
    RemoveTableColumnDto, RemoveTableDto, RemoveTableRowDto, SplitTableCellDto,
};

use test_harness::{
    FrameRelationshipField, block_controller, export_text, frame_controller, get_all_block_ids,
    get_document_stats, get_sorted_cells, get_table_ids, setup_with_text, table_cell_controller,
    table_controller,
};

// ─── InsertTable tests ──────────────────────────────────────────

#[test]
fn test_insert_table_basic() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("")?;

    let result = document_editing_controller::insert_table(
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

    assert!(result.table_id > 0);

    // Verify table entity exists with correct dimensions
    let table = table_controller::get(&db, &(result.table_id as u64))?.unwrap();
    assert_eq!(table.rows, 2);
    assert_eq!(table.columns, 3);

    // Verify 6 cells created
    let cells = get_sorted_cells(&db, &(result.table_id as u64))?;
    assert_eq!(cells.len(), 6);

    // Verify cell grid positions
    assert_eq!((cells[0].row, cells[0].column), (0, 0));
    assert_eq!((cells[1].row, cells[1].column), (0, 1));
    assert_eq!((cells[2].row, cells[2].column), (0, 2));
    assert_eq!((cells[3].row, cells[3].column), (1, 0));
    assert_eq!((cells[4].row, cells[4].column), (1, 1));
    assert_eq!((cells[5].row, cells[5].column), (1, 2));

    // Verify each cell has a frame
    for cell in &cells {
        assert!(
            cell.cell_frame.is_some(),
            "Cell ({},{}) has no frame",
            cell.row,
            cell.column
        );
        let frame = frame_controller::get(&db, &cell.cell_frame.unwrap())?.unwrap();
        // Frame should have blocks
        let block_ids =
            frame_controller::get_relationship(&db, &frame.id, &FrameRelationshipField::Blocks)?;
        assert!(
            !block_ids.is_empty(),
            "Cell ({},{}) frame has no blocks",
            cell.row,
            cell.column
        );
    }

    // Verify table is owned by document
    let table_ids = get_table_ids(&db)?;
    assert!(table_ids.contains(&(result.table_id as u64)));

    // Verify document block_count increased (1 existing empty block + 6 cell blocks)
    let stats = get_document_stats(&db)?;
    assert_eq!(stats.block_count, 7);

    Ok(())
}

#[test]
fn test_insert_table_mid_document() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Hello\nWorld")?;

    // Insert 2x2 table between "Hello" and "World" (position 6 = after block separator)
    let _result = document_editing_controller::insert_table(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableDto {
            position: 6,
            anchor: 6,
            rows: 2,
            columns: 2,
        },
    )?;

    // Verify blocks after the table have shifted positions
    let all_block_ids = get_all_block_ids(&db)?;
    let mut all_blocks: Vec<_> = all_block_ids
        .iter()
        .filter_map(|id| block_controller::get(&db, id).ok().flatten())
        .collect();
    all_blocks.sort_by_key(|b| b.document_position);

    // "Hello" block at position 0, then 4 table cell blocks, then "World" block shifted
    let hello_block = all_blocks.iter().find(|b| b.plain_text == "Hello").unwrap();
    assert_eq!(hello_block.document_position, 0);

    let world_block = all_blocks.iter().find(|b| b.plain_text == "World").unwrap();
    // Original position was 6, shifted by 4 (2x2 table cells)
    assert_eq!(world_block.document_position, 10);

    Ok(())
}

#[test]
fn test_insert_table_undo_redo() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("")?;

    let result = document_editing_controller::insert_table(
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
    let table_id = result.table_id as u64;

    // Table exists
    assert!(table_controller::get(&db, &table_id)?.is_some());
    let tables_before = get_table_ids(&db)?;
    assert_eq!(tables_before.len(), 1);

    // Undo
    urm.undo(None)?;

    // Table should be gone
    let tables_after_undo = get_table_ids(&db)?;
    assert_eq!(tables_after_undo.len(), 0);

    let stats = get_document_stats(&db)?;
    assert_eq!(stats.block_count, 1); // back to just the original empty block

    // Redo
    urm.redo(None)?;

    let tables_after_redo = get_table_ids(&db)?;
    assert_eq!(tables_after_redo.len(), 1);

    Ok(())
}

#[test]
fn test_insert_table_validation() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("")?;

    // 0 rows should fail
    let result = document_editing_controller::insert_table(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableDto {
            position: 0,
            anchor: 0,
            rows: 0,
            columns: 3,
        },
    );
    assert!(result.is_err());

    // 0 columns should fail
    let result = document_editing_controller::insert_table(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableDto {
            position: 0,
            anchor: 0,
            rows: 2,
            columns: 0,
        },
    );
    assert!(result.is_err());

    Ok(())
}

#[test]
fn test_insert_table_updates_document_stats() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Hello")?;

    let stats_before = get_document_stats(&db)?;
    assert_eq!(stats_before.block_count, 1);

    document_editing_controller::insert_table(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableDto {
            position: 5,
            anchor: 5,
            rows: 3,
            columns: 2,
        },
    )?;

    let stats_after = get_document_stats(&db)?;
    assert_eq!(stats_after.block_count, 7); // 1 original + 6 table cells

    Ok(())
}

// ─── RemoveTable tests ──────────────────────────────────────────

#[test]
fn test_remove_table() -> Result<()> {
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
            columns: 3,
        },
    )?;
    let table_id = insert_result.table_id;

    // Verify table exists
    assert_eq!(get_table_ids(&db)?.len(), 1);

    document_editing_controller::remove_table(
        &db,
        &hub,
        &mut urm,
        None,
        &RemoveTableDto { table_id },
    )?;

    // Table should be gone
    assert_eq!(get_table_ids(&db)?.len(), 0);

    // Block count should be back to just the original empty block
    let stats = get_document_stats(&db)?;
    assert_eq!(stats.block_count, 1);

    Ok(())
}

#[test]
fn test_remove_table_undo() -> Result<()> {
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

    document_editing_controller::remove_table(
        &db,
        &hub,
        &mut urm,
        None,
        &RemoveTableDto {
            table_id: insert_result.table_id,
        },
    )?;

    assert_eq!(get_table_ids(&db)?.len(), 0);

    // Undo remove
    urm.undo(None)?;

    // Table should be restored (1 original empty block + 4 table cell blocks)
    assert_eq!(get_table_ids(&db)?.len(), 1);
    let stats = get_document_stats(&db)?;
    assert_eq!(stats.block_count, 5);

    Ok(())
}

#[test]
fn test_remove_table_shifts_positions() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Hello\nWorld")?;

    let insert_result = document_editing_controller::insert_table(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableDto {
            position: 6,
            anchor: 6,
            rows: 2,
            columns: 2,
        },
    )?;

    document_editing_controller::remove_table(
        &db,
        &hub,
        &mut urm,
        None,
        &RemoveTableDto {
            table_id: insert_result.table_id,
        },
    )?;

    // "World" block should be back at its original position
    let text = export_text(&db, &hub)?;
    assert_eq!(text, "Hello\nWorld");

    Ok(())
}

// ─── InsertTableRow tests ───────────────────────────────────────

#[test]
fn test_insert_row_at_end() -> Result<()> {
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
            columns: 3,
        },
    )?;
    let table_id = insert_result.table_id;

    let row_result = document_editing_controller::insert_table_row(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableRowDto {
            table_id,
            row_index: 2,
        },
    )?;

    assert_eq!(row_result.new_row_count, 3);

    let table = table_controller::get(&db, &(table_id as u64))?.unwrap();
    assert_eq!(table.rows, 3);

    let cells = get_sorted_cells(&db, &(table_id as u64))?;
    assert_eq!(cells.len(), 9); // 3 rows x 3 columns

    // New row cells at row 2
    let new_row_cells: Vec<_> = cells.iter().filter(|c| c.row == 2).collect();
    assert_eq!(new_row_cells.len(), 3);

    Ok(())
}

#[test]
fn test_insert_row_at_beginning() -> Result<()> {
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

    document_editing_controller::insert_table_row(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableRowDto {
            table_id,
            row_index: 0,
        },
    )?;

    let cells = get_sorted_cells(&db, &(table_id as u64))?;
    assert_eq!(cells.len(), 6); // 3 rows x 2 columns

    // Original row 0 cells should now be at row 1
    // New row 0 should have empty cells
    let row0_cells: Vec<_> = cells.iter().filter(|c| c.row == 0).collect();
    assert_eq!(row0_cells.len(), 2);

    let row1_cells: Vec<_> = cells.iter().filter(|c| c.row == 1).collect();
    assert_eq!(row1_cells.len(), 2);

    Ok(())
}

#[test]
fn test_insert_row_undo() -> Result<()> {
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

    document_editing_controller::insert_table_row(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableRowDto {
            table_id,
            row_index: 1,
        },
    )?;

    let table = table_controller::get(&db, &(table_id as u64))?.unwrap();
    assert_eq!(table.rows, 3);

    urm.undo(None)?;

    let table = table_controller::get(&db, &(table_id as u64))?.unwrap();
    assert_eq!(table.rows, 2);
    let cells = get_sorted_cells(&db, &(table_id as u64))?;
    assert_eq!(cells.len(), 4);

    Ok(())
}

// ─── InsertTableColumn tests ────────────────────────────────────

#[test]
fn test_insert_column_at_end() -> Result<()> {
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
            columns: 3,
        },
    )?;
    let table_id = insert_result.table_id;

    let col_result = document_editing_controller::insert_table_column(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableColumnDto {
            table_id,
            column_index: 3,
        },
    )?;

    assert_eq!(col_result.new_column_count, 4);

    let cells = get_sorted_cells(&db, &(table_id as u64))?;
    assert_eq!(cells.len(), 8); // 2 rows x 4 columns

    Ok(())
}

#[test]
fn test_insert_column_undo() -> Result<()> {
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

    document_editing_controller::insert_table_column(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableColumnDto {
            table_id,
            column_index: 1,
        },
    )?;

    assert_eq!(
        table_controller::get(&db, &(table_id as u64))?
            .unwrap()
            .columns,
        3
    );

    urm.undo(None)?;

    assert_eq!(
        table_controller::get(&db, &(table_id as u64))?
            .unwrap()
            .columns,
        2
    );

    Ok(())
}

// ─── RemoveTableRow tests ───────────────────────────────────────

#[test]
fn test_remove_row() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("")?;

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

    let row_result = document_editing_controller::remove_table_row(
        &db,
        &hub,
        &mut urm,
        None,
        &RemoveTableRowDto {
            table_id,
            row_index: 1,
        },
    )?;

    assert_eq!(row_result.new_row_count, 2);

    let cells = get_sorted_cells(&db, &(table_id as u64))?;
    assert_eq!(cells.len(), 4); // 2 rows x 2 columns

    // Row 0 cells should be unchanged, old row 2 now row 1
    let row0_cells: Vec<_> = cells.iter().filter(|c| c.row == 0).collect();
    assert_eq!(row0_cells.len(), 2);
    let row1_cells: Vec<_> = cells.iter().filter(|c| c.row == 1).collect();
    assert_eq!(row1_cells.len(), 2);

    Ok(())
}

#[test]
fn test_remove_last_row_fails() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("")?;

    let insert_result = document_editing_controller::insert_table(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableDto {
            position: 0,
            anchor: 0,
            rows: 1,
            columns: 2,
        },
    )?;

    let result = document_editing_controller::remove_table_row(
        &db,
        &hub,
        &mut urm,
        None,
        &RemoveTableRowDto {
            table_id: insert_result.table_id,
            row_index: 0,
        },
    );

    assert!(result.is_err());

    Ok(())
}

#[test]
fn test_remove_row_undo() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("")?;

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

    document_editing_controller::remove_table_row(
        &db,
        &hub,
        &mut urm,
        None,
        &RemoveTableRowDto {
            table_id,
            row_index: 0,
        },
    )?;

    assert_eq!(
        table_controller::get(&db, &(table_id as u64))?
            .unwrap()
            .rows,
        2
    );

    urm.undo(None)?;

    assert_eq!(
        table_controller::get(&db, &(table_id as u64))?
            .unwrap()
            .rows,
        3
    );
    assert_eq!(get_sorted_cells(&db, &(table_id as u64))?.len(), 6);

    Ok(())
}

// ─── RemoveTableColumn tests ────────────────────────────────────

#[test]
fn test_remove_column() -> Result<()> {
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
            columns: 3,
        },
    )?;
    let table_id = insert_result.table_id;

    let col_result = document_editing_controller::remove_table_column(
        &db,
        &hub,
        &mut urm,
        None,
        &RemoveTableColumnDto {
            table_id,
            column_index: 1,
        },
    )?;

    assert_eq!(col_result.new_column_count, 2);

    let cells = get_sorted_cells(&db, &(table_id as u64))?;
    assert_eq!(cells.len(), 4);

    // Columns should be 0 and 1 (old column 2 shifted to 1)
    for cell in &cells {
        assert!(cell.column < 2);
    }

    Ok(())
}

#[test]
fn test_remove_last_column_fails() -> Result<()> {
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
            columns: 1,
        },
    )?;

    let result = document_editing_controller::remove_table_column(
        &db,
        &hub,
        &mut urm,
        None,
        &RemoveTableColumnDto {
            table_id: insert_result.table_id,
            column_index: 0,
        },
    );

    assert!(result.is_err());

    Ok(())
}

// ─── MergeTableCells tests ──────────────────────────────────────

#[test]
fn test_merge_2x2_cells() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("")?;

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

    // Surviving cell should have row_span=2, column_span=2
    let surviving =
        table_cell_controller::get(&db, &(merge_result.merged_cell_id as u64))?.unwrap();
    assert_eq!(surviving.row_span, 2);
    assert_eq!(surviving.column_span, 2);

    // Should have 9 - 3 = 6 cells remaining (removed 3 of the 4 in the 2x2 block)
    let cells = get_sorted_cells(&db, &(table_id as u64))?;
    assert_eq!(cells.len(), 6);

    Ok(())
}

#[test]
fn test_merge_already_merged_fails() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("")?;

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

    // First merge
    document_editing_controller::merge_table_cells(
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

    // Second merge overlapping the first should fail
    let result = document_editing_controller::merge_table_cells(
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
    );

    assert!(result.is_err());

    Ok(())
}

#[test]
fn test_merge_undo() -> Result<()> {
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

    document_editing_controller::merge_table_cells(
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

    assert_eq!(get_sorted_cells(&db, &(table_id as u64))?.len(), 1);

    urm.undo(None)?;

    assert_eq!(get_sorted_cells(&db, &(table_id as u64))?.len(), 4);

    Ok(())
}

// ─── SplitTableCell tests ───────────────────────────────────────

#[test]
fn test_split_merged_cell() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("")?;

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

    // Merge 2x2 block
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
    assert_eq!(cells_after_merge.len(), 6);

    // Split the merged cell back into 2x2
    let split_result = document_editing_controller::split_table_cell(
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

    assert_eq!(split_result.new_cell_ids.len(), 4); // 4 sub-cells total

    let cells_after_split = get_sorted_cells(&db, &(table_id as u64))?;
    assert_eq!(cells_after_split.len(), 9); // back to 3x3

    Ok(())
}

#[test]
fn test_split_unmerged_cell_fails() -> Result<()> {
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

    // Get a cell (all are 1x1)
    let cells = get_sorted_cells(&db, &(insert_result.table_id as u64))?;
    let cell_id = cells[0].id as i64;

    // Try to split a 1x1 cell into 1x1 — should fail
    let result = document_editing_controller::split_table_cell(
        &db,
        &hub,
        &mut urm,
        None,
        &SplitTableCellDto {
            cell_id,
            split_rows: 1,
            split_columns: 1,
        },
    );

    assert!(result.is_err());

    Ok(())
}

#[test]
fn test_split_undo() -> Result<()> {
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

    // Merge all 4 cells
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

    assert_eq!(get_sorted_cells(&db, &(table_id as u64))?.len(), 4);

    // Undo split — back to merged state
    urm.undo(None)?;

    let cells = get_sorted_cells(&db, &(table_id as u64))?;
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].row_span, 2);
    assert_eq!(cells[0].column_span, 2);

    Ok(())
}

// ─── Document position consistency tests ────────────────────────

#[test]
fn test_positions_sequential_after_insert_table() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("First\nSecond\nThird")?;

    document_editing_controller::insert_table(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableDto {
            position: 6,
            anchor: 6,
            rows: 2,
            columns: 2,
        },
    )?;

    // All blocks should have non-negative, distinct positions
    let all_block_ids = get_all_block_ids(&db)?;
    let mut positions: Vec<i64> = all_block_ids
        .iter()
        .filter_map(|id| block_controller::get(&db, id).ok().flatten())
        .map(|b| b.document_position)
        .collect();
    positions.sort();

    // No duplicates
    let unique: std::collections::HashSet<i64> = positions.iter().copied().collect();
    assert_eq!(
        unique.len(),
        positions.len(),
        "Duplicate positions found: {:?}",
        positions
    );

    // All non-negative
    for pos in &positions {
        assert!(*pos >= 0, "Negative position found: {}", pos);
    }

    Ok(())
}

#[test]
fn test_positions_consistent_after_row_insert_and_remove() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Before\nAfter")?;

    let insert_result = document_editing_controller::insert_table(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableDto {
            position: 7,
            anchor: 7,
            rows: 2,
            columns: 2,
        },
    )?;
    let table_id = insert_result.table_id;

    // Add a row
    document_editing_controller::insert_table_row(
        &db,
        &hub,
        &mut urm,
        None,
        &InsertTableRowDto {
            table_id,
            row_index: 1,
        },
    )?;

    // Remove the row we just added
    document_editing_controller::remove_table_row(
        &db,
        &hub,
        &mut urm,
        None,
        &RemoveTableRowDto {
            table_id,
            row_index: 1,
        },
    )?;

    // Table should be back to 2x2
    let table = table_controller::get(&db, &(table_id as u64))?.unwrap();
    assert_eq!(table.rows, 2);
    assert_eq!(table.columns, 2);

    Ok(())
}
