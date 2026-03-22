use crate::SetTextFormatDto;
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

pub trait SetTextFormatUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn SetTextFormatUnitOfWorkTrait>;
}

#[macros::uow_action(entity = "Root", action = "Get")]
#[macros::uow_action(entity = "Root", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Get")]
#[macros::uow_action(entity = "Document", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Snapshot")]
#[macros::uow_action(entity = "Document", action = "Restore")]
#[macros::uow_action(entity = "Frame", action = "Get")]
#[macros::uow_action(entity = "Frame", action = "GetRelationship")]
#[macros::uow_action(entity = "Block", action = "Get")]
#[macros::uow_action(entity = "Block", action = "GetMulti")]
#[macros::uow_action(entity = "Block", action = "GetRelationship")]
#[macros::uow_action(entity = "InlineElement", action = "Get")]
#[macros::uow_action(entity = "InlineElement", action = "GetMulti")]
#[macros::uow_action(entity = "InlineElement", action = "Update")]
#[macros::uow_action(entity = "InlineElement", action = "Create")]
pub trait SetTextFormatUnitOfWorkTrait: CommandUnitOfWork {}

fn underline_style_to_entity(s: &crate::dtos::UnderlineStyle) -> common::entities::UnderlineStyle {
    match s {
        crate::dtos::UnderlineStyle::NoUnderline => common::entities::UnderlineStyle::NoUnderline,
        crate::dtos::UnderlineStyle::SingleUnderline => {
            common::entities::UnderlineStyle::SingleUnderline
        }
        crate::dtos::UnderlineStyle::DashUnderline => {
            common::entities::UnderlineStyle::DashUnderline
        }
        crate::dtos::UnderlineStyle::DotLine => common::entities::UnderlineStyle::DotLine,
        crate::dtos::UnderlineStyle::DashDotLine => common::entities::UnderlineStyle::DashDotLine,
        crate::dtos::UnderlineStyle::DashDotDotLine => {
            common::entities::UnderlineStyle::DashDotDotLine
        }
        crate::dtos::UnderlineStyle::WaveUnderline => {
            common::entities::UnderlineStyle::WaveUnderline
        }
        crate::dtos::UnderlineStyle::SpellCheckUnderline => {
            common::entities::UnderlineStyle::SpellCheckUnderline
        }
    }
}

fn vertical_alignment_to_entity(
    v: &crate::dtos::CharVerticalAlignment,
) -> common::entities::CharVerticalAlignment {
    match v {
        crate::dtos::CharVerticalAlignment::Normal => {
            common::entities::CharVerticalAlignment::Normal
        }
        crate::dtos::CharVerticalAlignment::SuperScript => {
            common::entities::CharVerticalAlignment::SuperScript
        }
        crate::dtos::CharVerticalAlignment::SubScript => {
            common::entities::CharVerticalAlignment::SubScript
        }
        crate::dtos::CharVerticalAlignment::Middle => {
            common::entities::CharVerticalAlignment::Middle
        }
        crate::dtos::CharVerticalAlignment::Bottom => {
            common::entities::CharVerticalAlignment::Bottom
        }
        crate::dtos::CharVerticalAlignment::Top => common::entities::CharVerticalAlignment::Top,
        crate::dtos::CharVerticalAlignment::Baseline => {
            common::entities::CharVerticalAlignment::Baseline
        }
    }
}

/// Get the character length of an inline element.
fn element_char_len(elem: &InlineElement) -> i64 {
    match &elem.content {
        InlineContent::Text(s) => s.chars().count() as i64,
        InlineContent::Image { .. } => 1,
        InlineContent::Empty => 0,
    }
}

/// Apply text format fields from the DTO to an inline element.
/// `None` fields are left unchanged (preserve existing formatting).
fn apply_text_format(elem: &mut InlineElement, dto: &SetTextFormatDto) {
    if let Some(ref v) = dto.font_family {
        elem.fmt_font_family = Some(v.clone());
    }
    if let Some(v) = dto.font_point_size {
        elem.fmt_font_point_size = Some(v);
    }
    if let Some(v) = dto.font_weight {
        elem.fmt_font_weight = Some(v);
    }
    if let Some(v) = dto.font_bold {
        elem.fmt_font_bold = Some(v);
    }
    if let Some(v) = dto.font_italic {
        elem.fmt_font_italic = Some(v);
    }
    if let Some(v) = dto.font_underline {
        elem.fmt_font_underline = Some(v);
    }
    if let Some(v) = dto.font_overline {
        elem.fmt_font_overline = Some(v);
    }
    if let Some(v) = dto.font_strikeout {
        elem.fmt_font_strikeout = Some(v);
    }
    if let Some(v) = dto.letter_spacing {
        elem.fmt_letter_spacing = Some(v);
    }
    if let Some(v) = dto.word_spacing {
        elem.fmt_word_spacing = Some(v);
    }
    if let Some(ref v) = dto.underline_style {
        elem.fmt_underline_style = Some(underline_style_to_entity(v));
    }
    if let Some(ref v) = dto.vertical_alignment {
        elem.fmt_vertical_alignment = Some(vertical_alignment_to_entity(v));
    }
    elem.updated_at = chrono::Utc::now();
}

