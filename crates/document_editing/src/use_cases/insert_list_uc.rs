use crate::InsertListDto;
use crate::InsertListResultDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::entities::{Block, Document, Frame, InlineContent, InlineElement, List, Root};
use common::snapshot::EntityTreeSnapshot;
use common::types::{EntityId, ROOT_ENTITY_ID};
use common::undo_redo::UndoRedoCommand;
use std::any::Any;
use super::editing_helpers::find_block_at_position;

pub trait InsertListUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn InsertListUnitOfWorkTrait>;
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
#[macros::uow_action(entity = "InlineElement", action = "Create")]
#[macros::uow_action(entity = "List", action = "Create")]
pub trait InsertListUnitOfWorkTrait: CommandUnitOfWork {}

pub struct InsertListUseCase {
    uow_factory: Box<dyn InsertListUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<InsertListDto>,
}

/// Convert from crate's ListStyle to common::entities::ListStyle
fn convert_list_style(style: &crate::dtos::ListStyle) -> common::entities::ListStyle {
    match style {
        crate::dtos::ListStyle::Disc => common::entities::ListStyle::Disc,
        crate::dtos::ListStyle::Circle => common::entities::ListStyle::Circle,
        crate::dtos::ListStyle::Square => common::entities::ListStyle::Square,
        crate::dtos::ListStyle::Decimal => common::entities::ListStyle::Decimal,
        crate::dtos::ListStyle::LowerAlpha => common::entities::ListStyle::LowerAlpha,
        crate::dtos::ListStyle::UpperAlpha => common::entities::ListStyle::UpperAlpha,
        crate::dtos::ListStyle::LowerRoman => common::entities::ListStyle::LowerRoman,
        crate::dtos::ListStyle::UpperRoman => common::entities::ListStyle::UpperRoman,
    }
}

fn execute_insert_list(
    uow: &mut Box<dyn InsertListUnitOfWorkTrait>,
    dto: &InsertListDto,
) -> Result<(InsertListResultDto, EntityTreeSnapshot)> {
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

    // Get frames
    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
    let frame_id = *frame_ids
        .first()
        .ok_or_else(|| anyhow!("Document has no frames"))?;

    let frame = uow
        .get_frame(&frame_id)?
        .ok_or_else(|| anyhow!("Frame not found"))?;

    // Get block IDs from frame
    let block_ids = uow.get_frame_relationship(&frame_id, &FrameRelationshipField::Blocks)?;

    // Get all blocks
    let blocks_opt = uow.get_block_multi(&block_ids)?;
    let mut blocks: Vec<Block> = blocks_opt.into_iter().filter_map(|b| b).collect();
    blocks.sort_by_key(|b| b.document_position);

    // Find block at position to determine insert index
    let (_current_block, block_idx, _offset) = find_block_at_position(&blocks, position)?;

    let now = chrono::Utc::now();

    // Create the List entity
    let list = List {
        id: 0,
        created_at: now,
        updated_at: now,
        style: convert_list_style(&dto.style),
        indent: 0,
        prefix: String::new(),
        suffix: String::new(),
    };
    let created_list = uow.create_list(&list, doc_id, -1)?;

    // Create a new empty block with the list reference
    let new_block_position = if !blocks.is_empty() {
        let current = &blocks[block_idx];
        current.document_position + current.text_length + 1
    } else {
        0
    };

    let new_block = Block {
        id: 0,
        created_at: now,
        updated_at: now,
        elements: vec![],
        list: Some(created_list.id),
        text_length: 0,
        document_position: new_block_position,
        plain_text: String::new(),
        ..Default::default()
    };

    let insert_index = (block_idx + 1) as i32;
    let created_block = uow.create_block(&new_block, frame_id, insert_index)?;

    // Create an empty inline element for the new block
    let empty_elem = InlineElement {
        id: 0,
        created_at: now,
        updated_at: now,
        content: InlineContent::Empty,
        ..Default::default()
    };
    uow.create_inline_element(&empty_elem, created_block.id, -1)?;

    // Update frame's child_order
    let mut updated_frame = frame.clone();
    let child_order_insert_pos = (block_idx + 1).min(updated_frame.child_order.len());
    updated_frame
        .child_order
        .insert(child_order_insert_pos, created_block.id as i64);
    updated_frame.updated_at = now;
    updated_frame.blocks =
        uow.get_frame_relationship(&frame_id, &FrameRelationshipField::Blocks)?;
    uow.update_frame(&updated_frame)?;

    // Update subsequent blocks' document_position (those after the new block)
    let mut blocks_to_update: Vec<Block> = Vec::new();
    for b in &blocks[(block_idx + 1)..] {
        let mut ub = b.clone();
        ub.document_position += 1; // block separator
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
        InsertListResultDto {
            list_id: created_list.id as i64,
            new_position: new_block_position,
        },
        snapshot,
    ))
}

impl InsertListUseCase {
    pub fn new(uow_factory: Box<dyn InsertListUnitOfWorkFactoryTrait>) -> Self {
        InsertListUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &InsertListDto) -> Result<InsertListResultDto> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let (result, snapshot) = execute_insert_list(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());

        uow.commit()?;
        Ok(result)
    }
}

impl UndoRedoCommand for InsertListUseCase {
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
        let (_, snapshot) = execute_insert_list(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
