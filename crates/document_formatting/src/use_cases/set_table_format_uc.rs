use crate::SetTableFormatDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::entities::{Document, Root, Table};
use common::snapshot::EntityTreeSnapshot;
use common::types::{EntityId, ROOT_ENTITY_ID};
use common::undo_redo::UndoRedoCommand;
use std::any::Any;

pub trait SetTableFormatUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn SetTableFormatUnitOfWorkTrait>;
}

#[macros::uow_action(entity = "Root", action = "Get")]
#[macros::uow_action(entity = "Root", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Get")]
#[macros::uow_action(entity = "Document", action = "Snapshot")]
#[macros::uow_action(entity = "Document", action = "Restore")]
#[macros::uow_action(entity = "Table", action = "Get")]
#[macros::uow_action(entity = "Table", action = "Update")]
pub trait SetTableFormatUnitOfWorkTrait: CommandUnitOfWork {}

fn alignment_to_entity(a: &crate::dtos::Alignment) -> common::entities::Alignment {
    match a {
        crate::dtos::Alignment::Left => common::entities::Alignment::Left,
        crate::dtos::Alignment::Right => common::entities::Alignment::Right,
        crate::dtos::Alignment::Center => common::entities::Alignment::Center,
        crate::dtos::Alignment::Justify => common::entities::Alignment::Justify,
    }
}

fn execute_set_table_format(
    uow: &mut Box<dyn SetTableFormatUnitOfWorkTrait>,
    dto: &SetTableFormatDto,
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

    // Look up table by ID directly
    let table_id = dto.table_id as EntityId;
    let table = uow
        .get_table(&table_id)?
        .ok_or_else(|| anyhow!("Table not found with id {}", dto.table_id))?;

    // Set table format fields -- None means preserve existing value.
    let mut updated = table.clone();
    if let Some(v) = dto.border {
        updated.fmt_border = Some(v);
    }
    if let Some(v) = dto.cell_spacing {
        updated.fmt_cell_spacing = Some(v);
    }
    if let Some(v) = dto.cell_padding {
        updated.fmt_cell_padding = Some(v);
    }
    if let Some(v) = dto.width {
        updated.fmt_width = Some(v);
    }
    if let Some(ref a) = dto.alignment {
        updated.fmt_alignment = Some(alignment_to_entity(a));
    }
    updated.updated_at = chrono::Utc::now();
    uow.update_table(&updated)?;

    Ok(snapshot)
}

pub struct SetTableFormatUseCase {
    uow_factory: Box<dyn SetTableFormatUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<SetTableFormatDto>,
}

impl SetTableFormatUseCase {
    pub fn new(uow_factory: Box<dyn SetTableFormatUnitOfWorkFactoryTrait>) -> Self {
        SetTableFormatUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &SetTableFormatDto) -> Result<()> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let snapshot = execute_set_table_format(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());

        uow.commit()?;
        Ok(())
    }
}

impl UndoRedoCommand for SetTableFormatUseCase {
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
        let snapshot = execute_set_table_format(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
