use super::editing_helpers::{
    collect_block_ids_recursive, find_block_at_position, is_word_boundary_punct,
};
use crate::DeleteTextDto;
use crate::DeleteTextResultDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::direct_access::block::block_repository::BlockRelationshipField;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::direct_access::table::TableRelationshipField;
use common::entities::{Block, Document, Frame, InlineContent, InlineElement, Root, TableCell};
use common::snapshot::EntityTreeSnapshot;
use common::types::{EntityId, ROOT_ENTITY_ID};
use common::undo_redo::UndoRedoCommand;
use std::any::Any;
use std::time::Instant;

pub trait DeleteTextUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn DeleteTextUnitOfWorkTrait>;
}

#[macros::uow_action(entity = "Root", action = "Get")]
#[macros::uow_action(entity = "Root", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Get")]
#[macros::uow_action(entity = "Document", action = "Update")]
#[macros::uow_action(entity = "Document", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Snapshot")]
#[macros::uow_action(entity = "Document", action = "Restore")]
#[macros::uow_action(entity = "Frame", action = "Get")]
#[macros::uow_action(entity = "Frame", action = "Update")]
#[macros::uow_action(entity = "Frame", action = "GetRelationship")]
#[macros::uow_action(entity = "Block", action = "Get")]
#[macros::uow_action(entity = "Block", action = "GetMulti")]
#[macros::uow_action(entity = "Block", action = "Update")]
#[macros::uow_action(entity = "Block", action = "UpdateMulti")]
#[macros::uow_action(entity = "Block", action = "Remove")]
#[macros::uow_action(entity = "Block", action = "GetRelationship")]
#[macros::uow_action(entity = "InlineElement", action = "Get")]
#[macros::uow_action(entity = "InlineElement", action = "GetMulti")]
#[macros::uow_action(entity = "InlineElement", action = "Update")]
#[macros::uow_action(entity = "InlineElement", action = "Create")]
#[macros::uow_action(entity = "InlineElement", action = "Remove")]
#[macros::uow_action(entity = "InlineElement", action = "RemoveMulti")]
#[macros::uow_action(entity = "Table", action = "GetRelationship")]
#[macros::uow_action(entity = "TableCell", action = "GetMulti")]
pub trait DeleteTextUnitOfWorkTrait: CommandUnitOfWork {}

pub struct DeleteTextUseCase {
    uow_factory: Box<dyn DeleteTextUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<DeleteTextDto>,
    last_result: Option<DeleteTextResultDto>,
    last_merge_time: Option<Instant>,
    is_single_char_origin: bool,
}