fn execute_set_text_format(
    uow: &mut Box<dyn SetTextFormatUnitOfWorkTrait>,
    dto: &SetTextFormatDto,
) -> Result<EntityTreeSnapshot> {
    // Get Root -> Document
    let root = uow
        .get_root(&ROOT_ENTITY_ID)?
        .ok_or_else(|| anyhow!("Root entity not found"))?;
    let doc_ids = uow.get_root_relationship(&root.id, &RootRelationshipField::Document)?;
    let doc_id = *doc_ids
        .first()
        .ok_or_else(|| anyhow!("Root has no document"))?;

    let _document = uow
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

    // Determine the range
    let range_start = std::cmp::min(dto.position, dto.anchor);
    let range_end = std::cmp::max(dto.position, dto.anchor);

    if range_start == range_end {
        return Ok(snapshot);
    }

    // Process each block that overlaps the range
    for block in &blocks {
        let block_start = block.document_position;
        let block_end = block_start + block.text_length;

        // Skip blocks outside the range
        if block_end <= range_start || block_start >= range_end {
            continue;
        }

        // Get elements for this block
        let element_ids =
            uow.get_block_relationship(&block.id, &BlockRelationshipField::Elements)?;
        let elements_opt = uow.get_inline_element_multi(&element_ids)?;
        let elements: Vec<InlineElement> = elements_opt.into_iter().flatten().collect();

        let mut elem_doc_pos = block_start;

        for elem in &elements {
            let elem_len = element_char_len(elem);
            let elem_start = elem_doc_pos;
            let elem_end = elem_start + elem_len;

            // Skip elements outside the range
            if elem_end <= range_start || elem_start >= range_end {
                elem_doc_pos += elem_len;
                continue;
            }

            // Element is fully within range
            if elem_start >= range_start && elem_end <= range_end {
                let mut updated = elem.clone();
                apply_text_format(&mut updated, dto);
                uow.update_inline_element(&updated)?;
            }
            // Element needs splitting
            else if let InlineContent::Text(ref text) = elem.content {
                let local_start = std::cmp::max(0, range_start - elem_start) as usize;
                let local_end = std::cmp::min(elem_len, range_end - elem_start) as usize;
                let chars: Vec<char> = text.chars().collect();

                if local_start > 0 && local_end < chars.len() {
                    // Split into three parts: before, middle (formatted), after
                    let before_text: String = chars[..local_start].iter().collect();
                    let middle_text: String = chars[local_start..local_end].iter().collect();
                    let after_text: String = chars[local_end..].iter().collect();

                    // Update the original element to keep only the "before" text
                    let mut updated_orig = elem.clone();
                    updated_orig.content = InlineContent::Text(before_text);
                    updated_orig.updated_at = chrono::Utc::now();
                    uow.update_inline_element(&updated_orig)?;

                    // Create the middle element with formatting
                    let mut middle_elem = elem.clone();
                    middle_elem.id = 0; // Will be assigned by create
                    middle_elem.content = InlineContent::Text(middle_text);
                    apply_text_format(&mut middle_elem, dto);
                    uow.create_inline_element(&middle_elem, block.id, -1)?;

                    // Create the after element (same formatting as original)
                    let mut after_elem = elem.clone();
                    after_elem.id = 0;
                    after_elem.content = InlineContent::Text(after_text);
                    after_elem.updated_at = chrono::Utc::now();
                    uow.create_inline_element(&after_elem, block.id, -1)?;
                } else if local_start > 0 {
                    // Split into two: before (unformatted), rest (formatted)
                    let before_text: String = chars[..local_start].iter().collect();
                    let rest_text: String = chars[local_start..].iter().collect();

                    let mut updated_orig = elem.clone();
                    updated_orig.content = InlineContent::Text(before_text);
                    updated_orig.updated_at = chrono::Utc::now();
                    uow.update_inline_element(&updated_orig)?;

                    let mut new_elem = elem.clone();
                    new_elem.id = 0;
                    new_elem.content = InlineContent::Text(rest_text);
                    apply_text_format(&mut new_elem, dto);
                    uow.create_inline_element(&new_elem, block.id, -1)?;
                } else {
                    // local_start == 0, split into: formatted part, rest
                    let formatted_text: String = chars[..local_end].iter().collect();
                    let rest_text: String = chars[local_end..].iter().collect();

                    let mut updated_orig = elem.clone();
                    updated_orig.content = InlineContent::Text(formatted_text);
                    apply_text_format(&mut updated_orig, dto);
                    uow.update_inline_element(&updated_orig)?;

                    let mut new_elem = elem.clone();
                    new_elem.id = 0;
                    new_elem.content = InlineContent::Text(rest_text);
                    new_elem.updated_at = chrono::Utc::now();
                    uow.create_inline_element(&new_elem, block.id, -1)?;
                }
            }

            elem_doc_pos += elem_len;
        }
    }

    Ok(snapshot)
}

pub struct SetTextFormatUseCase {
    uow_factory: Box<dyn SetTextFormatUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<SetTextFormatDto>,
}

impl SetTextFormatUseCase {
    pub fn new(uow_factory: Box<dyn SetTextFormatUnitOfWorkFactoryTrait>) -> Self {
        SetTextFormatUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &SetTextFormatDto) -> Result<()> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let snapshot = execute_set_text_format(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());

        uow.commit()?;
        Ok(())
    }
}

impl UndoRedoCommand for SetTextFormatUseCase {
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
        let snapshot = execute_set_text_format(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
