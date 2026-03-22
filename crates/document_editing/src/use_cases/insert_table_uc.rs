use crate::InsertTableDto;
use crate::InsertTableResultDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::entities::{
    Block, Document, Frame, InlineContent, InlineElement, Root, Table, TableCell,
};
use common::snapshot::EntityTreeSnapshot;
use common::types::{EntityId, ROOT_ENTITY_ID};
use common::undo_redo::UndoRedoCommand;
use std::any::Any;

use super::editing_helpers::find_block_at_position;

pub trait InsertTableUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn InsertTableUnitOfWorkTrait>;
}

#[macros::uow_action(entity = "Root", action = "Get")]
#[macros::uow_action(entity = "Root", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Get")]
#[macros::uow_action(entity = "Document", action = "Update")]
#[macros::uow_action(entity = "Document", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Snapshot")]
#[macros::uow_action(entity = "Document", action = "Restore")]
#[macros::uow_action(entity = "Frame", action = "Get")]
#[macros::uow_action(entity = "Frame", action = "Create")]
#[macros::uow_action(entity = "Frame", action = "Update")]
#[macros::uow_action(entity = "Frame", action = "GetRelationship")]
#[macros::uow_action(entity = "Block", action = "GetMulti")]
#[macros::uow_action(entity = "Block", action = "Create")]
#[macros::uow_action(entity = "Block", action = "Update")]
#[macros::uow_action(entity = "Block", action = "UpdateMulti")]
#[macros::uow_action(entity = "InlineElement", action = "Create")]
#[macros::uow_action(entity = "Table", action = "Create")]
#[macros::uow_action(entity = "TableCell", action = "Create")]
pub trait InsertTableUnitOfWorkTrait: CommandUnitOfWork {}

pub struct InsertTableUseCase {
    uow_factory: Box<dyn InsertTableUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<InsertTableDto>,
}