fn execute_delete(
    uow: &mut Box<dyn DeleteTextUnitOfWorkTrait>,
    dto: &DeleteTextDto,
) -> Result<(DeleteTextResultDto, EntityTreeSnapshot)> {
    if dto.position == dto.anchor {
        // No-op: nothing to delete, but we still need a snapshot for consistency
        // Actually, let's just return an empty result with a dummy snapshot
        let root = uow
            .get_root(&ROOT_ENTITY_ID)?
            .ok_or_else(|| anyhow!("Root entity not found"))?;
        let doc_ids = uow.get_root_relationship(&root.id, &RootRelationshipField::Document)?;
        let doc_id = *doc_ids
            .first()
            .ok_or_else(|| anyhow!("Root has no document"))?;
        let snapshot = uow.snapshot_document(&[doc_id])?;
        return Ok((
            DeleteTextResultDto {
                new_position: dto.position,
                deleted_text: String::new(),
            },
            snapshot,
        ));
    }

    let start = std::cmp::min(dto.position, dto.anchor);
    let end = std::cmp::max(dto.position, dto.anchor);

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

    // Get all block IDs in document order, traversing into nested frames
    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
    let frame_id = *frame_ids
        .first()
        .ok_or_else(|| anyhow!("Document has no frames"))?;

    let get_table_cell_frames = |table_id: &EntityId| -> anyhow::Result<Vec<EntityId>> {
        let cell_ids = uow.get_table_relationship(table_id, &TableRelationshipField::Cells)?;
        let cells_opt = uow.get_table_cell_multi(&cell_ids)?;
        let mut cells: Vec<_> = cells_opt.into_iter().flatten().collect();
        cells.sort_by(|a, b| a.row.cmp(&b.row).then(a.column.cmp(&b.column)));
        Ok(cells.into_iter().filter_map(|c| c.cell_frame).collect())
    };
    let all_block_ids = collect_block_ids_recursive(
        &|id| uow.get_frame(id),
        &|id, field| uow.get_frame_relationship(id, field),
        &get_table_cell_frames,
        &frame_id,
    )?;

    // Get all blocks
    let blocks_opt = uow.get_block_multi(&all_block_ids)?;
    let mut blocks: Vec<Block> = blocks_opt.into_iter().flatten().collect();
    blocks.sort_by_key(|b| b.document_position);

    // Find start and end blocks (used for cell detection, then re-used by the normal path)
    let (start_block, start_block_idx, start_offset) = find_block_at_position(&blocks, start)?;
    let (end_block_tmp, _end_block_idx_tmp, _end_offset_tmp) =
        find_block_at_position(&blocks, end)?;

    // ── Cell selection safety: detect cross-cell deletion ──────────
    // Build block_id → cell_frame_id map from all tables in the document.
    let table_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Tables)?;
    let mut block_to_cell_frame: std::collections::HashMap<EntityId, EntityId> =
        std::collections::HashMap::new();
    for &tid in &table_ids {
        let cell_ids = uow.get_table_relationship(&tid, &TableRelationshipField::Cells)?;
        let cells_opt = uow.get_table_cell_multi(&cell_ids)?;
        for cell in cells_opt.into_iter().flatten() {
            if let Some(cf_id) = cell.cell_frame {
                let blk_ids =
                    uow.get_frame_relationship(&cf_id, &FrameRelationshipField::Blocks)?;
                for bid in blk_ids {
                    block_to_cell_frame.insert(bid, cf_id);
                }
            }
        }
    }

    let start_cell = block_to_cell_frame.get(&start_block.id).copied();
    let end_cell = block_to_cell_frame.get(&end_block_tmp.id).copied();

    let is_cross_cell = match (start_cell, end_cell) {
        (Some(a), Some(b)) => a != b,              // different cells
        (Some(_), None) | (None, Some(_)) => true, // one in table, one outside
        (None, None) => false,                     // both outside tables
    };

    if is_cross_cell {
        // Cell selection mode: clear the contents of all affected cells instead
        // of merging blocks across cell boundaries (which corrupts structure).
        let now = chrono::Utc::now();
        let mut total_chars_removed: i64 = 0;

        // Collect all unique cell frames whose blocks fall in [start..end]
        let mut affected_cell_frames: Vec<EntityId> = Vec::new();
        for block in &blocks {
            if block.document_position + block.text_length >= start
                && block.document_position <= end
                && let Some(&cf_id) = block_to_cell_frame.get(&block.id)
                && !affected_cell_frames.contains(&cf_id)
            {
                affected_cell_frames.push(cf_id);
            }
        }

        // Clear each affected cell frame: keep first block, empty it, remove the rest
        for cf_id in &affected_cell_frames {
            let frame = uow
                .get_frame(cf_id)?
                .ok_or_else(|| anyhow!("Cell frame not found"))?;
            let blk_ids = uow.get_frame_relationship(cf_id, &FrameRelationshipField::Blocks)?;
            let blk_opts = uow.get_block_multi(&blk_ids)?;
            let mut cell_blocks: Vec<Block> = blk_opts.into_iter().flatten().collect();
            cell_blocks.sort_by_key(|b| b.document_position);

            if cell_blocks.is_empty() {
                continue;
            }

            // Sum text to remove from this cell
            let cell_chars: i64 = cell_blocks.iter().map(|b| b.text_length).sum();
            total_chars_removed += cell_chars;

            // Reset first block to empty
            let first_block = &mut cell_blocks[0];
            let elem_ids =
                uow.get_block_relationship(&first_block.id, &BlockRelationshipField::Elements)?;
            // Remove all existing elements
            if !elem_ids.is_empty() {
                uow.remove_inline_element_multi(&elem_ids)?;
            }
            // Create a single empty element
            let empty_elem = InlineElement {
                content: InlineContent::Empty,
                ..InlineElement::default()
            };
            uow.create_inline_element(&empty_elem, first_block.id, -1)?;

            // Update block to empty
            let mut updated = first_block.clone();
            updated.plain_text = String::new();
            updated.text_length = 0;
            updated.updated_at = now;
            uow.update_block(&updated)?;

            // Remove extra blocks
            let extra_block_ids: Vec<EntityId> = cell_blocks[1..].iter().map(|b| b.id).collect();
            for &eid in &extra_block_ids {
                let elem_ids =
                    uow.get_block_relationship(&eid, &BlockRelationshipField::Elements)?;
                if !elem_ids.is_empty() {
                    uow.remove_inline_element_multi(&elem_ids)?;
                }
                uow.remove_block(&eid)?;
            }

            // Update frame child_order to only contain the first block
            let mut updated_frame = frame.clone();
            updated_frame.child_order = vec![cell_blocks[0].id as i64];
            updated_frame.updated_at = now;
            uow.update_frame(&updated_frame)?;
        }

        // Update document character_count
        let mut updated_doc = document.clone();
        updated_doc.character_count -= total_chars_removed;
        if updated_doc.character_count < 0 {
            updated_doc.character_count = 0;
        }
        updated_doc.updated_at = now;
        uow.update_document(&updated_doc)?;

        return Ok((
            DeleteTextResultDto {
                new_position: start,
                deleted_text: String::new(), // We don't reconstruct the text for cell clear
            },
            snapshot,
        ));
    }
    // ── End cell selection safety ──────────────────────────────────

    let (end_block, end_block_idx, end_offset) = find_block_at_position(&blocks, end)?;
    let delete_len = end - start;

    if start_block_idx == end_block_idx {
        // Same block: simple case
        // Get elements for this block
        let element_ids =
            uow.get_block_relationship(&start_block.id, &BlockRelationshipField::Elements)?;
        let elements_opt = uow.get_inline_element_multi(&element_ids)?;
        let elements: Vec<InlineElement> = elements_opt.into_iter().flatten().collect();

        // Walk elements: update/neutralize in delete range, rebuild cached fields
        let mut deleted_text = String::new();
        let mut new_plain_text = String::new();
        let mut new_text_length: i64 = 0;
        let mut running_offset: i64 = 0;
        for elem in &elements {
            let elem_len = match &elem.content {
                InlineContent::Text(s) => s.chars().count() as i64,
                InlineContent::Image { .. } => 1,
                InlineContent::Empty => 0,
            };
            let elem_start = running_offset;
            let elem_end = running_offset + elem_len;

            // Check overlap with [start_offset, end_offset)
            let overlap_start = std::cmp::max(start_offset, elem_start);
            let overlap_end = std::cmp::min(end_offset, elem_end);

            if overlap_start < overlap_end {
                let local_start = (overlap_start - elem_start) as usize;
                let local_end = (overlap_end - elem_start) as usize;

                match &elem.content {
                    InlineContent::Text(s) => {
                        let chars: Vec<char> = s.chars().collect();
                        // Collect deleted text
                        let removed: String = chars[local_start..local_end].iter().collect();
                        deleted_text.push_str(&removed);
                        // Build surviving text
                        let new_text: String = chars[..local_start]
                            .iter()
                            .chain(chars[local_end..].iter())
                            .collect();
                        new_plain_text.push_str(&new_text);
                        new_text_length += new_text.chars().count() as i64;
                        let mut updated_elem = elem.clone();
                        updated_elem.content = InlineContent::Text(new_text);
                        updated_elem.updated_at = chrono::Utc::now();
                        uow.update_inline_element(&updated_elem)?;
                    }
                    InlineContent::Image { .. } => {
                        // Image in delete range — neutralize
                        let mut updated_elem = elem.clone();
                        updated_elem.content = InlineContent::Empty;
                        updated_elem.updated_at = chrono::Utc::now();
                        uow.update_inline_element(&updated_elem)?;
                    }
                    InlineContent::Empty => {}
                }
            } else {
                // Not in delete range — preserve
                match &elem.content {
                    InlineContent::Text(s) => {
                        new_plain_text.push_str(s);
                        new_text_length += s.chars().count() as i64;
                    }
                    InlineContent::Image { .. } => {
                        new_text_length += 1;
                    }
                    InlineContent::Empty => {}
                }
            }

            running_offset += elem_len;
        }

        let _positions_removed = start_block.text_length - new_text_length;

        // Update block cached fields from element content
        let mut updated_block = start_block.clone();
        updated_block.plain_text = new_plain_text;
        updated_block.text_length = new_text_length;
        updated_block.updated_at = chrono::Utc::now();
        uow.update_block(&updated_block)?;

        // Update subsequent blocks' document_position
        let mut blocks_to_update: Vec<Block> = Vec::new();
        for b in &blocks[(start_block_idx + 1)..] {
            let mut ub = b.clone();
            ub.document_position -= delete_len;
            ub.updated_at = chrono::Utc::now();
            blocks_to_update.push(ub);
        }
        if !blocks_to_update.is_empty() {
            uow.update_block_multi(&blocks_to_update)?;
        }

        // Update Document
        let mut updated_doc = document.clone();
        updated_doc.character_count -= delete_len;
        updated_doc.updated_at = chrono::Utc::now();
        uow.update_document(&updated_doc)?;

        Ok((
            DeleteTextResultDto {
                new_position: start,
                deleted_text,
            },
            snapshot,
        ))
    } else {
        // Cross-block deletion: handle block merging
        // Build deleted_text, start_remaining, end_remaining from element content
        // (not from plain_text slicing, which breaks when images are present)

        let now = chrono::Utc::now();
        let so = start_offset as usize;
        let eo = end_offset as usize;

        // Update start block's inline elements: truncate at the delete start offset
        let start_element_ids =
            uow.get_block_relationship(&start_block.id, &BlockRelationshipField::Elements)?;
        let start_elements_opt = uow.get_inline_element_multi(&start_element_ids)?;
        let start_elements: Vec<InlineElement> = start_elements_opt.into_iter().flatten().collect();

        let mut start_remaining = String::new();
        let mut start_surviving_images: i64 = 0;
        let mut deleted_text = String::new();

        // Walk start block elements to truncate at start_offset
        let mut char_cursor: usize = 0;
        let mut truncation_done = false;
        for elem in &start_elements {
            let elem_char_len = match &elem.content {
                InlineContent::Text(s) => s.chars().count(),
                InlineContent::Image { .. } => 1,
                InlineContent::Empty => 0,
            };

            if !truncation_done {
                if char_cursor + elem_char_len <= so {
                    // Entirely before delete point — keep
                    match &elem.content {
                        InlineContent::Text(s) => start_remaining.push_str(s),
                        InlineContent::Image { .. } => start_surviving_images += 1,
                        InlineContent::Empty => {}
                    }
                    char_cursor += elem_char_len;
                    continue;
                }
                // This element contains the delete start
                truncation_done = true;
                let local_cut = so - char_cursor;
                match &elem.content {
                    InlineContent::Text(s) => {
                        let chars: Vec<char> = s.chars().collect();
                        let kept: String = chars[..local_cut].iter().collect();
                        deleted_text.extend(&chars[local_cut..]);
                        start_remaining.push_str(&kept);
                        let mut updated = elem.clone();
                        updated.content = InlineContent::Text(kept);
                        updated.updated_at = now;
                        uow.update_inline_element(&updated)?;
                    }
                    InlineContent::Image { .. } => {
                        // Image at delete boundary — neutralize
                        let mut cleared = elem.clone();
                        cleared.content = InlineContent::Empty;
                        cleared.updated_at = now;
                        uow.update_inline_element(&cleared)?;
                    }
                    InlineContent::Empty => {}
                }
                char_cursor += elem_char_len;
            } else {
                // After the delete start — clear and collect deleted text
                if let InlineContent::Text(s) = &elem.content {
                    deleted_text.push_str(s);
                }
                let mut cleared = elem.clone();
                cleared.content = InlineContent::Text(String::new());
                cleared.updated_at = now;
                uow.update_inline_element(&cleared)?;
                char_cursor += elem_char_len;
            }
        }

        // Add intermediate blocks' text to deleted_text
        for b in &blocks[(start_block_idx + 1)..end_block_idx] {
            deleted_text.push('\n');
            deleted_text.push_str(&b.plain_text);
        }

        // Separator before end block
        deleted_text.push('\n');

        // Handle end block elements: keep content after end_offset, move to start block
        let end_element_ids =
            uow.get_block_relationship(&end_block.id, &BlockRelationshipField::Elements)?;
        let end_elements_opt = uow.get_inline_element_multi(&end_element_ids)?;
        let end_elements: Vec<InlineElement> = end_elements_opt.into_iter().flatten().collect();

        let mut end_remaining = String::new();
        let mut end_surviving_images: i64 = 0;

        let mut end_char_cursor: usize = 0;
        let mut past_delete = false;
        for elem in &end_elements {
            let elem_char_len = match &elem.content {
                InlineContent::Text(s) => s.chars().count(),
                InlineContent::Image { .. } => 1,
                InlineContent::Empty => 0,
            };

            if !past_delete {
                if end_char_cursor + elem_char_len <= eo {
                    // In delete range — collect deleted text
                    if let InlineContent::Text(s) = &elem.content {
                        deleted_text.push_str(s);
                    }
                    end_char_cursor += elem_char_len;
                    continue;
                }
                past_delete = true;
                let local_start = eo - end_char_cursor;
                match &elem.content {
                    InlineContent::Text(s) => {
                        let chars: Vec<char> = s.chars().collect();
                        // Collect deleted portion
                        let del: String = chars[..local_start].iter().collect();
                        deleted_text.push_str(&del);
                        // Keep the rest
                        if local_start < chars.len() {
                            let kept: String = chars[local_start..].iter().collect();
                            if !kept.is_empty() {
                                end_remaining.push_str(&kept);
                                let mut new_elem = elem.clone();
                                new_elem.id = 0;
                                new_elem.content = InlineContent::Text(kept);
                                new_elem.created_at = now;
                                new_elem.updated_at = now;
                                uow.create_inline_element(&new_elem, start_block.id, -1)?;
                            }
                        }
                    }
                    InlineContent::Image { .. } => {
                        if local_start == 0 {
                            // Image after delete boundary — keep
                            end_surviving_images += 1;
                            let mut new_elem = elem.clone();
                            new_elem.id = 0;
                            new_elem.created_at = now;
                            new_elem.updated_at = now;
                            uow.create_inline_element(&new_elem, start_block.id, -1)?;
                        }
                    }
                    _ => {}
                }
                end_char_cursor += elem_char_len;
            } else {
                // Entirely after delete — move to start block
                match &elem.content {
                    InlineContent::Text(s) => end_remaining.push_str(s),
                    InlineContent::Image { .. } => end_surviving_images += 1,
                    InlineContent::Empty => {}
                }
                let mut new_elem = elem.clone();
                new_elem.id = 0;
                new_elem.created_at = now;
                new_elem.updated_at = now;
                uow.create_inline_element(&new_elem, start_block.id, -1)?;
                end_char_cursor += elem_char_len;
            }
        }

        let merged_text = format!("{}{}", start_remaining, end_remaining);

        // Update start block cached fields from element content
        let mut updated_start = start_block.clone();
        updated_start.plain_text = merged_text.clone();
        updated_start.text_length =
            merged_text.chars().count() as i64 + start_surviving_images + end_surviving_images;
        updated_start.updated_at = now;
        uow.update_block(&updated_start)?;

        // Remove intermediate and end blocks
        let blocks_to_remove: Vec<EntityId> = blocks[(start_block_idx + 1)..=end_block_idx]
            .iter()
            .map(|b| b.id)
            .collect();
        let removed_count = blocks_to_remove.len() as i64;

        for block_id in &blocks_to_remove {
            uow.remove_block(block_id)?;
        }

        // Fetch the root frame to update its child_order
        let frame = uow
            .get_frame(&frame_id)?
            .ok_or_else(|| anyhow!("Frame not found"))?;
        let mut updated_frame = frame.clone();
        updated_frame
            .child_order
            .retain(|id| !blocks_to_remove.contains(&(*id as EntityId)));
        updated_frame.updated_at = chrono::Utc::now();
        uow.update_frame(&updated_frame)?;

        // Compute actual characters removed (text only, not block separators).
        // From start block: chars from start_offset to end of block
        let chars_from_start = start_block.text_length - start_offset;
        // Intermediate blocks: their full text_length
        let chars_from_middle: i64 = blocks[(start_block_idx + 1)..end_block_idx]
            .iter()
            .map(|b| b.text_length)
            .sum();
        // From end block: chars from 0 to end_offset
        let chars_from_end = end_offset;
        let chars_removed = chars_from_start + chars_from_middle + chars_from_end;

        // Update subsequent blocks' document_position
        // delete_len includes block separators; use it for position arithmetic
        let mut blocks_to_update: Vec<Block> = Vec::new();
        for b in &blocks[(end_block_idx + 1)..] {
            let mut ub = b.clone();
            ub.document_position -= delete_len;
            ub.updated_at = chrono::Utc::now();
            blocks_to_update.push(ub);
        }
        if !blocks_to_update.is_empty() {
            uow.update_block_multi(&blocks_to_update)?;
        }

        // Update Document
        let mut updated_doc = document.clone();
        updated_doc.character_count -= chars_removed;
        updated_doc.block_count -= removed_count;
        updated_doc.updated_at = chrono::Utc::now();
        uow.update_document(&updated_doc)?;

        Ok((
            DeleteTextResultDto {
                new_position: start,
                deleted_text,
            },
            snapshot,
        ))
    }
}

