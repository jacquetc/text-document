use crate::SetTextFormatDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::database::rope_helpers::{block_char_length, block_content_via_store};
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::entities::{Block, Document, Frame, Root};
use common::format_runs::{
    CharacterFormat, FormatRun, debug_assert_well_formed, splice_range,
};
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

/// Apply a SetTextFormatDto onto a CharacterFormat, overwriting only the
/// fields the dto sets to `Some(_)`. Returns the merged format.
fn merge_dto(base: &CharacterFormat, dto: &SetTextFormatDto) -> CharacterFormat {
    let mut out = base.clone();
    if let Some(ref v) = dto.font_family {
        out.font_family = Some(v.clone());
    }
    if let Some(v) = dto.font_point_size {
        out.font_point_size = Some(v);
    }
    if let Some(v) = dto.font_weight {
        out.font_weight = Some(v);
    }
    if let Some(v) = dto.font_bold {
        out.font_bold = Some(v);
    }
    if let Some(v) = dto.font_italic {
        out.font_italic = Some(v);
    }
    if let Some(v) = dto.font_underline {
        out.font_underline = Some(v);
    }
    if let Some(v) = dto.font_overline {
        out.font_overline = Some(v);
    }
    if let Some(v) = dto.font_strikeout {
        out.font_strikeout = Some(v);
    }
    if let Some(v) = dto.letter_spacing {
        out.letter_spacing = Some(v);
    }
    if let Some(v) = dto.word_spacing {
        out.word_spacing = Some(v);
    }
    if let Some(ref v) = dto.underline_style {
        out.underline_style = Some(underline_style_to_entity(v));
    }
    if let Some(ref v) = dto.vertical_alignment {
        out.vertical_alignment = Some(vertical_alignment_to_entity(v));
    }
    out
}

/// Map a char offset inside `plain_text` to the corresponding UTF-8 byte
/// offset. Inputs past the end of the string clamp to `plain_text.len()`.
fn char_to_byte(plain_text: &str, char_offset: usize) -> u32 {
    plain_text
        .char_indices()
        .nth(char_offset)
        .map(|(b, _)| b as u32)
        .unwrap_or(plain_text.len() as u32)
}

/// Build the replacement run list covering `[byte_start..byte_end)` of a
/// block, merging the dto's fields onto every existing run (and gaps
/// between runs default to `CharacterFormat::default()` before merging).
fn build_replacement_runs(
    existing_runs: &[FormatRun],
    byte_start: u32,
    byte_end: u32,
    dto: &SetTextFormatDto,
) -> Vec<FormatRun> {
    let mut out: Vec<FormatRun> = Vec::new();
    let mut cursor = byte_start;
    for run in existing_runs {
        if run.byte_end <= byte_start || run.byte_start >= byte_end {
            continue;
        }
        let overlap_start = std::cmp::max(run.byte_start, byte_start);
        let overlap_end = std::cmp::min(run.byte_end, byte_end);
        if overlap_start > cursor {
            out.push(FormatRun {
                byte_start: cursor,
                byte_end: overlap_start,
                format: merge_dto(&CharacterFormat::default(), dto),
            });
        }
        out.push(FormatRun {
            byte_start: overlap_start,
            byte_end: overlap_end,
            format: merge_dto(&run.format, dto),
        });
        cursor = overlap_end;
    }
    if cursor < byte_end {
        out.push(FormatRun {
            byte_start: cursor,
            byte_end,
            format: merge_dto(&CharacterFormat::default(), dto),
        });
    }
    out
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

    // Snapshot for undo before mutation (covers format_runs and block_images).
    let snapshot = uow.snapshot_document(&[doc_id])?;

    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;

    let mut all_block_ids = Vec::new();
    for fid in &frame_ids {
        let block_ids = uow.get_frame_relationship(fid, &FrameRelationshipField::Blocks)?;
        all_block_ids.extend(block_ids);
    }

    let blocks_opt = uow.get_block_multi(&all_block_ids)?;
    let mut blocks: Vec<Block> = blocks_opt.into_iter().flatten().collect();
    blocks.sort_by_key(|b| b.document_position);

    let range_start = std::cmp::min(dto.position, dto.anchor);
    let range_end = std::cmp::max(dto.position, dto.anchor);

    if range_start == range_end {
        return Ok(snapshot);
    }

    let store = uow.store();
    for block in &blocks {
        let block_start = block.document_position;
        let block_end = block_start + block_char_length(block, &store);

        if block_end <= range_start || block_start >= range_end {
            continue;
        }

        // Char-relative range within this block.
        let local_char_start =
            std::cmp::max(0, range_start - block_start) as usize;
        let local_char_end =
            std::cmp::min(block_char_length(block, &store), range_end - block_start) as usize;

        // Translate char offsets to byte offsets in plain_text. Images contribute
        // 0 bytes to plain_text but 1 to text_length; we adjust by counting
        // image anchors with byte_offset < local_char_end - existing_text_bytes.
        // Simpler: clamp the byte range to the plain_text length and update
        // image anchor formats whose byte_offset falls inside the char range
        // (treating each image as a single logical character).
        let store = uow.store();
        let block_text = block_content_via_store(block, &store);
        let plain_text_len = block_text.chars().count();
        let text_char_start = std::cmp::min(local_char_start, plain_text_len);
        let text_char_end = std::cmp::min(local_char_end, plain_text_len);
        let byte_start = char_to_byte(&block_text, text_char_start);
        let byte_end = char_to_byte(&block_text, text_char_end);

        // Update format runs over the byte range.
        if byte_start < byte_end {
            let mut runs_map = store.format_runs.write().unwrap();
            let runs = runs_map.entry(block.id).or_default();
            let replacement = build_replacement_runs(runs, byte_start, byte_end, dto);
            splice_range(runs, byte_start..byte_end, replacement);
            debug_assert_well_formed(runs, block_text.len());
        }

        // Update image anchor formats. An image at `byte_offset` is in the
        // selection if its byte_offset is inside [byte_start..byte_end].
        // Images use the same merge_dto semantics as text runs.
        {
            let mut images_map = store.block_images.write().unwrap();
            if let Some(images) = images_map.get_mut(&block.id) {
                for img in images.iter_mut() {
                    if img.byte_offset >= byte_start && img.byte_offset < byte_end {
                        img.format = merge_dto(&img.format, dto);
                    }
                }
            }
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
