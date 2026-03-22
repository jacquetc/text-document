use crate::InsertFrameDto;
use crate::InsertFrameResultDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::entities::{Block, Document, Frame, InlineContent, InlineElement, Root};
use common::snapshot::EntityTreeSnapshot;
use common::types::{EntityId, ROOT_ENTITY_ID};
use common::undo_redo::UndoRedoCommand;
use std::any::Any;

pub trait InsertFrameUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn InsertFrameUnitOfWorkTrait>;
}

#[macros::uow_action(entity = "Root", action = "Get")]
#[macros::uow_action(entity = "Root", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Get")]
#[macros::uow_action(entity = "Document", action = "Update")]
#[macros::uow_action(entity = "Document", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Snapshot")]
#[macros::uow_action(entity = "Document", action = "Restore")]
#[macros::uow_action(entity = "Frame", action = "Get")]
#[macros::uow_action(entity = "Frame", action = "Create")]
#[macros::uow_action(entity = "Frame", action = "Update")]
#[macros::uow_action(entity = "Frame", action = "GetRelationship")]
#[macros::uow_action(entity = "Block", action = "GetMulti")]
#[macros::uow_action(entity = "Block", action = "Create")]
#[macros::uow_action(entity = "InlineElement", action = "Create")]
pub trait InsertFrameUnitOfWorkTrait: CommandUnitOfWork {}

pub struct InsertFrameUseCase {
    uow_factory: Box<dyn InsertFrameUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<InsertFrameDto>,
}

/// Find which frame contains the given document position by walking
/// frames -> blocks and checking document_position ranges.
/// Returns the frame and the block index within it closest to position.
fn find_frame_at_position(
    uow: &dyn InsertFrameUnitOfWorkTrait,
    frame_ids: &[EntityId],
    position: i64,
) -> Result<Option<(Frame, usize)>> {
    for frame_id in frame_ids {
        let frame = match uow.get_frame(frame_id)? {
            Some(f) => f,
            None => continue,
        };
        let block_ids = uow.get_frame_relationship(frame_id, &FrameRelationshipField::Blocks)?;
        if block_ids.is_empty() {
            continue;
        }
        let blocks_opt = uow.get_block_multi(&block_ids)?;
        let mut blocks: Vec<Block> = blocks_opt.into_iter().flatten().collect();
        blocks.sort_by_key(|b| b.document_position);

        if let (Some(first), Some(last)) = (blocks.first(), blocks.last()) {
            let frame_start = first.document_position;
            let frame_end = last.document_position + last.text_length;
            if position >= frame_start && position <= frame_end {
                // Find block index closest to position
                let mut block_idx = 0;
                for (i, block) in blocks.iter().enumerate() {
                    if position <= block.document_position + block.text_length {
                        block_idx = i;
                        break;
                    }
                    block_idx = i;
                }
                return Ok(Some((frame, block_idx)));
            }
        }
    }
    Ok(None)
}

fn execute_insert_frame(
    uow: &mut Box<dyn InsertFrameUnitOfWorkTrait>,
    dto: &InsertFrameDto,
) -> Result<(InsertFrameResultDto, EntityTreeSnapshot)> {
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

    let now = chrono::Utc::now();

    // Determine the parent frame from the position
    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;

    let (parent_frame_id, child_order_insert_idx) =
        match find_frame_at_position(&**uow, &frame_ids, dto.position)? {
            Some((parent_frame, block_idx)) => {
                // Insert the new sub-frame into the parent's child_order
                // after the block at block_idx
                let insert_idx = (block_idx + 1).min(parent_frame.child_order.len());
                (Some(parent_frame.id), insert_idx)
            }
            None => {
                // Position doesn't fall in any frame — append as top-level
                (None, 0)
            }
        };

    // Create a new Frame with parent reference
    let new_frame = Frame {
        id: 0,
        created_at: now,
        updated_at: now,
        parent_frame: parent_frame_id,
        blocks: vec![],
        child_order: vec![],
        fmt_height: None,
        fmt_width: None,
        fmt_top_margin: None,
        fmt_bottom_margin: None,
        fmt_left_margin: None,
        fmt_right_margin: None,
        fmt_padding: None,
        fmt_border: None,
        fmt_position: None,
    };

    let created_frame = uow.create_frame(&new_frame, doc_id, -1)?;

    // Create an empty block inside the new frame
    let new_block = Block {
        id: 0,
        created_at: now,
        updated_at: now,
        elements: vec![],
        list: None,
        text_length: 0,
        document_position: 0,
        plain_text: String::new(),
        ..Default::default()
    };

    let created_block = uow.create_block(&new_block, created_frame.id, -1)?;

    // Create an empty inline element in the new block
    let empty_elem = InlineElement {
        id: 0,
        created_at: now,
        updated_at: now,
        content: InlineContent::Empty,
        ..Default::default()
    };
    uow.create_inline_element(&empty_elem, created_block.id, -1)?;

    // Update the new frame's child_order with its block
    let mut updated_new_frame = created_frame.clone();
    updated_new_frame.child_order = vec![created_block.id as i64];
    updated_new_frame.updated_at = now;
    uow.update_frame(&updated_new_frame)?;

    // If there's a parent frame, insert the new frame into its child_order
    if let Some(parent_id) = parent_frame_id {
        let parent_frame = uow
            .get_frame(&parent_id)?
            .ok_or_else(|| anyhow!("Parent frame not found"))?;
        let mut updated_parent = parent_frame.clone();
        let idx = child_order_insert_idx.min(updated_parent.child_order.len());
        // Use negative IDs to distinguish sub-frame references from block IDs
        // Convention: positive = block ID, negative = -(frame ID)
        updated_parent
            .child_order
            .insert(idx, -(created_frame.id as i64));
        updated_parent.updated_at = now;
        uow.update_frame(&updated_parent)?;
    }

    // Update Document (increment block_count for the new block)
    let mut updated_doc = document.clone();
    updated_doc.block_count += 1;
    updated_doc.updated_at = now;
    uow.update_document(&updated_doc)?;

    Ok((
        InsertFrameResultDto {
            frame_id: created_frame.id as i64,
        },
        snapshot,
    ))
}

impl InsertFrameUseCase {
    pub fn new(uow_factory: Box<dyn InsertFrameUnitOfWorkFactoryTrait>) -> Self {
        InsertFrameUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &InsertFrameDto) -> Result<InsertFrameResultDto> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let (result, snapshot) = execute_insert_frame(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());

        uow.commit()?;
        Ok(result)
    }
}

impl UndoRedoCommand for InsertFrameUseCase {
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
        let (_, snapshot) = execute_insert_frame(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
