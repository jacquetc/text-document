use crate::RemoveTableColumnDto;
use crate::RemoveTableColumnResultDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::direct_access::table::table_repository::TableRelationshipField;
use common::entities::{Block, Document, Frame, Root, Table, TableCell};
use common::snapshot::EntityTreeSnapshot;
use common::types::{EntityId, ROOT_ENTITY_ID};
use common::undo_redo::UndoRedoCommand;
use std::any::Any;

pub trait RemoveTableColumnUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn RemoveTableColumnUnitOfWorkTrait>;
}

#[macros::uow_action(entity = "Root", action = "Get")]
#[macros::uow_action(entity = "Root", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Get")]
#[macros::uow_action(entity = "Document", action = "Update")]
#[macros::uow_action(entity = "Document", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Snapshot")]
#[macros::uow_action(entity = "Document", action = "Restore")]
#[macros::uow_action(entity = "Frame", action = "GetMulti")]
#[macros::uow_action(entity = "Frame", action = "Remove")]
#[macros::uow_action(entity = "Frame", action = "GetRelationship")]
#[macros::uow_action(entity = "Block", action = "GetMulti")]
#[macros::uow_action(entity = "Block", action = "UpdateMulti")]
#[macros::uow_action(entity = "Table", action = "Get")]
#[macros::uow_action(entity = "Table", action = "Update")]
#[macros::uow_action(entity = "Table", action = "GetRelationship")]
#[macros::uow_action(entity = "TableCell", action = "GetMulti")]
#[macros::uow_action(entity = "TableCell", action = "Remove")]
#[macros::uow_action(entity = "TableCell", action = "RemoveMulti")]
#[macros::uow_action(entity = "TableCell", action = "UpdateMulti")]
pub trait RemoveTableColumnUnitOfWorkTrait: CommandUnitOfWork {}

pub struct RemoveTableColumnUseCase {
    uow_factory: Box<dyn RemoveTableColumnUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<RemoveTableColumnDto>,
}

