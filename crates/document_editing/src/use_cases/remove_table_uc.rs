use crate::RemoveTableDto;
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

pub trait RemoveTableUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn RemoveTableUnitOfWorkTrait>;
}

#[macros::uow_action(entity = "Root", action = "Get")]
#[macros::uow_action(entity = "Root", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Get")]
#[macros::uow_action(entity = "Document", action = "Update")]
#[macros::uow_action(entity = "Document", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Snapshot")]
#[macros::uow_action(entity = "Document", action = "Restore")]
#[macros::uow_action(entity = "Frame", action = "Get")]
#[macros::uow_action(entity = "Frame", action = "GetMulti")]
#[macros::uow_action(entity = "Frame", action = "Update")]
#[macros::uow_action(entity = "Frame", action = "Remove")]
#[macros::uow_action(entity = "Frame", action = "GetRelationship")]
#[macros::uow_action(entity = "Block", action = "GetMulti")]
#[macros::uow_action(entity = "Block", action = "UpdateMulti")]
#[macros::uow_action(entity = "Table", action = "Get")]
#[macros::uow_action(entity = "Table", action = "Remove")]
#[macros::uow_action(entity = "Table", action = "GetRelationship")]
#[macros::uow_action(entity = "TableCell", action = "GetMulti")]
pub trait RemoveTableUnitOfWorkTrait: CommandUnitOfWork {}

pub struct RemoveTableUseCase {
    uow_factory: Box<dyn RemoveTableUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<RemoveTableDto>,
}

fn execute_remove_table(
    uow: &mut Box<dyn RemoveTableUnitOfWorkTrait>,
    dto: &RemoveTableDto,
) -> Result<EntityTreeSnapshot> {
    let table_id = dto.table_id as EntityId;

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

    // Verify the table exists
    let _table = uow
        .get_table(&table_id)?
        .ok_or_else(|| anyhow!("Table {} not found", table_id))?;

    // Snapshot for undo before mutation
    let snapshot = uow.snapshot_document(&[doc_id])?;

    let now = chrono::Utc::now();

    // Get the table's cells to find cell frames
    let cell_ids = uow.get_table_relationship(&table_id, &TableRelationshipField::Cells)?;
    let cells_opt = uow.get_table_cell_multi(&cell_ids)?;
    let cells: Vec<TableCell> = cells_opt.into_iter().flatten().collect();

    // Collect cell frame IDs
    let cell_frame_ids: Vec<EntityId> = cells.iter().filter_map(|c| c.cell_frame).collect();

    // Count how many cell blocks exist (for position shifting)
    let mut total_cell_blocks: i64 = 0;
    let mut min_cell_position: Option<i64> = None;
    for fid in &cell_frame_ids {
        let block_ids = uow.get_frame_relationship(fid, &FrameRelationshipField::Blocks)?;
        if !block_ids.is_empty() {
            let blocks_opt = uow.get_block_multi(&block_ids)?;
            for block in blocks_opt.into_iter().flatten() {
                total_cell_blocks += 1;
                match min_cell_position {
                    None => min_cell_position = Some(block.document_position),
                    Some(min) if block.document_position < min => {
                        min_cell_position = Some(block.document_position);
                    }
                    _ => {}
                }
            }
        }
    }

    // Find the anchor frame (frame with table == Some(table_id))
    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
    let mut anchor_frame_id: Option<EntityId> = None;
    for fid in &frame_ids {
        let frame = match uow.get_frame(fid)? {
            Some(f) => f,
            None => continue,
        };
        if frame.table == Some(table_id) {
            anchor_frame_id = Some(frame.id);
            break;
        }
    }

    // Remove cell frames (cascade removes their blocks and elements)
    for fid in &cell_frame_ids {
        uow.remove_frame(fid)?;
    }

    // Remove anchor frame
    if let Some(anchor_id) = anchor_frame_id {
        // First, remove the anchor from its parent frame's child_order
        let frames_opt = uow.get_frame_multi(&frame_ids)?;
        for frame_opt in &frames_opt {
            if let Some(frame) = frame_opt {
                let neg_anchor = -(anchor_id as i64);
                if frame.child_order.contains(&neg_anchor) {
                    let mut updated = frame.clone();
                    updated.child_order.retain(|&x| x != neg_anchor);
                    updated.updated_at = now;
                    uow.update_frame(&updated)?;
                    break;
                }
            }
        }
        uow.remove_frame(&anchor_id)?;
    }

    // Remove the table (cascade removes TableCells)
    uow.remove_table(&table_id)?;

    // Shift document_position for blocks after the removed table
    if let Some(table_start_pos) = min_cell_position {
        // Get all remaining blocks and shift those after the table
        let remaining_frame_ids =
            uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
        let mut blocks_to_shift: Vec<Block> = Vec::new();
        for fid in &remaining_frame_ids {
            let block_ids = uow.get_frame_relationship(fid, &FrameRelationshipField::Blocks)?;
            if !block_ids.is_empty() {
                let blocks_opt = uow.get_block_multi(&block_ids)?;
                for block in blocks_opt.into_iter().flatten() {
                    if block.document_position >= table_start_pos {
                        let mut shifted = block;
                        shifted.document_position -= total_cell_blocks;
                        shifted.updated_at = now;
                        blocks_to_shift.push(shifted);
                    }
                }
            }
        }
        if !blocks_to_shift.is_empty() {
            uow.update_block_multi(&blocks_to_shift)?;
        }
    }

    // Update Document stats
    let mut updated_doc = document.clone();
    updated_doc.block_count -= total_cell_blocks;
    updated_doc.updated_at = now;
    uow.update_document(&updated_doc)?;

    Ok(snapshot)
}

impl RemoveTableUseCase {
    pub fn new(uow_factory: Box<dyn RemoveTableUnitOfWorkFactoryTrait>) -> Self {
        RemoveTableUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &RemoveTableDto) -> Result<()> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let snapshot = execute_remove_table(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());

        uow.commit()?;
        Ok(())
    }
}

impl UndoRedoCommand for RemoveTableUseCase {
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
        let snapshot = execute_remove_table(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
