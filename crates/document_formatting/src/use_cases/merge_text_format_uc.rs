use crate::MergeTextFormatDto;
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

pub trait MergeTextFormatUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn MergeTextFormatUnitOfWorkTrait>;
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
pub trait MergeTextFormatUnitOfWorkTrait: CommandUnitOfWork {}

/// Apply the merge dto onto a CharacterFormat, overwriting only fields the
/// dto sets to `Some(_)`. Non-empty `font_family` follows the original
/// semantic (empty string was treated as "no change" by the legacy code).
fn merge_dto(base: &CharacterFormat, dto: &MergeTextFormatDto) -> CharacterFormat {
    let mut out = base.clone();
    if let Some(ref family) = dto.font_family
        && !family.is_empty()
    {
        out.font_family = Some(family.clone());
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
    if let Some(v) = dto.font_strikeout {
        out.font_strikeout = Some(v);
    }
    out
}

fn char_to_byte(plain_text: &str, char_offset: usize) -> u32 {
    plain_text
        .char_indices()
        .nth(char_offset)
        .map(|(b, _)| b as u32)
        .unwrap_or(plain_text.len() as u32)
}

fn build_replacement_runs(
    existing_runs: &[FormatRun],
    byte_start: u32,
    byte_end: u32,
    dto: &MergeTextFormatDto,
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

fn execute_merge_text_format(
    uow: &mut Box<dyn MergeTextFormatUnitOfWorkTrait>,
    dto: &MergeTextFormatDto,
) -> Result<EntityTreeSnapshot> {
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

        let local_char_start =
            std::cmp::max(0, range_start - block_start) as usize;
        let local_char_end =
            std::cmp::min(block_char_length(block, &store), range_end - block_start) as usize;

        let block_text = block_content_via_store(block, &store);
        let plain_text_len = block_text.chars().count();
        let text_char_start = std::cmp::min(local_char_start, plain_text_len);
        let text_char_end = std::cmp::min(local_char_end, plain_text_len);
        let byte_start = char_to_byte(&block_text, text_char_start);
        let byte_end = char_to_byte(&block_text, text_char_end);

        if byte_start < byte_end {
            let mut runs_map = store.format_runs.write().unwrap();
            let runs = runs_map.entry(block.id).or_default();
            let replacement = build_replacement_runs(runs, byte_start, byte_end, dto);
            splice_range(runs, byte_start..byte_end, replacement);
            debug_assert_well_formed(runs, block_text.len());
        }

        // Update image anchor formats inside the selected byte range.
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

pub struct MergeTextFormatUseCase {
    uow_factory: Box<dyn MergeTextFormatUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<MergeTextFormatDto>,
}

impl MergeTextFormatUseCase {
    pub fn new(uow_factory: Box<dyn MergeTextFormatUnitOfWorkFactoryTrait>) -> Self {
        MergeTextFormatUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &MergeTextFormatDto) -> Result<()> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let snapshot = execute_merge_text_format(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());

        uow.commit()?;
        Ok(())
    }
}

impl UndoRedoCommand for MergeTextFormatUseCase {
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
        let snapshot = execute_merge_text_format(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
