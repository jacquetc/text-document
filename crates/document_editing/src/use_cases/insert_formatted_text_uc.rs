use super::editing_helpers::{collect_block_ids_recursive, find_element_at_offset};
use crate::InsertFormattedTextDto;
use crate::InsertFormattedTextResultDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::direct_access::block::block_repository::BlockRelationshipField;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::entities::{Block, Document, Frame, InlineContent, InlineElement, Root};
use common::types::{EntityId, ROOT_ENTITY_ID};
use common::undo_redo::UndoRedoCommand;
use std::any::Any;

pub trait InsertFormattedTextUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn InsertFormattedTextUnitOfWorkTrait>;
}

#[macros::uow_action(entity = "Root", action = "Get")]
#[macros::uow_action(entity = "Root", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Get")]
#[macros::uow_action(entity = "Document", action = "Update")]
#[macros::uow_action(entity = "Document", action = "GetRelationship")]
#[macros::uow_action(entity = "Frame", action = "Get")]
#[macros::uow_action(entity = "Frame", action = "GetRelationship")]
#[macros::uow_action(entity = "Block", action = "Get")]
#[macros::uow_action(entity = "Block", action = "GetMulti")]
#[macros::uow_action(entity = "Block", action = "Update")]
#[macros::uow_action(entity = "Block", action = "UpdateMulti")]
#[macros::uow_action(entity = "Block", action = "GetRelationship")]
#[macros::uow_action(entity = "Block", action = "SetRelationship")]
#[macros::uow_action(entity = "InlineElement", action = "Get")]
#[macros::uow_action(entity = "InlineElement", action = "GetMulti")]
#[macros::uow_action(entity = "InlineElement", action = "Update")]
#[macros::uow_action(entity = "InlineElement", action = "Create")]
#[macros::uow_action(entity = "InlineElement", action = "Remove")]
pub trait InsertFormattedTextUnitOfWorkTrait: CommandUnitOfWork {}

/// Lightweight undo data — stores only the few entities that actually changed.
/// No snapshot serialization at all.
struct UndoData {
    /// The original inline element before it was split.
    original_element: InlineElement,
    /// The original list of element IDs on the block (to restore the relationship).
    original_element_ids: Vec<EntityId>,
    /// IDs of elements created by the insertion (to delete on undo).
    created_element_ids: Vec<EntityId>,
    /// The original block state (plain_text, text_length, etc.).
    original_block: Block,
    /// Document ID and original character_count.
    doc_id: EntityId,
    original_character_count: i64,
    /// Block ID.
    block_id: EntityId,
}

pub struct InsertFormattedTextUseCase {
    uow_factory: Box<dyn InsertFormattedTextUnitOfWorkFactoryTrait>,
    undo_data: Option<UndoData>,
    last_dto: Option<InsertFormattedTextDto>,
}

