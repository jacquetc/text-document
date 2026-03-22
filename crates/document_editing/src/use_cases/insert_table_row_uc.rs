use crate::InsertTableRowDto;
use crate::InsertTableRowResultDto;
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

pub trait InsertTableRowUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn InsertTableRowUnitOfWorkTrait>;
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
#[macros::uow_action(entity = "Table", action = "Update")]
#[macros::uow_action(entity = "Table", action = "GetRelationship")]
#[macros::uow_action(entity = "TableCell", action = "GetMulti")]
#[macros::uow_action(entity = "TableCell", action = "Create")]
#[macros::uow_action(entity = "TableCell", action = "UpdateMulti")]
pub trait InsertTableRowUnitOfWorkTrait: CommandUnitOfWork {}

pub struct InsertTableRowUseCase {
    uow_factory: Box<dyn InsertTableRowUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<InsertTableRowDto>,
}

fn execute_insert_table_row(
    uow: &mut Box<dyn InsertTableRowUnitOfWorkTrait>,
    dto: &InsertTableRowDto,
) -> Result<(InsertTableRowResultDto, EntityTreeSnapshot)> {
    let table_id = dto.table_id as EntityId;
    let now = chrono::Utc::now();

    // Get Root -> Document
    let root = uow
        .get_root(&ROOT_ENTITY_ID)?
        .ok_or_else(|| anyhow!("Root not found"))?;
    let doc_ids = uow.get_root_relationship(&root.id, &RootRelationshipField::Document)?;
    let doc_id = *doc_ids.first().ok_or_else(|| anyhow!("No document"))?;
    let document = uow
        .get_document(&doc_id)?
        .ok_or_else(|| anyhow!("Document not found"))?;

    let table = uow
        .get_table(&table_id)?
        .ok_or_else(|| anyhow!("Table not found"))?;

    if dto.row_index < 0 || dto.row_index > table.rows {
        return Err(anyhow!(
            "Row index {} out of range [0, {}]",
            dto.row_index,
            table.rows
        ));
    }

    let snapshot = uow.snapshot_document(&[doc_id])?;

    // Get existing cells and compute base_pos BEFORE creating new cells
    let cell_ids = uow.get_table_relationship(&table_id, &TableRelationshipField::Cells)?;
    let cells_opt = uow.get_table_cell_multi(&cell_ids)?;
    let cells: Vec<TableCell> = cells_opt.into_iter().flatten().collect();

    // Find base position from existing cell blocks (must be done before new cells are created)
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

    let mut cells_to_shift: Vec<TableCell> = Vec::new();
    for cell in &cells {
        if cell.row >= dto.row_index {
            let mut shifted = cell.clone();
            shifted.row += 1;
            shifted.updated_at = now;
            cells_to_shift.push(shifted);
        }
    }
    if !cells_to_shift.is_empty() {
        uow.update_table_cell_multi(&cells_to_shift)?;
    }

    // Create new cells for the inserted row
    for c in 0..table.columns {
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

        let cell = TableCell {
            id: 0,
            created_at: now,
            updated_at: now,
            row: dto.row_index,
            column: c,
            row_span: 1,
            column_span: 1,
            cell_frame: Some(created_frame.id),
            fmt_padding: None,
            fmt_border: None,
            fmt_vertical_alignment: None,
            fmt_background_color: None,
        };
        uow.create_table_cell(&cell, table_id, -1)?;
    }

    // Update table row count
    let mut updated_table = table.clone();
    updated_table.rows += 1;
    updated_table.updated_at = now;
    uow.update_table(&updated_table)?;

    // Recalculate document positions for all table cell blocks (using base_pos computed earlier)
    let all_cell_ids = uow.get_table_relationship(&table_id, &TableRelationshipField::Cells)?;
    let all_cells_opt = uow.get_table_cell_multi(&all_cell_ids)?;
    let mut all_cells: Vec<TableCell> = all_cells_opt.into_iter().flatten().collect();
    all_cells.sort_by(|a, b| a.row.cmp(&b.row).then(a.column.cmp(&b.column)));

    let cell_frame_ids: Vec<EntityId> = all_cells.iter().filter_map(|c| c.cell_frame).collect();

    // Assign positions in row-major order
    let mut cell_blocks_to_update: Vec<Block> = Vec::new();
    for (i, cell) in all_cells.iter().enumerate() {
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

    // Shift non-table blocks after the table
    let added_cells = table.columns;
    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
    let cell_frame_set: std::collections::HashSet<EntityId> = cell_frame_ids.into_iter().collect();
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

    // Update Document stats
    let mut updated_doc = document.clone();
    updated_doc.block_count += table.columns;
    updated_doc.updated_at = now;
    uow.update_document(&updated_doc)?;

    Ok((
        InsertTableRowResultDto {
            new_row_count: updated_table.rows,
        },
        snapshot,
    ))
}

impl InsertTableRowUseCase {
    pub fn new(uow_factory: Box<dyn InsertTableRowUnitOfWorkFactoryTrait>) -> Self {
        InsertTableRowUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &InsertTableRowDto) -> Result<InsertTableRowResultDto> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;
        let (result, snapshot) = execute_insert_table_row(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());
        uow.commit()?;
        Ok(result)
    }
}

impl UndoRedoCommand for InsertTableRowUseCase {
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
        let (_, snapshot) = execute_insert_table_row(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