fn execute_remove_table_column(
    uow: &mut Box<dyn RemoveTableColumnUnitOfWorkTrait>,
    dto: &RemoveTableColumnDto,
) -> Result<(RemoveTableColumnResultDto, EntityTreeSnapshot)> {
    let table_id = dto.table_id as EntityId;
    let column_index = dto.column_index;
    let now = chrono::Utc::now();

    // Get Root -> Document
    let root = uow
        .get_root(&ROOT_ENTITY_ID)?
        .ok_or_else(|| anyhow!("Root entity not found"))?;
    let doc_ids = uow.get_root_relationship(&root.id, &RootRelationshipField::Document)?;
    let doc_id = *doc_ids
        .first()
        .ok_or_else(|| anyhow!("Root has no document"))?;
    let document = uow
        .get_document(&doc_id)?
        .ok_or_else(|| anyhow!("Document not found"))?;

    // Get the table
    let table = uow
        .get_table(&table_id)?
        .ok_or_else(|| anyhow!("Table {} not found", table_id))?;

    // Validate: table must have more than 1 column
    if table.columns <= 1 {
        return Err(anyhow!(
            "Cannot remove column: table must have more than 1 column"
        ));
    }

    if column_index < 0 || column_index >= table.columns {
        return Err(anyhow!(
            "Column index {} out of range [0, {})",
            column_index,
            table.columns
        ));
    }

    // Snapshot document for undo
    let snapshot = uow.snapshot_document(&[doc_id])?;

    // Get all cells in the table
    let cell_ids = uow.get_table_relationship(&table_id, &TableRelationshipField::Cells)?;
    let cells_opt = uow.get_table_cell_multi(&cell_ids)?;
    let cells: Vec<TableCell> = cells_opt.into_iter().flatten().collect();

    // Get cells in the target column (where cell.column == column_index)
    let column_cells: Vec<&TableCell> = cells.iter().filter(|c| c.column == column_index).collect();

    // Collect cell frame IDs for the target column
    let column_cell_frame_ids: Vec<EntityId> =
        column_cells.iter().filter_map(|c| c.cell_frame).collect();

    // Remove those cell frames (cascade removes blocks/elements)
    for fid in &column_cell_frame_ids {
        uow.remove_frame(fid)?;
    }

    // Remove the TableCell entities for the target column
    let column_cell_ids: Vec<EntityId> = column_cells.iter().map(|c| c.id).collect();
    uow.remove_table_cell_multi(&column_cell_ids)?;

    // Shift remaining cells with column > column_index (decrement column by 1)
    let mut cells_to_shift: Vec<TableCell> = Vec::new();
    for cell in &cells {
        if cell.column > column_index {
            let mut shifted = cell.clone();
            shifted.column -= 1;
            shifted.updated_at = now;
            cells_to_shift.push(shifted);
        }
    }
    if !cells_to_shift.is_empty() {
        uow.update_table_cell_multi(&cells_to_shift)?;
    }

    // Update table.columns
    let mut updated_table = table.clone();
    updated_table.columns -= 1;
    updated_table.updated_at = now;
    uow.update_table(&updated_table)?;

    // Recalculate document_positions for remaining cell blocks
    // Re-fetch all remaining cells after removal and shifting
    let remaining_cell_ids =
        uow.get_table_relationship(&table_id, &TableRelationshipField::Cells)?;
    let remaining_cells_opt = uow.get_table_cell_multi(&remaining_cell_ids)?;
    let mut remaining_cells: Vec<TableCell> = remaining_cells_opt.into_iter().flatten().collect();
    remaining_cells.sort_by(|a, b| a.row.cmp(&b.row).then(a.column.cmp(&b.column)));

    let remaining_cell_frame_ids: Vec<EntityId> = remaining_cells
        .iter()
        .filter_map(|c| c.cell_frame)
        .collect();

    // Find base position from existing cell blocks
    let mut base_pos: Option<i64> = None;
    for cf_id in &remaining_cell_frame_ids {
        let block_ids = uow.get_frame_relationship(cf_id, &FrameRelationshipField::Blocks)?;
        let blocks_opt = uow.get_block_multi(&block_ids)?;
        for block in blocks_opt.into_iter().flatten() {
            match base_pos {
                None => base_pos = Some(block.document_position),
                Some(bp) if block.document_position < bp => {
                    base_pos = Some(block.document_position);
                }
                _ => {}
            }
        }
    }
    let base_pos = base_pos.unwrap_or(0);

    // Assign positions in row-major order
    let mut cell_blocks_to_update: Vec<Block> = Vec::new();
    for (i, cell) in remaining_cells.iter().enumerate() {
        if let Some(cf_id) = cell.cell_frame {
            let block_ids = uow.get_frame_relationship(&cf_id, &FrameRelationshipField::Blocks)?;
            let blocks_opt = uow.get_block_multi(&block_ids)?;
            if let Some(Some(mut block)) = blocks_opt.into_iter().next() {
                block.document_position = base_pos + i as i64;
                block.updated_at = now;
                cell_blocks_to_update.push(block);
            }
        }
    }
    if !cell_blocks_to_update.is_empty() {
        uow.update_block_multi(&cell_blocks_to_update)?;
    }

    // Shift non-table blocks after the table (subtract table.rows positions)
    let removed_cells = table.rows; // one cell per row was removed
    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
    let cell_frame_set: std::collections::HashSet<EntityId> =
        remaining_cell_frame_ids.into_iter().collect();
    let mut shifted_blocks: Vec<Block> = Vec::new();
    for fid in &frame_ids {
        if cell_frame_set.contains(fid) {
            continue;
        }
        let block_ids = uow.get_frame_relationship(fid, &FrameRelationshipField::Blocks)?;
        if block_ids.is_empty() {
            continue;
        }
        let blocks_opt = uow.get_block_multi(&block_ids)?;
        for block in blocks_opt.into_iter().flatten() {
            if block.document_position >= base_pos {
                let mut shifted = block;
                shifted.document_position -= removed_cells;
                shifted.updated_at = now;
                shifted_blocks.push(shifted);
            }
        }
    }
    if !shifted_blocks.is_empty() {
        uow.update_block_multi(&shifted_blocks)?;
    }

    // Update Document.block_count (subtract table.rows)
    let mut updated_doc = document.clone();
    updated_doc.block_count -= table.rows;
    updated_doc.updated_at = now;
    uow.update_document(&updated_doc)?;

    Ok((
        RemoveTableColumnResultDto {
            new_column_count: updated_table.columns,
        },
        snapshot,
    ))
}

impl RemoveTableColumnUseCase {
    pub fn new(uow_factory: Box<dyn RemoveTableColumnUnitOfWorkFactoryTrait>) -> Self {
        RemoveTableColumnUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &RemoveTableColumnDto) -> Result<RemoveTableColumnResultDto> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;
        let (result, snapshot) = execute_remove_table_column(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());
        uow.commit()?;
        Ok(result)
    }
}

impl UndoRedoCommand for RemoveTableColumnUseCase {
    fn undo(&mut self) -> Result<()> {
        let snapshot = self
            .undo_snapshot
            .as_ref()
            .ok_or_else(|| anyhow!("No snapshot available for undo"))?
            .clone();

        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;
        uow.restore_document(&snapshot)?;
        uow.commit()?;
        Ok(())
    }

    fn redo(&mut self) -> Result<()> {
        let dto = self
            .last_dto
            .as_ref()
            .ok_or_else(|| anyhow!("No DTO available for redo"))?
            .clone();

        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;
        let (_, snapshot) = execute_remove_table_column(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