impl DeleteTextUseCase {
    pub fn new(uow_factory: Box<dyn DeleteTextUnitOfWorkFactoryTrait>) -> Self {
        DeleteTextUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
            last_result: None,
            last_merge_time: None,
            is_single_char_origin: false,
        }
    }

    pub fn execute(&mut self, dto: &DeleteTextDto) -> Result<DeleteTextResultDto> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let (result, snapshot) = execute_delete(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());
        self.last_result = Some(result.clone());
        self.last_merge_time = Some(Instant::now());
        self.is_single_char_origin = (dto.position - dto.anchor).abs() == 1;

        uow.commit()?;
        Ok(result)
    }
}

impl UndoRedoCommand for DeleteTextUseCase {
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
        let (_, snapshot) = execute_delete(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn can_merge(&self, other: &dyn UndoRedoCommand) -> bool {
        let Some(other_cmd) = other.as_any().downcast_ref::<DeleteTextUseCase>() else {
            return false;
        };

        let (Some(self_dto), Some(self_result), Some(self_time)) =
            (&self.last_dto, &self.last_result, &self.last_merge_time)
        else {
            return false;
        };
        let (Some(other_dto), Some(_other_result), Some(other_time)) = (
            &other_cmd.last_dto,
            &other_cmd.last_result,
            &other_cmd.last_merge_time,
        ) else {
            return false;
        };

        // Rule 1: Time limit — 2 seconds
        if other_time.duration_since(*self_time) > std::time::Duration::from_secs(2) {
            return false;
        }

        // Rule 2: Both must originate from single-char deletes
        if !self.is_single_char_origin {
            return false;
        }
        if (other_dto.position - other_dto.anchor).abs() != 1 {
            return false;
        }

        // Rule 3: Same direction (both backspace or both forward delete)
        let self_is_backspace = self_dto.position > self_dto.anchor;
        let other_is_backspace = other_dto.position > other_dto.anchor;
        if self_is_backspace != other_is_backspace {
            return false;
        }

        // Rule 4: Contiguity
        if self_is_backspace {
            // Backspace: new delete's upper end == previous cursor position
            if other_dto.position.max(other_dto.anchor) != self_result.new_position {
                return false;
            }
        } else {
            // Forward delete: new delete's lower end == previous cursor position
            if other_dto.position.min(other_dto.anchor) != self_result.new_position {
                return false;
            }
        }

        // Rule 5: Max merged length — 200 chars
        let self_range = (self_dto.position - self_dto.anchor).abs();
        if self_range + 1 > 200 {
            return false;
        }

        // Rule 6: Word boundary — break after deleting a space/punctuation
        if let Some(last_deleted_char) = self_result.deleted_text.chars().next()
            && (last_deleted_char.is_whitespace() || is_word_boundary_punct(last_deleted_char))
        {
            return false;
        }

        true
    }

    fn merge(&mut self, other: &dyn UndoRedoCommand) -> bool {
        let Some(other_cmd) = other.as_any().downcast_ref::<DeleteTextUseCase>() else {
            return false;
        };

        let Some(self_dto) = &self.last_dto else {
            return false;
        };
        let Some(other_result) = &other_cmd.last_result else {
            return false;
        };
        let Some(other_time) = &other_cmd.last_merge_time else {
            return false;
        };

        let self_is_backspace = self_dto.position > self_dto.anchor;

        // Extend the combined delete range by one char in the appropriate direction
        let combined_dto = if self_is_backspace {
            // Backspace: extend anchor backward (toward smaller positions)
            DeleteTextDto {
                position: self_dto.position,
                anchor: self_dto.anchor - 1,
            }
        } else {
            // Forward delete: extend anchor forward (toward larger positions)
            DeleteTextDto {
                position: self_dto.position,
                anchor: self_dto.anchor + 1,
            }
        };

        // Keep self.undo_snapshot — state before the deletion burst started
        self.last_dto = Some(combined_dto);
        self.last_result = Some(other_result.clone());
        self.last_merge_time = Some(*other_time);

        true
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
