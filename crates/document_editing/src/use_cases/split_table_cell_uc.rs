use crate::SplitTableCellDto;
use crate::SplitTableCellResultDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::direct_access::table::table_repository::TableRelationshipField;
use common::entities::{
    Block, Document, Frame, InlineContent, InlineElement, Root, Table, TableCell,
};
use common::snapshot::EntityTreeSnapshot;
use common::types::{EntityId, ROOT_ENTITY_ID};
use common::undo_redo::UndoRedoCommand;
use std::any::Any;

pub trait SplitTableCellUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn SplitTableCellUnitOfWorkTrait>;
}

#[macros::uow_action(entity = "Root", action = "Get")]
#[macros::uow_action(entity = "Root", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Get")]
#[macros::uow_action(entity = "Document", action = "Update")]
#[macros::uow_action(entity = "Document", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Snapshot")]
#[macros::uow_action(entity = "Document", action = "Restore")]
#[macros::uow_action(entity = "Frame", action = "Create")]
#[macros::uow_action(entity = "Frame", action = "Update")]
#[macros::uow_action(entity = "Frame", action = "GetRelationship")]
#[macros::uow_action(entity = "Block", action = "Create")]
#[macros::uow_action(entity = "Block", action = "GetMulti")]
#[macros::uow_action(entity = "Block", action = "UpdateMulti")]
#[macros::uow_action(entity = "InlineElement", action = "Create")]
#[macros::uow_action(entity = "Table", action = "Get")]
#[macros::uow_action(entity = "Table", action = "GetRelationship")]
#[macros::uow_action(entity = "TableCell", action = "Get")]
#[macros::uow_action(entity = "TableCell", action = "GetMulti")]
#[macros::uow_action(entity = "TableCell", action = "Create")]
#[macros::uow_action(entity = "TableCell", action = "Update")]
pub trait SplitTableCellUnitOfWorkTrait: CommandUnitOfWork {}

pub struct SplitTableCellUseCase {
    uow_factory: Box<dyn SplitTableCellUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<SplitTableCellDto>,
}

