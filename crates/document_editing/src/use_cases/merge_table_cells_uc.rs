use crate::MergeTableCellsDto;
use crate::MergeTableCellsResultDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::direct_access::table::table_repository::TableRelationshipField;
use common::entities::{Block, Document, Frame, InlineElement, Root, Table, TableCell};
use common::snapshot::EntityTreeSnapshot;
use common::types::{EntityId, ROOT_ENTITY_ID};
use common::undo_redo::UndoRedoCommand;
use std::any::Any;

pub trait MergeTableCellsUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn MergeTableCellsUnitOfWorkTrait>;
}

#[macros::uow_action(entity = "Root", action = "Get")]
#[macros::uow_action(entity = "Root", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Get")]
#[macros::uow_action(entity = "Document", action = "Update")]
#[macros::uow_action(entity = "Document", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Snapshot")]
#[macros::uow_action(entity = "Document", action = "Restore")]
#[macros::uow_action(entity = "Frame", action = "Get")]
#[macros::uow_action(entity = "Frame", action = "Remove")]
#[macros::uow_action(entity = "Frame", action = "GetRelationship")]
#[macros::uow_action(entity = "Block", action = "GetMulti")]
#[macros::uow_action(entity = "Block", action = "UpdateMulti")]
#[macros::uow_action(entity = "InlineElement", action = "GetMulti")]
#[macros::uow_action(entity = "Table", action = "Get")]
#[macros::uow_action(entity = "Table", action = "GetRelationship")]
#[macros::uow_action(entity = "TableCell", action = "GetMulti")]
#[macros::uow_action(entity = "TableCell", action = "Update")]
#[macros::uow_action(entity = "TableCell", action = "RemoveMulti")]
pub trait MergeTableCellsUnitOfWorkTrait: CommandUnitOfWork {}

pub struct MergeTableCellsUseCase {
    uow_factory: Box<dyn MergeTableCellsUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<MergeTableCellsDto>,
}

