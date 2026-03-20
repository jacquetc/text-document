use crate::SetFrameFormatDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::entities::{Document, Frame, Root};
use common::snapshot::EntityTreeSnapshot;
use common::types::EntityId;
use common::undo_redo::UndoRedoCommand;
use std::any::Any;

pub trait SetFrameFormatUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn SetFrameFormatUnitOfWorkTrait>;
}

#[macros::uow_action(entity = "Root", action = "Get")]
#[macros::uow_action(entity = "Root", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Get")]
#[macros::uow_action(entity = "Document", action = "Snapshot")]
#[macros::uow_action(entity = "Document", action = "Restore")]
#[macros::uow_action(entity = "Frame", action = "Get")]
#[macros::uow_action(entity = "Frame", action = "Update")]
pub trait SetFrameFormatUnitOfWorkTrait: CommandUnitOfWork {}

fn execute_set_frame_format(
    uow: &mut Box<dyn SetFrameFormatUnitOfWorkTrait>,
    dto: &SetFrameFormatDto,
) -> Result<EntityTreeSnapshot> {
    // Get Root -> Document
    let root = uow
        .get_root(&1)?
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

    // Look up frame by ID directly
    let frame_id = dto.frame_id as EntityId;
    let frame = uow
        .get_frame(&frame_id)?
        .ok_or_else(|| anyhow!("Frame not found with id {}", dto.frame_id))?;

    // Set frame format fields
    let mut updated = frame.clone();
    updated.fmt_height = Some(dto.height);
    updated.fmt_width = Some(dto.width);
    updated.fmt_top_margin = Some(dto.top_margin);
    updated.fmt_bottom_margin = Some(dto.bottom_margin);
    updated.fmt_left_margin = Some(dto.left_margin);
    updated.fmt_right_margin = Some(dto.right_margin);
    updated.fmt_padding = Some(dto.padding);
    updated.fmt_border = Some(dto.border);
    updated.updated_at = chrono::Utc::now();
    uow.update_frame(&updated)?;

    Ok(snapshot)
}

pub struct SetFrameFormatUseCase {
    uow_factory: Box<dyn SetFrameFormatUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<SetFrameFormatDto>,
}

impl SetFrameFormatUseCase {
    pub fn new(uow_factory: Box<dyn SetFrameFormatUnitOfWorkFactoryTrait>) -> Self {
        SetFrameFormatUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &SetFrameFormatDto) -> Result<()> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let snapshot = execute_set_frame_format(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());

        uow.commit()?;
        Ok(())
    }
}

impl UndoRedoCommand for SetFrameFormatUseCase {
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
        let snapshot = execute_set_frame_format(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
