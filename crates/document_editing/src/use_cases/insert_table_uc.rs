use crate::InsertTableDto;
use crate::InsertTableResultDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::database::rope_helpers::{
    block_char_length, rope_insert_block_at, rope_insert_table_anchor, top_level_frame_end_byte,
};
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::entities::{Block, Document, Frame, Root, Table, TableCell};
use common::snapshot::EntityTreeSnapshot;
use common::types::{EntityId, ROOT_ENTITY_ID};
use common::undo_redo::UndoRedoCommand;
use std::any::Any;

use super::editing_helpers::{
    CellFrameCreator, create_cell_frame, find_block_at_position, impl_cell_frame_creator,
};

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
#[macros::uow_action(entity = "Table", action = "Create")]
#[macros::uow_action(entity = "TableCell", action = "Create")]
pub trait InsertTableUnitOfWorkTrait: CommandUnitOfWork {}

impl_cell_frame_creator!(dyn InsertTableUnitOfWorkTrait);

pub struct InsertTableUseCase {
    uow_factory: Box<dyn InsertTableUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<InsertTableDto>,
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

    // Find the frame containing the insertion position. Also remember
    // the cursor-adjacent block id (None for empty docs) so the rope
    // mirror knows where to place the U+FFFC sentinel, and compute
    // `cell_start_pos` — the document-position-space coordinate where
    // the first cell will live, *matching what `snapshot_from_child_order`
    // will compute* once the table anchor is inserted into the parent
    // frame's child_order. That's the critical invariant: cell
    // `document_position` values must equal the snapshot's running_pos,
    // otherwise lookups like `insert_text` (which sort blocks by
    // `document_position`) drift from the cursor positions the
    // rendered snapshot reports.
    let (parent_frame_id, child_order_insert_idx, rope_anchor, cell_start_pos): (
        EntityId,
        usize,
        Option<(EntityId, bool)>,
        i64,
    ) = if all_blocks.is_empty() {
        // Empty document — use the first frame. No host block, so the
        // table goes first in child_order; cells start at the cursor
        // position (which is 0 in a fresh empty doc).
        let first_frame_id = frame_ids
            .first()
            .ok_or_else(|| anyhow!("Document has no frames"))?;
        (*first_frame_id, 0usize, None, insert_pos)
    } else {
        let (target_block, _, offset) =
            find_block_at_position(&all_blocks, insert_pos, &uow.store())?;
        let target_length = block_char_length(&target_block, &uow.store());

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
        // When at the very start of the block (offset == 0), place the
        // table before it so the table can be the first flow element.
        // Otherwise (cursor anywhere inside or at the end of the host
        // block) the table goes after the whole host block — it is
        // NOT split — so cell positions follow the host block's full
        // extent, regardless of how far in the cursor sits.
        let after = offset > 0;
        let after_idx = if after { 1 } else { 0 };
        let cell_start = if after {
            // Cells live one boundary past the host block's end.
            target_block.document_position + target_length + 1
        } else {
            // Cells take the host block's current spot; the host
            // shifts to past the cells (handled by the shift loop
            // below at `document_position >= insert_pos`).
            target_block.document_position
        };
        (
            found_frame_id,
            found_block_idx + after_idx,
            Some((target_block.id, after)),
            cell_start,
        )
    };

    // 1. Create the Table entity (owned by Document)
    let table = Table {
        id: 0,
        created_at: now,
        updated_at: now,
        cells: vec![],
        rows: dto.rows,
        columns: dto.columns,
        column_widths: vec![0; dto.columns as usize],
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
        fmt_is_blockquote: None,
        table: Some(created_table.id),
        byte_range: (0, 0),
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

    // Mirror the table-anchor into the global rope (no-op under
    // default backend). Inserts a U+FFFC sentinel + boundary newline,
    // registers a TableAnchor(table_id) marker in the offset index.
    // Cell-internal content stays in Block.plain_text for now —
    // plan §1.6's Frame.byte_range model is a follow-up commit.
    if let Some((target_block_id, after)) = rope_anchor {
        rope_insert_table_anchor(&uow.store(), created_table.id, target_block_id, after);
    }

    // 4. Assign document_position to all cell blocks in row-major
    // order. `cell_start_pos` was computed up front to match exactly
    // what `snapshot_from_child_order` (text_frame.rs) will report
    // for the first cell once the table anchor is part of the parent
    // frame's child_order. Each subsequent empty cell adds 1 (the
    // per-cell boundary) so cells occupy a contiguous run of N
    // positions starting at `cell_start_pos`.
    //
    // The two cases that contribute to `cell_start_pos` correctness:
    //
    //   * "after" insertion (cursor offset > 0 within the host block):
    //     the table goes *after* the whole host block in child_order
    //     — it is not split. So cells must start at
    //     `host.document_position + host.length + 1`, not at
    //     `insert_pos + 1`. The two are equal only when the cursor
    //     is exactly at the end of the host block (offset == length);
    //     elsewhere, `insert_pos + 1` lands inside the host's
    //     document-position range, putting cell positions out of
    //     sync with the snapshot the user sees.
    //
    //   * "before" insertion (cursor offset == 0) and empty doc:
    //     cells take the cursor's position; the displaced host
    //     block (if any) shifts to past the cells via the
    //     `document_position >= insert_pos` loop below.
    //
    // Tests: `inserted_3x3_typing_in_each_cell_lands_in_that_cell`
    // (after-at-end-of-block), `inserted_2x2_at_start_of_block_lands_before_it`
    // (before), `inserted_2x2_in_empty_document_starts_at_zero` (empty),
    // `inserted_2x2_deep_in_long_block_document_position_matches_snapshot`
    // (after-deep-in-block — the user-reported "table inserted before
    // the current block" regression, where document_position drifted
    // from snapshot in proportion to `host.length - offset`).
    let mut blocks_to_update: Vec<Block> = Vec::new();
    for (offset, cell_block) in cell_blocks.iter().enumerate() {
        let mut updated_block = cell_block.clone();
        updated_block.document_position = cell_start_pos + offset as i64;
        updated_block.updated_at = now;
        blocks_to_update.push(updated_block);
    }
    if !blocks_to_update.is_empty() {
        uow.update_block_multi(&blocks_to_update)?;
    }

    // Mirror cell-block creation into the global rope. Per plan §1.6
    // cells of a table live AT THE END of the containing top-level
    // frame's rope range — ideally BEFORE any following top-level
    // frame's content. Each cell is preceded by a `\n` boundary.
    //
    // The anchor sentinel was just inserted, so the top-level frame's
    // current end byte (computed fresh from block_offsets, not from
    // the stale Frame.byte_range) already includes that sentinel.
    //
    // LIMITATION (deferred): when a following top-level frame's content
    // coincides with frame 1's end byte (typical when the following
    // frame's first block is empty), `rope_insert_block_at`'s shift
    // semantics leave the following entry at the colliding byte and
    // place the cell at byte_pos+1 — so cells end up AFTER the
    // following frame in the rope. Real public-API workloads do not
    // currently produce multi-top-level-frame docs, so this layout
    // anomaly is invisible to users.
    //
    // No-op under default backend.
    {
        let store = uow.store();
        let mut next_byte = top_level_frame_end_byte(&store, parent_frame_id);
        for cell_block in &cell_blocks {
            // Newly-created cells via `create_cell_frame` are empty,
            // so we know the content is "" — no need to read from the
            // store/entity (the block hasn't been registered in the
            // rope yet anyway).
            rope_insert_block_at(&store, next_byte, cell_block.id, "");
            next_byte += 1;
        }
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

    let new_position = insert_pos;

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