fn execute_merge_table_cells(
    uow: &mut Box<dyn MergeTableCellsUnitOfWorkTrait>,
    dto: &MergeTableCellsDto,
) -> Result<(MergeTableCellsResultDto, EntityTreeSnapshot)> {
    let table_id = dto.table_id as EntityId;
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

    // Get and validate table
    let table = uow
        .get_table(&table_id)?
        .ok_or_else(|| anyhow!("Table {} not found", table_id))?;

    // Validate range
    if dto.start_row > dto.end_row {
        return Err(anyhow!(
            "start_row ({}) must be <= end_row ({})",
            dto.start_row,
            dto.end_row
        ));
    }
    if dto.start_column > dto.end_column {
        return Err(anyhow!(
            "start_column ({}) must be <= end_column ({})",
            dto.start_column,
            dto.end_column
        ));
    }
    if dto.start_row < 0
        || dto.end_row >= table.rows
        || dto.start_column < 0
        || dto.end_column >= table.columns
    {
        return Err(anyhow!(
            "Merge range [{},{} - {},{}] out of table bounds ({}x{})",
            dto.start_row,
            dto.start_column,
            dto.end_row,
            dto.end_column,
            table.rows,
            table.columns
        ));
    }

    // Get all cells of this table
    let cell_ids = uow.get_table_relationship(&table_id, &TableRelationshipField::Cells)?;
    let cells_opt = uow.get_table_cell_multi(&cell_ids)?;
    let cells: Vec<TableCell> = cells_opt.into_iter().flatten().collect();

    // Identify cells in the merge range
    let cells_in_range: Vec<&TableCell> = cells
        .iter()
        .filter(|c| {
            c.row >= dto.start_row
                && c.row <= dto.end_row
                && c.column >= dto.start_column
                && c.column <= dto.end_column
        })
        .collect();

    // Validate: no cell in the range already has row_span > 1 or column_span > 1
    for cell in &cells_in_range {
        if cell.row_span > 1 || cell.column_span > 1 {
            return Err(anyhow!(
                "Cell at row={}, column={} already has span (row_span={}, column_span={}). Cannot merge already-merged cells.",
                cell.row,
                cell.column,
                cell.row_span,
                cell.column_span
            ));
        }
    }

    // Snapshot for undo before mutation
    let snapshot = uow.snapshot_document(&[doc_id])?;

    // Identify the surviving cell (top-left)
    let surviving_cell = cells_in_range
        .iter()
        .find(|c| c.row == dto.start_row && c.column == dto.start_column)
        .ok_or_else(|| {
            anyhow!(
                "No cell found at start position ({}, {})",
                dto.start_row,
                dto.start_column
            )
        })?;
    let surviving_cell_id = surviving_cell.id;

    // Cells to remove (all in range except the surviving cell)
    let cells_to_remove: Vec<&TableCell> = cells_in_range
        .iter()
        .filter(|c| c.id != surviving_cell_id)
        .copied()
        .collect();

    let removed_cell_count = cells_to_remove.len() as i64;

    // Find base_pos from existing cell blocks BEFORE any mutation
    let existing_cell_frame_ids: Vec<EntityId> =
        cells.iter().filter_map(|c| c.cell_frame).collect();
    let mut base_pos: Option<i64> = None;
    for cf_id in &existing_cell_frame_ids {
        let block_ids = uow.get_frame_relationship(cf_id, &FrameRelationshipField::Blocks)?;
        let blocks_opt = uow.get_block_multi(&block_ids)?;
        for block in blocks_opt.into_iter().flatten() {
            match base_pos {
                None => base_pos = Some(block.document_position),
                Some(bp) if block.document_position < bp => {
                    base_pos = Some(block.document_position)
                }
                _ => {}
            }
        }
    }
    let base_pos = base_pos.unwrap_or(0);

    // Remove the other cells' frames (cascade removes blocks/elements) and the cell entities
    let remove_frame_ids: Vec<EntityId> = cells_to_remove
        .iter()
        .filter_map(|c| c.cell_frame)
        .collect();
    for fid in &remove_frame_ids {
        uow.remove_frame(fid)?;
    }

    let remove_cell_ids: Vec<EntityId> = cells_to_remove.iter().map(|c| c.id).collect();
    if !remove_cell_ids.is_empty() {
        uow.remove_table_cell_multi(&remove_cell_ids)?;
    }

    // Update the surviving cell's row_span and column_span
    let mut updated_surviving = (*surviving_cell).clone();
    updated_surviving.row_span = dto.end_row - dto.start_row + 1;
    updated_surviving.column_span = dto.end_column - dto.start_column + 1;
    updated_surviving.updated_at = now;
    uow.update_table_cell(&updated_surviving)?;

    // Recalculate document_position for all remaining table cell blocks
    let remaining_cell_ids =
        uow.get_table_relationship(&table_id, &TableRelationshipField::Cells)?;
    let remaining_cells_opt = uow.get_table_cell_multi(&remaining_cell_ids)?;
    let mut remaining_cells: Vec<TableCell> = remaining_cells_opt.into_iter().flatten().collect();
    remaining_cells.sort_by(|a, b| a.row.cmp(&b.row).then(a.column.cmp(&b.column)));

    let remaining_frame_ids: Vec<EntityId> = remaining_cells
        .iter()
        .filter_map(|c| c.cell_frame)
        .collect();

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

    // Shift non-table blocks after the table (reduce by number of removed cells)
    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
    let cell_frame_set: std::collections::HashSet<EntityId> =
        remaining_frame_ids.into_iter().collect();
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
                shifted.document_position -= removed_cell_count;
                shifted.updated_at = now;
                shifted_blocks.push(shifted);
            }
        }
    }
    if !shifted_blocks.is_empty() {
        uow.update_block_multi(&shifted_blocks)?;
    }

    // Update Document.block_count
    let mut updated_doc = document.clone();
    updated_doc.block_count -= removed_cell_count;
    updated_doc.updated_at = now;
    uow.update_document(&updated_doc)?;

    Ok((
        MergeTableCellsResultDto {
            merged_cell_id: surviving_cell_id as i64,
        },
        snapshot,
    ))
}

impl MergeTableCellsUseCase {
    pub fn new(uow_factory: Box<dyn MergeTableCellsUnitOfWorkFactoryTrait>) -> Self {
        MergeTableCellsUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &MergeTableCellsDto) -> Result<MergeTableCellsResultDto> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;
        let (result, snapshot) = execute_merge_table_cells(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());
        uow.commit()?;
        Ok(result)
    }
}

impl UndoRedoCommand for MergeTableCellsUseCase {
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
        let (_, snapshot) = execute_merge_table_cells(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
