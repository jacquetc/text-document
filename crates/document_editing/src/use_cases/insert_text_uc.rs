use super::editing_helpers::{
    find_block_at_position, find_element_at_offset, is_word_boundary_punct,
};
use crate::InsertTextDto;
use crate::InsertTextResultDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::direct_access::block::block_repository::BlockRelationshipField;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::entities::{Block, Document, Frame, InlineContent, InlineElement, Root};
use common::types::{EntityId, ROOT_ENTITY_ID};
use common::undo_redo::UndoRedoCommand;
use std::any::Any;
use std::time::Instant;

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

/// Lightweight undo data for the no-selection insert path.
struct UndoData {
    original_element: InlineElement,
    original_block: Block,
    doc_id: EntityId,
    original_character_count: i64,
    text_len: i64,
    frame_id: EntityId,
    block_id: EntityId,
}

/// Undo data for the selection-replacement path (needs full snapshot).
enum InsertTextUndo {
    /// Fast path: simple insert, no selection. Clone-based undo.
    Simple(UndoData),
    /// Slow path: selection replacement. Uses full document snapshot.
    SelectionReplacement(common::snapshot::EntityTreeSnapshot),
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
    let elements: Vec<InlineElement> = elements_opt.into_iter().flatten().collect();

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

/// Execute insert with selection replacement — uses full document snapshot for undo.
fn execute_insert_with_selection(
    uow: &mut Box<dyn InsertTextUnitOfWorkTrait>,
    dto: &InsertTextDto,
) -> Result<(InsertTextResultDto, InsertTextUndo)> {
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

    // Full snapshot needed for selection replacement undo
    let snapshot = uow.snapshot_document(&[doc_id])?;

    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
    let frame_id = *frame_ids
        .first()
        .ok_or_else(|| anyhow!("Document has no frames"))?;

    let block_ids = uow.get_frame_relationship(&frame_id, &FrameRelationshipField::Blocks)?;
    let blocks_opt = uow.get_block_multi(&block_ids)?;
    let mut blocks: Vec<Block> = blocks_opt.into_iter().flatten().collect();
    blocks.sort_by_key(|b| b.document_position);

    let sel_start = std::cmp::min(dto.position, dto.anchor);
    let sel_end = std::cmp::max(dto.position, dto.anchor);
    let position = sel_start;

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

    document.character_count -= chars_removed;
    document.updated_at = chrono::Utc::now();
    uow.update_document(&document)?;

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
    blocks = blocks_opt.into_iter().flatten().collect();
    blocks.sort_by_key(|b| b.document_position);
    document = uow
        .get_document(&doc_id)?
        .ok_or_else(|| anyhow!("Document not found"))?;

    // Now insert text (same as no-selection path)
    let (block, block_idx, offset) = find_block_at_position(&blocks, position)?;
    let element_ids = uow.get_block_relationship(&block.id, &BlockRelationshipField::Elements)?;
    let elements_opt = uow.get_inline_element_multi(&element_ids)?;
    let elements: Vec<InlineElement> = elements_opt.into_iter().flatten().collect();

    if elements.is_empty() {
        return Err(anyhow!("Block has no inline elements"));
    }

    let (element, _elem_idx, elem_offset) = find_element_at_offset(&elements, offset)?;

    let mut updated_element = element.clone();
    match &updated_element.content {
        InlineContent::Text(s) => {
            let mut new_text = s.clone();
            let byte_offset = char_to_byte_offset(&new_text, elem_offset);
            new_text.insert_str(byte_offset, &dto.text);
            updated_element.content = InlineContent::Text(new_text);
        }
        InlineContent::Empty => {
            updated_element.content = InlineContent::Text(dto.text.clone());
        }
        InlineContent::Image { .. } => {
            return Err(anyhow!("Cannot insert text into an image element"));
        }
    }
    updated_element.updated_at = chrono::Utc::now();
    uow.update_inline_element(&updated_element)?;

    let text_len = dto.text.chars().count() as i64;
    let mut updated_block = block.clone();
    updated_block.text_length += text_len;
    let mut plain = updated_block.plain_text.clone();
    let byte_pos = char_to_byte_offset(&plain, offset);
    plain.insert_str(byte_pos, &dto.text);
    updated_block.plain_text = plain;
    updated_block.updated_at = chrono::Utc::now();
    uow.update_block(&updated_block)?;

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

    let mut updated_doc = document.clone();
    updated_doc.character_count += text_len;
    updated_doc.updated_at = chrono::Utc::now();
    uow.update_document(&updated_doc)?;

    Ok((
        InsertTextResultDto {
            new_position: position + text_len,
            blocks_affected: 1,
        },
        InsertTextUndo::SelectionReplacement(snapshot),
    ))
}

/// Execute insert without selection — optimized path with clone-based undo.
fn execute_insert_simple(
    uow: &mut Box<dyn InsertTextUnitOfWorkTrait>,
    dto: &InsertTextDto,
) -> Result<(InsertTextResultDto, InsertTextUndo)> {
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

    // Get frame and its child_order (block IDs in document order)
    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
    let frame_id = *frame_ids
        .first()
        .ok_or_else(|| anyhow!("Document has no frames"))?;

    let frame = uow
        .get_frame(&frame_id)?
        .ok_or_else(|| anyhow!("Frame not found"))?;

    // Get ordered block IDs — prefer child_order for binary search
    let ordered_block_ids: Vec<EntityId> = if !frame.child_order.is_empty() {
        frame
            .child_order
            .iter()
            .filter(|&&id| id > 0)
            .map(|&id| id as EntityId)
            .collect()
    } else {
        uow.get_frame_relationship(&frame_id, &FrameRelationshipField::Blocks)?
    };

    if ordered_block_ids.is_empty() {
        return Err(anyhow!("No blocks in document"));
    }

    // Binary search to find the block at position
    let (block, block_idx) =
        find_block_at_position_binary(uow, &ordered_block_ids, position)?;
    let offset = position - block.document_position;

    // Save originals for undo (cheap clones, no serialization)
    let original_block = block.clone();

    // Get elements for this block
    let element_ids = uow.get_block_relationship(&block.id, &BlockRelationshipField::Elements)?;
    let elements_opt = uow.get_inline_element_multi(&element_ids)?;
    let elements: Vec<InlineElement> = elements_opt.into_iter().flatten().collect();

    if elements.is_empty() {
        return Err(anyhow!("Block has no inline elements"));
    }

    let (element, _elem_idx, elem_offset) = find_element_at_offset(&elements, offset)?;
    let original_element = element.clone();

    // Insert text into the element
    let mut updated_element = element.clone();
    match &updated_element.content {
        InlineContent::Text(s) => {
            let mut new_text = s.clone();
            let byte_offset = char_to_byte_offset(&new_text, elem_offset);
            new_text.insert_str(byte_offset, &dto.text);
            updated_element.content = InlineContent::Text(new_text);
        }
        InlineContent::Empty => {
            updated_element.content = InlineContent::Text(dto.text.clone());
        }
        InlineContent::Image { .. } => {
            return Err(anyhow!("Cannot insert text into an image element"));
        }
    }
    updated_element.updated_at = chrono::Utc::now();
    uow.update_inline_element(&updated_element)?;

    // Update block cached fields
    let text_len = dto.text.chars().count() as i64;
    let mut updated_block = block.clone();
    updated_block.text_length += text_len;
    let mut plain = updated_block.plain_text.clone();
    let byte_pos = char_to_byte_offset(&plain, offset);
    plain.insert_str(byte_pos, &dto.text);
    updated_block.plain_text = plain;
    updated_block.updated_at = chrono::Utc::now();
    uow.update_block(&updated_block)?;

    // Update subsequent blocks' document_position — only read blocks AFTER the target
    let subsequent_ids: Vec<EntityId> = ordered_block_ids[(block_idx + 1)..].to_vec();
    if !subsequent_ids.is_empty() {
        let subsequent_opt = uow.get_block_multi(&subsequent_ids)?;
        let now = chrono::Utc::now();
        let mut blocks_to_update: Vec<Block> = Vec::new();
        for b in subsequent_opt.into_iter().flatten() {
            let mut ub = b;
            ub.document_position += text_len;
            ub.updated_at = now;
            blocks_to_update.push(ub);
        }
        if !blocks_to_update.is_empty() {
            uow.update_block_multi(&blocks_to_update)?;
        }
    }

    // Update Document.character_count
    let mut updated_doc = document.clone();
    updated_doc.character_count += text_len;
    updated_doc.updated_at = chrono::Utc::now();
    uow.update_document(&updated_doc)?;

    let undo_data = UndoData {
        original_element,
        original_block,
        doc_id,
        original_character_count: document.character_count,
        text_len,
        frame_id,
        block_id: block.id,
    };

    Ok((
        InsertTextResultDto {
            new_position: position + text_len,
            blocks_affected: 1,
        },
        InsertTextUndo::Simple(undo_data),
    ))
}

/// Binary search through ordered block IDs to find the block containing `position`.
fn find_block_at_position_binary(
    uow: &Box<dyn InsertTextUnitOfWorkTrait>,
    ordered_block_ids: &[EntityId],
    position: i64,
) -> Result<(Block, usize)> {
    if ordered_block_ids.is_empty() {
        return Err(anyhow!("No blocks in document"));
    }

    let mut left = 0usize;
    let mut right = ordered_block_ids.len() - 1;

    while left <= right {
        let mid = left + (right - left) / 2;
        let block = uow
            .get_block(&ordered_block_ids[mid])?
            .ok_or_else(|| anyhow!("Block not found"))?;
        let block_end = block.document_position + block.text_length;

        if position >= block.document_position && position <= block_end {
            return Ok((block, mid));
        } else if position < block.document_position {
            if mid == 0 {
                return Ok((block, mid));
            }
            right = mid - 1;
        } else {
            left = mid + 1;
        }
    }

    // Fallback to last block
    let last_idx = ordered_block_ids.len() - 1;
    let block = uow
        .get_block(&ordered_block_ids[last_idx])?
        .ok_or_else(|| anyhow!("Block not found"))?;
    Ok((block, last_idx))
}

/// Convert a char offset to a byte offset in a string.
fn char_to_byte_offset(s: &str, char_offset: i64) -> usize {
    let mut idx = 0;
    let mut count = 0i64;
    for (ci, ch) in s.char_indices() {
        if count == char_offset {
            return ci;
        }
        count += 1;
        idx = ci + ch.len_utf8();
    }
    if count < char_offset {
        s.len()
    } else {
        idx
    }
}

pub struct InsertTextUseCase {
    uow_factory: Box<dyn InsertTextUnitOfWorkFactoryTrait>,
    undo_data: Option<InsertTextUndo>,
    last_dto: Option<InsertTextDto>,
    last_result: Option<InsertTextResultDto>,
    last_merge_time: Option<Instant>,
    was_selection_replacement: bool,
}

impl InsertTextUseCase {
    pub fn new(uow_factory: Box<dyn InsertTextUnitOfWorkFactoryTrait>) -> Self {
        InsertTextUseCase {
            uow_factory,
            undo_data: None,
            last_dto: None,
            last_result: None,
            last_merge_time: None,
            was_selection_replacement: false,
        }
    }