fn execute_insert_formatted_text(
    uow: &mut Box<dyn InsertFormattedTextUnitOfWorkTrait>,
    dto: &InsertFormattedTextDto,
) -> Result<(InsertFormattedTextResultDto, UndoData)> {
    let position = std::cmp::min(dto.position, dto.anchor);

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

    // Get all block IDs in document order, traversing into nested frames
    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
    let frame_id = *frame_ids
        .first()
        .ok_or_else(|| anyhow!("Document has no frames"))?;

    let ordered_block_ids = collect_block_ids_recursive(
        &|id| uow.get_frame(id),
        &|id, field| uow.get_frame_relationship(id, field),
        &frame_id,
    )?;

    if ordered_block_ids.is_empty() {
        return Err(anyhow!("No blocks in document"));
    }

    // Find block at position by computing positions on the fly
    let (block, _block_idx, block_pos) =
        find_block_at_position_sequential(&**uow, &ordered_block_ids, position)?;
    let offset = position - block_pos;

    // Save original block for undo (cheap clone, no DB serialization)
    let original_block = block.clone();

    // Get elements for this block
    let element_ids = uow.get_block_relationship(&block.id, &BlockRelationshipField::Elements)?;
    let original_element_ids = element_ids.clone();
    let elements_opt = uow.get_inline_element_multi(&element_ids)?;
    let elements: Vec<InlineElement> = elements_opt.into_iter().flatten().collect();

    if elements.is_empty() {
        return Err(anyhow!("Block has no inline elements"));
    }

    // Find element at offset
    let (element, elem_idx, elem_offset) = find_element_at_offset(&elements, offset)?;

    // Save original element for undo
    let original_element = element.clone();

    let now = chrono::Utc::now();
    let text_len = dto.text.chars().count() as i64;
    let mut created_element_ids: Vec<EntityId> = Vec::new();

    // Split the current element at the insertion point and insert a new formatted element
    match &element.content {
        InlineContent::Text(s) => {
            let chars: Vec<char> = s.chars().collect();
            let before_text: String = chars[..elem_offset as usize].iter().collect();
            let after_text: String = chars[elem_offset as usize..].iter().collect();

            // Truncate the current element to before_text
            let mut updated = element.clone();
            updated.content = InlineContent::Text(before_text);
            updated.updated_at = now;
            uow.update_inline_element(&updated)?;

            // Create the new formatted element
            let new_elem = InlineElement {
                id: 0,
                created_at: now,
                updated_at: now,
                content: InlineContent::Text(dto.text.clone()),
                fmt_font_family: Some(dto.font_family.clone()),
                fmt_font_point_size: Some(dto.font_point_size),
                fmt_font_bold: Some(dto.font_bold),
                fmt_font_italic: Some(dto.font_italic),
                fmt_font_underline: Some(dto.font_underline),
                fmt_font_strikeout: Some(dto.font_strikeout),
                ..Default::default()
            };
            let insert_index = (elem_idx + 1) as i32;
            let created = uow.create_inline_element(&new_elem, block.id, insert_index)?;
            created_element_ids.push(created.id);

            // Create the after element if non-empty
            if !after_text.is_empty() {
                let after_elem = InlineElement {
                    id: 0,
                    created_at: now,
                    updated_at: now,
                    content: InlineContent::Text(after_text),
                    fmt_font_family: element.fmt_font_family.clone(),
                    fmt_font_point_size: element.fmt_font_point_size,
                    fmt_font_bold: element.fmt_font_bold,
                    fmt_font_italic: element.fmt_font_italic,
                    fmt_font_underline: element.fmt_font_underline,
                    fmt_font_overline: element.fmt_font_overline,
                    fmt_font_strikeout: element.fmt_font_strikeout,
                    fmt_font_weight: element.fmt_font_weight,
                    fmt_letter_spacing: element.fmt_letter_spacing,
                    fmt_word_spacing: element.fmt_word_spacing,
                    fmt_anchor_href: element.fmt_anchor_href.clone(),
                    fmt_anchor_names: element.fmt_anchor_names.clone(),
                    fmt_is_anchor: element.fmt_is_anchor,
                    fmt_tooltip: element.fmt_tooltip.clone(),
                    fmt_underline_style: element.fmt_underline_style.clone(),
                    fmt_vertical_alignment: element.fmt_vertical_alignment.clone(),
                };
                let created_after =
                    uow.create_inline_element(&after_elem, block.id, insert_index + 1)?;
                created_element_ids.push(created_after.id);
            }
        }
        InlineContent::Empty => {
            let mut updated = element.clone();
            updated.content = InlineContent::Text(dto.text.clone());
            updated.fmt_font_family = Some(dto.font_family.clone());
            updated.fmt_font_point_size = Some(dto.font_point_size);
            updated.fmt_font_bold = Some(dto.font_bold);
            updated.fmt_font_italic = Some(dto.font_italic);
            updated.fmt_font_underline = Some(dto.font_underline);
            updated.fmt_font_strikeout = Some(dto.font_strikeout);
            updated.updated_at = now;
            uow.update_inline_element(&updated)?;
        }
        InlineContent::Image { .. } => {
            return Err(anyhow!("Cannot insert text into an image element"));
        }
    }

    // Update block cached fields
    let mut updated_block = block.clone();
    updated_block.text_length += text_len;
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
    updated_block.updated_at = now;
    uow.update_block(&updated_block)?;

    // Note: we intentionally do NOT update subsequent blocks' document_position here.
    // Positions are computed on the fly from child_order + text_length.

    // Update Document.character_count
    let mut updated_doc = document.clone();
    updated_doc.character_count += text_len;
    updated_doc.updated_at = now;
    uow.update_document(&updated_doc)?;

    let undo_data = UndoData {
        original_element,
        original_element_ids,
        created_element_ids,
        original_block,
        doc_id,
        original_character_count: document.character_count,
        block_id: block.id,
    };

    Ok((
        InsertFormattedTextResultDto {
            new_position: position + text_len,
        },
        undo_data,
    ))
}

