use super::editing_helpers::{collect_block_ids_recursive, find_block_at_position};
use crate::InsertBlockDto;
use crate::InsertBlockResultDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::direct_access::block::block_repository::BlockRelationshipField;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::entities::{Block, Document, Frame, InlineContent, InlineElement, Root};
use common::snapshot::EntityTreeSnapshot;
use common::types::{EntityId, ROOT_ENTITY_ID};
use common::undo_redo::UndoRedoCommand;
use std::any::Any;

pub trait InsertBlockUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn InsertBlockUnitOfWorkTrait>;
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
#[macros::uow_action(entity = "Block", action = "Create")]
#[macros::uow_action(entity = "Block", action = "GetRelationship")]
#[macros::uow_action(entity = "InlineElement", action = "Get")]
#[macros::uow_action(entity = "InlineElement", action = "GetMulti")]
#[macros::uow_action(entity = "InlineElement", action = "Update")]
#[macros::uow_action(entity = "InlineElement", action = "Create")]
pub trait InsertBlockUnitOfWorkTrait: CommandUnitOfWork {}

pub struct InsertBlockUseCase {
    uow_factory: Box<dyn InsertBlockUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<InsertBlockDto>,
}

fn execute_insert_block(
    uow: &mut Box<dyn InsertBlockUnitOfWorkTrait>,
    dto: &InsertBlockDto,
) -> Result<(InsertBlockResultDto, EntityTreeSnapshot)> {
    let position = dto.position;

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

    let all_block_ids = collect_block_ids_recursive(
        &|id| uow.get_frame(id),
        &|id, field| uow.get_frame_relationship(id, field),
        &frame_id,
    )?;

    // Get all blocks
    let blocks_opt = uow.get_block_multi(&all_block_ids)?;
    let mut blocks: Vec<Block> = blocks_opt.into_iter().flatten().collect();
    blocks.sort_by_key(|b| b.document_position);

    // Find block at position
    let (current_block, block_idx, offset) = find_block_at_position(&blocks, position)?;

    // Get elements for the current block
    let element_ids =
        uow.get_block_relationship(&current_block.id, &BlockRelationshipField::Elements)?;
    let elements_opt = uow.get_inline_element_multi(&element_ids)?;
    let elements: Vec<InlineElement> = elements_opt.into_iter().flatten().collect();

    // Split the block at the offset
    let plain_chars: Vec<char> = current_block.plain_text.chars().collect();
    let split_pos = (offset as usize).min(plain_chars.len());
    let text_before: String = plain_chars[..split_pos].iter().collect();
    let text_after: String = plain_chars[split_pos..].iter().collect();

    // Walk elements to find which element contains the split point, preserving formatting.
    // Elements before the split point stay in current block.
    // The element at the split point is truncated (keeping chars before the split).
    // Elements after the split point are cleared in the current block and will be
    // recreated in the new block.
    let now = chrono::Utc::now();
    let mut after_elements: Vec<InlineElement> = Vec::new(); // elements for the new block
    let mut char_cursor: usize = 0;
    let mut split_found = false;

    for elem in &elements {
        let elem_char_len = match &elem.content {
            InlineContent::Text(s) => s.chars().count(),
            InlineContent::Image { .. } => 1,
            InlineContent::Empty => 0,
        };

        if !split_found {
            if char_cursor + elem_char_len <= split_pos {
                // Entire element is before split — keep unchanged
                char_cursor += elem_char_len;
                continue;
            }
            // This element contains the split point
            split_found = true;
            let local_split = split_pos - char_cursor;

            match &elem.content {
                InlineContent::Text(s) => {
                    let chars: Vec<char> = s.chars().collect();
                    let before_text: String = chars[..local_split].iter().collect();
                    let after_text: String = chars[local_split..].iter().collect();

                    // Truncate this element to before_text
                    let mut updated = elem.clone();
                    updated.content = InlineContent::Text(before_text);
                    updated.updated_at = now;
                    uow.update_inline_element(&updated)?;

                    // Save the after portion as a new element (with same formatting)
                    if !after_text.is_empty() {
                        let mut new_elem = elem.clone();
                        new_elem.id = 0;
                        new_elem.content = InlineContent::Text(after_text);
                        new_elem.created_at = now;
                        new_elem.updated_at = now;
                        after_elements.push(new_elem);
                    }
                }
                InlineContent::Image { .. } => {
                    if local_split == 0 {
                        // Image goes to the new block
                        let mut new_elem = elem.clone();
                        new_elem.id = 0;
                        new_elem.created_at = now;
                        new_elem.updated_at = now;
                        after_elements.push(new_elem);
                        // Clear this element in current block
                        let mut cleared = elem.clone();
                        cleared.content = InlineContent::Empty;
                        cleared.updated_at = now;
                        uow.update_inline_element(&cleared)?;
                    }
                    // else image stays in current block (split is after image)
                }
                InlineContent::Empty => {}
            }
            char_cursor += elem_char_len;
        } else {
            // Element is entirely after the split — move to new block
            let mut new_elem = elem.clone();
            new_elem.id = 0;
            new_elem.created_at = now;
            new_elem.updated_at = now;
            after_elements.push(new_elem);

            // Clear in current block
            let mut cleared = elem.clone();
            cleared.content = InlineContent::Text(String::new());
            cleared.updated_at = now;
            uow.update_inline_element(&cleared)?;

            char_cursor += elem_char_len;
        }
    }

    // If no elements were split (split at very end), create an empty element for new block
    if after_elements.is_empty() {
        after_elements.push(InlineElement {
            id: 0,
            created_at: now,
            updated_at: now,
            content: InlineContent::Text(text_after.clone()),
            ..Default::default()
        });
    }

    // Update the current block cached fields
    let mut updated_current = current_block.clone();
    updated_current.plain_text = text_before.clone();
    updated_current.text_length = text_before.chars().count() as i64;
    updated_current.updated_at = now;
    uow.update_block(&updated_current)?;

    // Create a new block with text_after
    let new_block_position = current_block.document_position + updated_current.text_length + 1;
    let new_block = Block {
        id: 0,
        created_at: now,
        updated_at: now,
        elements: vec![],
        list: current_block.list,
        text_length: text_after.chars().count() as i64,
        document_position: new_block_position,
        plain_text: text_after,
        fmt_alignment: current_block.fmt_alignment.clone(),
        fmt_top_margin: current_block.fmt_top_margin,
        fmt_bottom_margin: current_block.fmt_bottom_margin,
        fmt_left_margin: current_block.fmt_left_margin,
        fmt_right_margin: current_block.fmt_right_margin,
        fmt_heading_level: current_block.fmt_heading_level,
        fmt_indent: current_block.fmt_indent,
        fmt_text_indent: current_block.fmt_text_indent,
        fmt_marker: current_block.fmt_marker.clone(),
        fmt_tab_positions: current_block.fmt_tab_positions.clone(),
        fmt_line_height: current_block.fmt_line_height,
        fmt_non_breakable_lines: current_block.fmt_non_breakable_lines,
        fmt_direction: current_block.fmt_direction.clone(),
        fmt_background_color: current_block.fmt_background_color.clone(),
        fmt_is_code_block: current_block.fmt_is_code_block,
        fmt_code_language: current_block.fmt_code_language.clone(),
    };

    // Find the frame that owns the current block (may be a nested sub-frame)
    fn find_owner_frame(
        uow: &dyn InsertBlockUnitOfWorkTrait,
        fid: &EntityId,
        target_block_id: EntityId,
    ) -> Result<Option<EntityId>> {
        let f = uow
            .get_frame(fid)?
            .ok_or_else(|| anyhow!("Frame not found"))?;
        for &entry in &f.child_order {
            if entry > 0 && entry as EntityId == target_block_id {
                return Ok(Some(*fid));
            } else if entry < 0 {
                let sub = (-entry) as EntityId;
                if let Some(owner) = find_owner_frame(uow, &sub, target_block_id)? {
                    return Ok(Some(owner));
                }
            }
        }
        let block_ids = uow.get_frame_relationship(fid, &FrameRelationshipField::Blocks)?;
        if block_ids.contains(&target_block_id) {
            return Ok(Some(*fid));
        }
        Ok(None)
    }
    let owner_frame_id = find_owner_frame(&**uow, &frame_id, current_block.id)?
        .unwrap_or(frame_id);

    let created_block = uow.create_block(&new_block, owner_frame_id, -1)?;

    // Create inline elements for the new block, preserving formatting
    for after_elem in &after_elements {
        uow.create_inline_element(after_elem, created_block.id, -1)?;
    }

    // Update the owner frame's child_order to include the new block after the split point
    let frame = uow
        .get_frame(&owner_frame_id)?
        .ok_or_else(|| anyhow!("Owner frame not found"))?;
    let mut updated_frame = frame.clone();
    // Find current_block's position in child_order and insert after it
    let co_idx = updated_frame
        .child_order
        .iter()
        .position(|&e| e > 0 && e as EntityId == current_block.id)
        .map(|i| i + 1)
        .unwrap_or(updated_frame.child_order.len());
    updated_frame
        .child_order
        .insert(co_idx, created_block.id as i64);
    updated_frame.updated_at = now;
    updated_frame.blocks =
        uow.get_frame_relationship(&owner_frame_id, &FrameRelationshipField::Blocks)?;
    uow.update_frame(&updated_frame)?;

    // Update subsequent blocks' document_position (those after the new block)
    // The new block's text_length is the length of text_after
    // Blocks after the original block_idx need their positions recalculated
    let mut blocks_to_update: Vec<Block> = Vec::new();
    for b in &blocks[(block_idx + 1)..] {
        let mut ub = b.clone();
        // Splitting a block introduces one additional block separator, so all
        // subsequent blocks shift forward by 1.
        ub.document_position += 1;
        ub.updated_at = now;
        blocks_to_update.push(ub);
    }
    if !blocks_to_update.is_empty() {
        uow.update_block_multi(&blocks_to_update)?;
    }

    // Update Document.block_count
    let mut updated_doc = document.clone();
    updated_doc.block_count += 1;
    updated_doc.updated_at = now;
    uow.update_document(&updated_doc)?;

    Ok((
        InsertBlockResultDto {
            new_position: new_block_position,
            new_block_id: created_block.id as i64,
        },
        snapshot,
    ))
}

impl InsertBlockUseCase {
    pub fn new(uow_factory: Box<dyn InsertBlockUnitOfWorkFactoryTrait>) -> Self {
        InsertBlockUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &InsertBlockDto) -> Result<InsertBlockResultDto> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let (result, snapshot) = execute_insert_block(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());

        uow.commit()?;
        Ok(result)
    }
}

impl UndoRedoCommand for InsertBlockUseCase {
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
        let (_, snapshot) = execute_insert_block(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