    pub fn execute(&mut self, dto: &InsertTextDto) -> Result<InsertTextResultDto> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let has_selection = dto.position != dto.anchor;
        let (result, undo) = if has_selection {
            execute_insert_with_selection(&mut uow, dto)?
        } else {
            execute_insert_simple(&mut uow, dto)?
        };

        self.undo_data = Some(undo);
        self.last_dto = Some(dto.clone());
        self.last_result = Some(result.clone());
        self.last_merge_time = Some(Instant::now());
        self.was_selection_replacement = has_selection;

        uow.commit()?;
        Ok(result)
    }
}

impl UndoRedoCommand for InsertTextUseCase {
    fn undo(&mut self) -> Result<()> {
        let undo = self
            .undo_data
            .as_ref()
            .ok_or_else(|| anyhow!("No undo data available"))?;

        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        match undo {
            InsertTextUndo::SelectionReplacement(snapshot) => {
                uow.restore_document(&snapshot.clone())?;
            }
            InsertTextUndo::Simple(data) => {
                // Restore original element
                uow.update_inline_element(&data.original_element)?;

                // Restore original block
                uow.update_block(&data.original_block)?;

                // Reverse position shifts on subsequent blocks
                let frame = uow
                    .get_frame(&data.frame_id)?
                    .ok_or_else(|| anyhow!("Frame not found"))?;
                let ordered_block_ids: Vec<EntityId> = frame
                    .child_order
                    .iter()
                    .filter(|&&id| id > 0)
                    .map(|&id| id as EntityId)
                    .collect();

                if let Some(block_idx) = ordered_block_ids
                    .iter()
                    .position(|&id| id == data.block_id)
                {
                    let subsequent_ids: Vec<EntityId> =
                        ordered_block_ids[(block_idx + 1)..].to_vec();
                    if !subsequent_ids.is_empty() {
                        let subsequent_opt = uow.get_block_multi(&subsequent_ids)?;
                        let now = chrono::Utc::now();
                        let mut blocks_to_update: Vec<Block> = Vec::new();
                        for b in subsequent_opt.into_iter().flatten() {
                            let mut ub = b;
                            ub.document_position -= data.text_len;
                            ub.updated_at = now;
                            blocks_to_update.push(ub);
                        }
                        if !blocks_to_update.is_empty() {
                            uow.update_block_multi(&blocks_to_update)?;
                        }
                    }
                }

                // Restore document character_count
                let mut doc = uow
                    .get_document(&data.doc_id)?
                    .ok_or_else(|| anyhow!("Document not found"))?;
                doc.character_count = data.original_character_count;
                doc.updated_at = chrono::Utc::now();
                uow.update_document(&doc)?;
            }
        }

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
        let has_selection = dto.position != dto.anchor;
        let (_, undo) = if has_selection {
            execute_insert_with_selection(&mut uow, &dto)?
        } else {
            execute_insert_simple(&mut uow, &dto)?
        };
        self.undo_data = Some(undo);
        uow.commit()?;
        Ok(())
    }