/// Binary search through ordered block IDs to find the block containing `position`.
/// Find the block containing `position` by computing positions on the fly
/// from child_order + text_length. No dependency on stored document_position.
fn find_block_at_position_sequential(
    uow: &dyn InsertFormattedTextUnitOfWorkTrait,
    ordered_block_ids: &[EntityId],
    position: i64,
) -> Result<(Block, usize, i64)> {
    if ordered_block_ids.is_empty() {
        return Err(anyhow!("No blocks in document"));
    }

    let mut running_pos: i64 = 0;
    for (idx, &block_id) in ordered_block_ids.iter().enumerate() {
        let block = uow
            .get_block(&block_id)?
            .ok_or_else(|| anyhow!("Block not found"))?;
        let block_end = running_pos + block.text_length;

        if position >= running_pos && position <= block_end {
            return Ok((block, idx, running_pos));
        }
        running_pos = block_end + 1;
    }

    // Fallback to last block
    let last_idx = ordered_block_ids.len() - 1;
    let block = uow
        .get_block(&ordered_block_ids[last_idx])?
        .ok_or_else(|| anyhow!("Block not found"))?;
    let mut pos: i64 = 0;
    for &id in &ordered_block_ids[..last_idx] {
        if let Some(b) = uow.get_block(&id)? {
            pos += b.text_length + 1;
        }
    }
    Ok((block, last_idx, pos))
}

impl InsertFormattedTextUseCase {
    pub fn new(uow_factory: Box<dyn InsertFormattedTextUnitOfWorkFactoryTrait>) -> Self {
        InsertFormattedTextUseCase {
            uow_factory,
            undo_data: None,
            last_dto: None,
        }
    }

    pub fn execute(
        &mut self,
        dto: &InsertFormattedTextDto,
    ) -> Result<InsertFormattedTextResultDto> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let (result, undo_data) = execute_insert_formatted_text(&mut uow, dto)?;
        self.undo_data = Some(undo_data);
        self.last_dto = Some(dto.clone());

        uow.commit()?;
        Ok(result)
    }
}

impl UndoRedoCommand for InsertFormattedTextUseCase {
    fn undo(&mut self) -> Result<()> {
        let undo_data = self
            .undo_data
            .as_ref()
            .ok_or_else(|| anyhow!("No undo data available"))?;

        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        // 1. Delete created elements
        for &elem_id in &undo_data.created_element_ids {
            uow.remove_inline_element(&elem_id)?;
        }

        // 2. Restore the original element
        uow.update_inline_element(&undo_data.original_element)?;

        // 3. Restore the block's element relationship to original
        uow.set_block_relationship(
            &undo_data.block_id,
            &BlockRelationshipField::Elements,
            &undo_data.original_element_ids,
        )?;

        // 4. Restore the original block (plain_text, text_length, etc.)
        uow.update_block(&undo_data.original_block)?;

        // No position shifts to reverse — insert doesn't update them.
        // Positions are computed on the fly from child_order + text_length.

        // 5. Restore document character_count
        let mut doc = uow
            .get_document(&undo_data.doc_id)?
            .ok_or_else(|| anyhow!("Document not found"))?;
        doc.character_count = undo_data.original_character_count;
        doc.updated_at = chrono::Utc::now();
        uow.update_document(&doc)?;

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
        let (_, undo_data) = execute_insert_formatted_text(&mut uow, &dto)?;
        self.undo_data = Some(undo_data);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