/// Create a single cell Frame with one empty Block containing one empty InlineElement.
/// Returns the created frame's ID and the created block.
fn create_cell_frame(
    uow: &mut Box<dyn InsertTableUnitOfWorkTrait>,
    doc_id: EntityId,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(EntityId, Block)> {
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
        table: None, // Will not set table on cell frames — only the anchor frame gets it
    };
    let created_frame = uow.create_frame(&cell_frame, doc_id, -1)?;

    let block = Block {
        id: 0,
        created_at: now,
        updated_at: now,
        elements: vec![],
        list: None,
        text_length: 0,
        document_position: 0, // Will be set later
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

    // Update frame's child_order
    let mut updated_frame = created_frame.clone();
    updated_frame.child_order = vec![created_block.id as i64];
    updated_frame.updated_at = now;
    uow.update_frame(&updated_frame)?;

    Ok((created_frame.id, created_block))
}

fn execute_insert_table(
    uow: &mut Box<dyn InsertTableUnitOfWorkTrait>,
    dto: &InsertTableDto,
) -> Result<(InsertTableResultDto, EntityTreeSnapshot)> {
    if dto.rows < 1 || dto.columns < 1 {
        return Err(anyhow!("Table must have at least 1 row and 1 column"));
    }

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

    // Snapshot for undo before mutation
    let snapshot = uow.snapshot_document(&[doc_id])?;

    // Find the insertion position — determine the parent frame and where in child_order
    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;

    // Get all blocks across all frames to find the insertion point
    let mut all_blocks: Vec<Block> = Vec::new();
    for fid in &frame_ids {
        let block_ids = uow.get_frame_relationship(fid, &FrameRelationshipField::Blocks)?;
        if !block_ids.is_empty() {
            let blocks_opt = uow.get_block_multi(&block_ids)?;
            all_blocks.extend(blocks_opt.into_iter().flatten());
        }
    }
    all_blocks.sort_by_key(|b| b.document_position);

    // Resolve selection: use min position, delete selection if any
    let insert_pos = dto.position.min(dto.anchor);

    // Find the frame containing the insertion position
    let (parent_frame_id, child_order_insert_idx) = if all_blocks.is_empty() {
        // Empty document — use the first frame
        let first_frame_id = frame_ids
            .first()
            .ok_or_else(|| anyhow!("Document has no frames"))?;
        (*first_frame_id, 0usize)
    } else {
        let (target_block, _, _) = find_block_at_position(&all_blocks, insert_pos)?;
        // Find which frame owns this block
        let mut found_frame_id = frame_ids[0];
        let mut found_block_idx = 0usize;
        'outer: for fid in &frame_ids {
            let block_ids = uow.get_frame_relationship(fid, &FrameRelationshipField::Blocks)?;
            for (bi, bid) in block_ids.iter().enumerate() {
                if *bid == target_block.id {
                    found_frame_id = *fid;
                    found_block_idx = bi;
                    break 'outer;
                }
            }
        }
        (found_frame_id, found_block_idx + 1)
    };

    // 1. Create the Table entity (owned by Document)
    let table = Table {
        id: 0,
        created_at: now,
        updated_at: now,
        cells: vec![],
        rows: dto.rows,
        columns: dto.columns,
        column_widths: vec![],
        fmt_border: None,
        fmt_cell_spacing: None,
        fmt_cell_padding: None,
        fmt_width: None,
        fmt_alignment: None,
    };
    let created_table = uow.create_table(&table, doc_id, -1)?;

    // 2. Create cell frames and TableCells in row-major order
    let total_cells = dto.rows * dto.columns;
    let mut cell_blocks: Vec<Block> = Vec::with_capacity(total_cells as usize);

    for r in 0..dto.rows {
        for c in 0..dto.columns {
            // Create a cell frame with an empty block
            let (cell_frame_id, created_block) = create_cell_frame(uow, doc_id, now)?;

            cell_blocks.push(created_block);

            // Create the TableCell entity
            let cell = TableCell {
                id: 0,
                created_at: now,
                updated_at: now,
                row: r,
                column: c,
                row_span: 1,
                column_span: 1,
                cell_frame: Some(cell_frame_id),
                fmt_padding: None,
                fmt_border: None,
                fmt_vertical_alignment: None,
                fmt_background_color: None,
            };
            uow.create_table_cell(&cell, created_table.id, -1)?;
        }
    }

    // 3. Create the anchor frame (the frame that represents the table in the document flow)
    let anchor_frame = Frame {
        id: 0,
        created_at: now,
        updated_at: now,
        parent_frame: Some(parent_frame_id),
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
        table: Some(created_table.id),
    };
    let created_anchor = uow.create_frame(&anchor_frame, doc_id, -1)?;

    // Insert the anchor frame into the parent frame's child_order
    let parent_frame = uow
        .get_frame(&parent_frame_id)?
        .ok_or_else(|| anyhow!("Parent frame not found"))?;
    let mut updated_parent = parent_frame.clone();
    let idx = child_order_insert_idx.min(updated_parent.child_order.len());
    // Convention: negative = -(frame ID) for sub-frame references in child_order
    updated_parent
        .child_order
        .insert(idx, -(created_anchor.id as i64));
    updated_parent.updated_at = now;
    uow.update_frame(&updated_parent)?;

    // 4. Assign document_position to all cell blocks in row-major order
    // The table's blocks start at insert_pos, each cell block gets 1 position
    // (the separator character between blocks, like a newline)
    let mut current_pos = insert_pos;
    let mut blocks_to_update: Vec<Block> = Vec::new();
    for cell_block in &cell_blocks {
        let mut updated_block = cell_block.clone();
        updated_block.document_position = current_pos;
        updated_block.updated_at = now;
        blocks_to_update.push(updated_block);
        // Each empty block takes 1 position (the block separator)
        current_pos += 1;
    }
    if !blocks_to_update.is_empty() {
        uow.update_block_multi(&blocks_to_update)?;
    }

    // 5. Shift document_position for all blocks after the table
    let table_size = total_cells; // Each cell block occupies 1 position (empty block = separator)
    let mut shifted_blocks: Vec<Block> = Vec::new();
    for block in &all_blocks {
        if block.document_position >= insert_pos {
            let mut shifted = block.clone();
            shifted.document_position += table_size;
            shifted.updated_at = now;
            shifted_blocks.push(shifted);
        }
    }
    if !shifted_blocks.is_empty() {
        uow.update_block_multi(&shifted_blocks)?;
    }

    // 6. Update Document stats
    let mut updated_doc = document.clone();
    updated_doc.block_count += total_cells;
    updated_doc.updated_at = now;
    uow.update_document(&updated_doc)?;

    let new_position = insert_pos + table_size;

    Ok((
        InsertTableResultDto {
            table_id: created_table.id as i64,
            new_position,
        },
        snapshot,
    ))
}

impl InsertTableUseCase {
    pub fn new(uow_factory: Box<dyn InsertTableUnitOfWorkFactoryTrait>) -> Self {
        InsertTableUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &InsertTableDto) -> Result<InsertTableResultDto> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let (result, snapshot) = execute_insert_table(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());

        uow.commit()?;
        Ok(result)
    }
}

impl UndoRedoCommand for InsertTableUseCase {
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
        let (_, snapshot) = execute_insert_table(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
