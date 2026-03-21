use crate::InsertTextDto;
use crate::InsertTextResultDto;
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

pub trait InsertTextUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn InsertTextUnitOfWorkTrait>;
}

#[macros::uow_action(entity = "Root", action = "Get")]
#[macros::uow_action(entity = "Root", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Get")]
#[macros::uow_action(entity = "Document", action = "Update")]
#[macros::uow_action(entity = "Document", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Snapshot")]
#[macros::uow_action(entity = "Document", action = "Restore")]
#[macros::uow_action(entity = "Frame", action = "Get")]
#[macros::uow_action(entity = "Frame", action = "GetRelationship")]
#[macros::uow_action(entity = "Block", action = "Get")]
#[macros::uow_action(entity = "Block", action = "GetMulti")]
#[macros::uow_action(entity = "Block", action = "Update")]
#[macros::uow_action(entity = "Block", action = "UpdateMulti")]
#[macros::uow_action(entity = "Block", action = "GetRelationship")]
#[macros::uow_action(entity = "InlineElement", action = "Get")]
#[macros::uow_action(entity = "InlineElement", action = "GetMulti")]
#[macros::uow_action(entity = "InlineElement", action = "Update")]
pub trait InsertTextUnitOfWorkTrait: CommandUnitOfWork {}

pub struct InsertTextUseCase {
    uow_factory: Box<dyn InsertTextUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<InsertTextDto>,
}

/// Find the block containing the given document position from a list of blocks.
/// Returns (block, index_in_list, offset_within_block).
fn find_block_at_position(blocks: &[Block], position: i64) -> Result<(Block, usize, i64)> {
    for (i, block) in blocks.iter().enumerate() {
        let block_start = block.document_position;
        let block_end = block_start + block.text_length;
        // The position is within this block (inclusive of block_end for appending at end)
        if position >= block_start && position <= block_end {
            let offset = position - block_start;
            return Ok((block.clone(), i, offset));
        }
    }
    // If position is beyond all blocks, use the last block
    if let Some(block) = blocks.last() {
        let offset = block.text_length;
        return Ok((block.clone(), blocks.len() - 1, offset));
    }
    Err(anyhow!("No blocks found in document"))
}

/// Find the inline element at a given offset within a block, and compute
/// the offset within that element.
/// Returns (element, index_in_list, offset_within_element).
fn find_element_at_offset(
    elements: &[InlineElement],
    offset: i64,
) -> Result<(InlineElement, usize, i64)> {
    let mut running = 0i64;
    for (i, elem) in elements.iter().enumerate() {
        let elem_len = match &elem.content {
            InlineContent::Text(s) => s.chars().count() as i64,
            InlineContent::Image { .. } => 1,
            InlineContent::Empty => 0,
        };
        if offset <= running + elem_len {
            return Ok((elem.clone(), i, offset - running));
        }
        running += elem_len;
    }
    // Fall back to last element at its end
    if let Some(elem) = elements.last() {
        let elem_len = match &elem.content {
            InlineContent::Text(s) => s.chars().count() as i64,
            InlineContent::Image { .. } => 1,
            InlineContent::Empty => 0,
        };
        return Ok((elem.clone(), elements.len() - 1, elem_len));
    }
    Err(anyhow!("No inline elements found in block"))
}

