use super::editing_helpers::{find_block_at_position, find_element_at_offset};
use crate::InsertFormattedTextDto;
use crate::InsertFormattedTextResultDto;
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

pub trait InsertFormattedTextUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn InsertFormattedTextUnitOfWorkTrait>;
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
#[macros::uow_action(entity = "InlineElement", action = "Create")]
pub trait InsertFormattedTextUnitOfWorkTrait: CommandUnitOfWork {}

pub struct InsertFormattedTextUseCase {
    uow_factory: Box<dyn InsertFormattedTextUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<InsertFormattedTextDto>,
}

fn execute_insert_formatted_text(
    uow: &mut Box<dyn InsertFormattedTextUnitOfWorkTrait>,
    dto: &InsertFormattedTextDto,
) -> Result<(InsertFormattedTextResultDto, EntityTreeSnapshot)> {
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
    let mut blocks: Vec<Block> = blocks_opt.into_iter().flatten().collect();
    blocks.sort_by_key(|b| b.document_position);

    // Find block at position
    let (block, block_idx, offset) = find_block_at_position(&blocks, position)?;

    // Get elements for this block
    let element_ids = uow.get_block_relationship(&block.id, &BlockRelationshipField::Elements)?;
    let elements_opt = uow.get_inline_element_multi(&element_ids)?;
    let elements: Vec<InlineElement> = elements_opt.into_iter().flatten().collect();

    if elements.is_empty() {
        return Err(anyhow!("Block has no inline elements"));
    }

    // Find element at offset
    let (element, elem_idx, elem_offset) = find_element_at_offset(&elements, offset)?;

    let now = chrono::Utc::now();
    let text_len = dto.text.chars().count() as i64;

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
            uow.create_inline_element(&new_elem, block.id, insert_index)?;

            // Create the after element if non-empty
            if !after_text.is_empty() {
                let after_elem = InlineElement {
                    id: 0,
                    created_at: now,
                    updated_at: now,
                    content: InlineContent::Text(after_text),
                    // Copy formatting from the original element
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
                uow.create_inline_element(&after_elem, block.id, insert_index + 1)?;
            }
        }
        InlineContent::Empty => {
            // Replace the empty element content with our formatted text
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
    updated_block.updated_at = now;
    uow.update_block(&updated_block)?;

    // Update subsequent blocks' document_position
    let mut blocks_to_update: Vec<Block> = Vec::new();
    for b in &blocks[(block_idx + 1)..] {
        let mut ub = b.clone();
        ub.document_position += text_len;
        ub.updated_at = now;
        blocks_to_update.push(ub);
    }
    if !blocks_to_update.is_empty() {
        uow.update_block_multi(&blocks_to_update)?;
    }

    // Update Document.character_count
    let mut updated_doc = document.clone();
    updated_doc.character_count += text_len;
    updated_doc.updated_at = now;
    uow.update_document(&updated_doc)?;

    Ok((
        InsertFormattedTextResultDto {
            new_position: position + text_len,
        },
        snapshot,
    ))
}

impl InsertFormattedTextUseCase {
    pub fn new(uow_factory: Box<dyn InsertFormattedTextUnitOfWorkFactoryTrait>) -> Self {
        InsertFormattedTextUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(
        &mut self,
        dto: &InsertFormattedTextDto,
    ) -> Result<InsertFormattedTextResultDto> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let (result, snapshot) = execute_insert_formatted_text(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());

        uow.commit()?;
        Ok(result)
    }
}

impl UndoRedoCommand for InsertFormattedTextUseCase {
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
        let (_, snapshot) = execute_insert_formatted_text(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