    fn can_merge(&self, other: &dyn UndoRedoCommand) -> bool {
        let Some(other_cmd) = other.as_any().downcast_ref::<InsertTextUseCase>() else {
            return false;
        };

        let (Some(self_result), Some(self_time), Some(self_dto)) =
            (&self.last_result, &self.last_merge_time, &self.last_dto)
        else {
            return false;
        };
        let (Some(other_dto), Some(other_time)) = (&other_cmd.last_dto, &other_cmd.last_merge_time)
        else {
            return false;
        };

        // Rule 1: Time limit — 2 seconds between keystrokes
        if other_time.duration_since(*self_time) > std::time::Duration::from_secs(2) {
            return false;
        }

        // Rule 2: The new command must NOT be a selection replacement
        if other_cmd.was_selection_replacement {
            return false;
        }

        // Rule 3: Contiguous — other.position must equal self.new_position
        if other_dto.position != self_result.new_position {
            return false;
        }

        // Rule 4: Word boundary — break on space/punctuation after non-space
        let self_text = &self_dto.text;
        let other_text = &other_dto.text;
        if let (Some(last_self), Some(first_other)) =
            (self_text.chars().next_back(), other_text.chars().next())
        {
            if !last_self.is_whitespace()
                && (first_other.is_whitespace() || is_word_boundary_punct(first_other))
            {
                return false;
            }
        }

        true
    }

    fn merge(&mut self, other: &dyn UndoRedoCommand) -> bool {
        let Some(other_cmd) = other.as_any().downcast_ref::<InsertTextUseCase>() else {
            return false;
        };

        // Keep our undo data (original state) but update the DTO and result
        if let (Some(self_dto), Some(other_dto)) =
            (&mut self.last_dto, &other_cmd.last_dto)
        {
            self_dto.text.push_str(&other_dto.text);
            self_dto.anchor = self_dto.position; // merged command always has no selection
        }
        if let Some(other_result) = &other_cmd.last_result {
            self.last_result = Some(other_result.clone());
        }
        self.last_merge_time = other_cmd.last_merge_time;

        true
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