/// Delete a character range within a single block's inline elements.
/// Updates the element content and returns the number of characters removed.
fn delete_range_in_block(
    uow: &mut Box<dyn InsertTextUnitOfWorkTrait>,
    block: &Block,
    start_offset: i64,
    end_offset: i64,
) -> Result<i64> {
    let element_ids = uow.get_block_relationship(&block.id, &BlockRelationshipField::Elements)?;
    let elements_opt = uow.get_inline_element_multi(&element_ids)?;
    let elements: Vec<InlineElement> = elements_opt.into_iter().filter_map(|e| e).collect();

    let mut running: i64 = 0;
    for elem in &elements {
        let elem_len = match &elem.content {
            InlineContent::Text(s) => s.chars().count() as i64,
            InlineContent::Image { .. } => 1,
            InlineContent::Empty => 0,
        };
        let elem_start = running;
        let elem_end = running + elem_len;

        let overlap_start = std::cmp::max(start_offset, elem_start);
        let overlap_end = std::cmp::min(end_offset, elem_end);

        if overlap_start < overlap_end {
            let local_start = (overlap_start - elem_start) as usize;
            let local_end = (overlap_end - elem_start) as usize;

            if let InlineContent::Text(s) = &elem.content {
                let chars: Vec<char> = s.chars().collect();
                let new_text: String = chars[..local_start]
                    .iter()
                    .chain(chars[local_end..].iter())
                    .collect();
                let mut updated = elem.clone();
                updated.content = InlineContent::Text(new_text);
                updated.updated_at = chrono::Utc::now();
                uow.update_inline_element(&updated)?;
            }
        }
        running += elem_len;
    }

    let chars_removed = end_offset - start_offset;

    // Update block cached fields
    let plain_chars: Vec<char> = block.plain_text.chars().collect();
    let so = start_offset as usize;
    let eo = (end_offset as usize).min(plain_chars.len());
    let new_plain: String = plain_chars[..so]
        .iter()
        .chain(plain_chars[eo..].iter())
        .collect();
    let mut updated_block = block.clone();
    updated_block.plain_text = new_plain;
    updated_block.text_length -= chars_removed;
    updated_block.updated_at = chrono::Utc::now();
    uow.update_block(&updated_block)?;

    Ok(chars_removed)
}