fn execute_split_table_cell(
    uow: &mut Box<dyn SplitTableCellUnitOfWorkTrait>,
    dto: &SplitTableCellDto,
) -> Result<(SplitTableCellResultDto, EntityTreeSnapshot)> {
    let cell_id = dto.cell_id as EntityId;
    let now = chrono::Utc::now();

    // Validate split dimensions
    if dto.split_rows < 1 || dto.split_columns < 1 {
        return Err(anyhow!("split_rows and split_columns must be >= 1"));
    }

    // Get the cell
    let cell = uow
        .get_table_cell(&cell_id)?
        .ok_or_else(|| anyhow!("TableCell {} not found", cell_id))?;

    // Validate spans
    if cell.row_span < dto.split_rows {
        return Err(anyhow!(
            "Cannot split into {} rows: cell row_span is only {}",
            dto.split_rows,
            cell.row_span
        ));
    }
    if cell.column_span < dto.split_columns {
        return Err(anyhow!(
            "Cannot split into {} columns: cell column_span is only {}",
            dto.split_columns,
            cell.column_span
        ));
    }
    if cell.row_span == 1 && cell.column_span == 1 && dto.split_rows == 1 && dto.split_columns == 1
    {
        return Err(anyhow!(
            "Nothing to split: cell already has row_span=1, column_span=1 and split is 1x1"
        ));
    }

    // Get Root -> Document
    let root = uow
        .get_root(&ROOT_ENTITY_ID)?
        .ok_or_else(|| anyhow!("Root not found"))?;
    let doc_ids = uow.get_root_relationship(&root.id, &RootRelationshipField::Document)?;
    let doc_id = *doc_ids.first().ok_or_else(|| anyhow!("No document"))?;
    let document = uow
        .get_document(&doc_id)?
        .ok_or_else(|| anyhow!("Document not found"))?;

    // Find the table that owns this cell
    let table_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Tables)?;
    let mut owner_table_id: Option<EntityId> = None;
    for tid in &table_ids {
        let cids = uow.get_table_relationship(tid, &TableRelationshipField::Cells)?;
        if cids.contains(&cell_id) {
            owner_table_id = Some(*tid);
            break;
        }
    }
    let table_id = owner_table_id.ok_or_else(|| anyhow!("No table owns cell {}", cell_id))?;
    let _table = uow
        .get_table(&table_id)?
        .ok_or_else(|| anyhow!("Table not found"))?;

    // Snapshot for undo
    let snapshot = uow.snapshot_document(&[doc_id])?;

    // Get existing cells and compute base_pos BEFORE creating new cells
    let cell_ids = uow.get_table_relationship(&table_id, &TableRelationshipField::Cells)?;
    let cells_opt = uow.get_table_cell_multi(&cell_ids)?;
    let cells: Vec<TableCell> = cells_opt.into_iter().flatten().collect();

    // Find base position from existing cell blocks
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

    // Calculate sub-cell spans
    let base_row_span = cell.row_span / dto.split_rows;
    let extra_rows = cell.row_span % dto.split_rows;
    let base_col_span = cell.column_span / dto.split_columns;
    let extra_cols = cell.column_span % dto.split_columns;

    // Build list of sub-cell positions and spans
    let mut sub_cells: Vec<(i64, i64, i64, i64)> = Vec::new(); // (row, col, row_span, col_span)
    let mut current_row = cell.row;
    for ri in 0..dto.split_rows {
        let rs = base_row_span + if ri < extra_rows { 1 } else { 0 };
        let mut current_col = cell.column;
        for ci in 0..dto.split_columns {
            let cs = base_col_span + if ci < extra_cols { 1 } else { 0 };
            sub_cells.push((current_row, current_col, rs, cs));
            current_col += cs;
        }
        current_row += rs;
    }

    // The first sub-cell corresponds to the original cell
    // All others are new cells
    let mut all_cell_ids_result: Vec<i64> = Vec::new();

    // Update the original cell: set to first sub-cell position/span
    let (_, _, first_rs, first_cs) = sub_cells[0];
    let mut updated_cell = cell.clone();
    updated_cell.row_span = first_rs;
    updated_cell.column_span = first_cs;
    updated_cell.updated_at = now;
    uow.update_table_cell(&updated_cell)?;
    all_cell_ids_result.push(cell.id as i64);

    // Create new cells for remaining sub-cell positions
    for &(r, c, rs, cs) in &sub_cells[1..] {
        // Create frame for the new cell
        let cell_frame = Frame {
            id: 0,
            created_at: now,
            updated_at: now,
            parent_frame: None,
            blocks: vec![],
            child_order: vec![],
            fmt_height: None,
            fmt_width: None,
            fmt_top_margin: None,
            fmt_bottom_margin: None,
            fmt_left_margin: None,
            fmt_right_margin: None,
            fmt_padding: None,
            fmt_border: None,
            fmt_position: None,
            fmt_is_blockquote: None,
            table: None,
        };
        let created_frame = uow.create_frame(&cell_frame, doc_id, -1)?;

        let block = Block {
            id: 0,
            created_at: now,
            updated_at: now,
            elements: vec![],
            list: None,
            text_length: 0,
            document_position: 0,
            plain_text: String::new(),
            ..Default::default()
        };
        let created_block = uow.create_block(&block, created_frame.id, -1)?;

        let empty_elem = InlineElement {
            id: 0,
            created_at: now,
            updated_at: now,
            content: InlineContent::Empty,
            ..Default::default()
        };
        uow.create_inline_element(&empty_elem, created_block.id, -1)?;

        let mut updated_frame = created_frame.clone();
        updated_frame.child_order = vec![created_block.id as i64];
        updated_frame.updated_at = now;
        uow.update_frame(&updated_frame)?;

        let new_cell = TableCell {
            id: 0,
            created_at: now,
            updated_at: now,
            row: r,
            column: c,
            row_span: rs,
            column_span: cs,
            cell_frame: Some(created_frame.id),
            fmt_padding: None,
            fmt_border: None,
            fmt_vertical_alignment: None,
            fmt_background_color: None,
        };
        let created_cell = uow.create_table_cell(&new_cell, table_id, -1)?;
        all_cell_ids_result.push(created_cell.id as i64);
    }

    // Recalculate document_position for all cell blocks
    let all_cell_ids = uow.get_table_relationship(&table_id, &TableRelationshipField::Cells)?;
    let all_cells_opt = uow.get_table_cell_multi(&all_cell_ids)?;
    let mut all_cells: Vec<TableCell> = all_cells_opt.into_iter().flatten().collect();
    all_cells.sort_by(|a, b| a.row.cmp(&b.row).then(a.column.cmp(&b.column)));

    let cell_frame_ids: Vec<EntityId> = all_cells.iter().filter_map(|c| c.cell_frame).collect();

    let mut cell_blocks_to_update: Vec<Block> = Vec::new();
    for (i, ac) in all_cells.iter().enumerate() {
        if let Some(cf_id) = ac.cell_frame {
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

    // Shift non-table blocks after the table
    let added_cells = (sub_cells.len() as i64) - 1; // original cell already counted
    if added_cells > 0 {
        let frame_ids =
            uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
        let cell_frame_set: std::collections::HashSet<EntityId> =
            cell_frame_ids.into_iter().collect();
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
                    shifted.document_position += added_cells;
                    shifted.updated_at = now;
                    shifted_blocks.push(shifted);
                }
            }
        }
        if !shifted_blocks.is_empty() {
            uow.update_block_multi(&shifted_blocks)?;
        }
    }

    // Update Document stats
    let mut updated_doc = document.clone();
    updated_doc.block_count += (sub_cells.len() as i64) - 1;
    updated_doc.updated_at = now;
    uow.update_document(&updated_doc)?;

    Ok((
        SplitTableCellResultDto {
            new_cell_ids: all_cell_ids_result,
        },
        snapshot,
    ))
}

impl SplitTableCellUseCase {
    pub fn new(uow_factory: Box<dyn SplitTableCellUnitOfWorkFactoryTrait>) -> Self {
        SplitTableCellUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &SplitTableCellDto) -> Result<SplitTableCellResultDto> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;
        let (result, snapshot) = execute_split_table_cell(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());
        uow.commit()?;
        Ok(result)
    }
}

impl UndoRedoCommand for SplitTableCellUseCase {
    fn undo(&mut self) -> Result<()> {
        let snapshot = self
            .undo_snapshot
            .as_ref()
            .ok_or_else(|| anyhow!("No snapshot for undo"))?
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
            .ok_or_else(|| anyhow!("No DTO for redo"))?
            .clone();
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;
        let (_, snapshot) = execute_split_table_cell(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