fn execute_insert(
    uow: &mut Box<dyn InsertTextUnitOfWorkTrait>,
    dto: &InsertTextDto,
) -> Result<(InsertTextResultDto, EntityTreeSnapshot)> {
    // Get Root -> Document
    let root = uow
        .get_root(&ROOT_ENTITY_ID)?
        .ok_or_else(|| anyhow!("Root entity not found"))?;
    let doc_ids = uow.get_root_relationship(&root.id, &RootRelationshipField::Document)?;
    let doc_id = *doc_ids
        .first()
        .ok_or_else(|| anyhow!("Root has no document"))?;

    let mut document = uow
        .get_document(&doc_id)?
        .ok_or_else(|| anyhow!("Document not found"))?;

    // Snapshot for undo before mutation
    let snapshot = uow.snapshot_document(&[doc_id])?;

    // Get frames
    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
    let frame_id = *frame_ids
        .first()
        .ok_or_else(|| anyhow!("Document has no frames"))?;

    // Get block IDs from frame
    let block_ids = uow.get_frame_relationship(&frame_id, &FrameRelationshipField::Blocks)?;

    // Get all blocks
    let blocks_opt = uow.get_block_multi(&block_ids)?;
    let mut blocks: Vec<Block> = blocks_opt.into_iter().filter_map(|b| b).collect();
    blocks.sort_by_key(|b| b.document_position);

    // Handle selection deletion (position != anchor) — same-block only
    let position;
    if dto.position != dto.anchor {
        let sel_start = std::cmp::min(dto.position, dto.anchor);
        let sel_end = std::cmp::max(dto.position, dto.anchor);
        position = sel_start;

        let (sel_block, sel_block_idx, sel_start_offset) =
            find_block_at_position(&blocks, sel_start)?;
        let (_, sel_end_block_idx, sel_end_offset) = find_block_at_position(&blocks, sel_end)?;

        if sel_block_idx != sel_end_block_idx {
            return Err(anyhow!(
                "Cross-block selection replacement is not supported by insert_text. \
                 Use delete_text first, then insert_text."
            ));
        }

        let chars_removed =
            delete_range_in_block(uow, &sel_block, sel_start_offset, sel_end_offset)?;

        // Update document character_count
        document.character_count -= chars_removed;
        document.updated_at = chrono::Utc::now();
        uow.update_document(&document)?;

        // Shift subsequent blocks
        let shift = chars_removed;
        let mut to_update = Vec::new();
        for b in &blocks[(sel_block_idx + 1)..] {
            let mut ub = b.clone();
            ub.document_position -= shift;
            ub.updated_at = chrono::Utc::now();
            to_update.push(ub);
        }
        if !to_update.is_empty() {
            uow.update_block_multi(&to_update)?;
        }

        // Re-read blocks after deletion
        let blocks_opt = uow.get_block_multi(&block_ids)?;
        blocks = blocks_opt.into_iter().filter_map(|b| b).collect();
        blocks.sort_by_key(|b| b.document_position);
        document = uow
            .get_document(&doc_id)?
            .ok_or_else(|| anyhow!("Document not found"))?;
    } else {
        position = dto.position;
    }

    // Find block at position
    let (block, block_idx, offset) = find_block_at_position(&blocks, position)?;

    // Get elements for this block
    let element_ids = uow.get_block_relationship(&block.id, &BlockRelationshipField::Elements)?;
    let elements_opt = uow.get_inline_element_multi(&element_ids)?;
    let elements: Vec<InlineElement> = elements_opt.into_iter().filter_map(|e| e).collect();

    if elements.is_empty() {
        return Err(anyhow!("Block has no inline elements"));
    }

    // Find element at offset
    let (element, _elem_idx, elem_offset) = find_element_at_offset(&elements, offset)?;

    // Insert text into the element
    let mut updated_element = element.clone();
    match &updated_element.content {
        InlineContent::Text(s) => {
            let mut new_text = s.clone();
            let byte_offset = if elem_offset as usize > new_text.len() {
                new_text.len()
            } else {
                // Find char boundary
                let mut idx = 0;
                let mut char_count = 0i64;
                for (ci, ch) in new_text.char_indices() {
                    if char_count == elem_offset {
                        idx = ci;
                        break;
                    }
                    char_count += 1;
                    idx = ci + ch.len_utf8();
                }
                if char_count < elem_offset {
                    idx = new_text.len();
                }
                idx
            };
            new_text.insert_str(byte_offset, &dto.text);
            updated_element.content = InlineContent::Text(new_text);
        }
        InlineContent::Empty => {
            updated_element.content = InlineContent::Text(dto.text.clone());
        }
        InlineContent::Image { .. } => {
            // Cannot insert text into an image element
            return Err(anyhow!("Cannot insert text into an image element"));
        }
    }
    updated_element.updated_at = chrono::Utc::now();
    uow.update_inline_element(&updated_element)?;

    // Update block cached fields
    let text_len = dto.text.chars().count() as i64;
    let mut updated_block = block.clone();
    updated_block.text_length += text_len;
    // Rebuild plain_text: insert at the offset
    let mut plain = updated_block.plain_text.clone();
    let byte_pos = {
        let mut idx = 0;
        let mut char_count = 0i64;
        for (ci, ch) in plain.char_indices() {
            if char_count == offset {
                idx = ci;
                break;
            }
            char_count += 1;
            idx = ci + ch.len_utf8();
        }
        if char_count < offset {
            idx = plain.len();
        }
        idx
    };
    plain.insert_str(byte_pos, &dto.text);
    updated_block.plain_text = plain;
    updated_block.updated_at = chrono::Utc::now();
    uow.update_block(&updated_block)?;

    // Update subsequent blocks' document_position
    let mut blocks_to_update: Vec<Block> = Vec::new();
    for b in &blocks[(block_idx + 1)..] {
        let mut ub = b.clone();
        ub.document_position += text_len;
        ub.updated_at = chrono::Utc::now();
        blocks_to_update.push(ub);
    }
    if !blocks_to_update.is_empty() {
        uow.update_block_multi(&blocks_to_update)?;
    }

    // Update Document.character_count
    let mut updated_doc = document.clone();
    updated_doc.character_count += text_len;
    updated_doc.updated_at = chrono::Utc::now();
    uow.update_document(&updated_doc)?;

    Ok((
        InsertTextResultDto {
            new_position: position + text_len,
            blocks_affected: 1,
        },
        snapshot,
    ))
}

impl InsertTextUseCase {
    pub fn new(uow_factory: Box<dyn InsertTextUnitOfWorkFactoryTrait>) -> Self {
        InsertTextUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &InsertTextDto) -> Result<InsertTextResultDto> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let (result, snapshot) = execute_insert(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());

        uow.commit()?;
        Ok(result)
    }
}

impl UndoRedoCommand for InsertTextUseCase {
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
        let (_, snapshot) = execute_insert(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
