use crate::SetTableCellFormatDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::entities::{Document, Root, TableCell};
use common::snapshot::EntityTreeSnapshot;
use common::types::{EntityId, ROOT_ENTITY_ID};
use common::undo_redo::UndoRedoCommand;
use std::any::Any;

pub trait SetTableCellFormatUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn SetTableCellFormatUnitOfWorkTrait>;
}

#[macros::uow_action(entity = "Root", action = "Get")]
#[macros::uow_action(entity = "Root", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Get")]
#[macros::uow_action(entity = "Document", action = "Snapshot")]
#[macros::uow_action(entity = "Document", action = "Restore")]
#[macros::uow_action(entity = "TableCell", action = "Get")]
#[macros::uow_action(entity = "TableCell", action = "Update")]
pub trait SetTableCellFormatUnitOfWorkTrait: CommandUnitOfWork {}

fn cell_vertical_alignment_to_entity(
    a: &crate::dtos::CellVerticalAlignment,
) -> common::entities::CellVerticalAlignment {
    match a {
        crate::dtos::CellVerticalAlignment::Top => common::entities::CellVerticalAlignment::Top,
        crate::dtos::CellVerticalAlignment::Middle => {
            common::entities::CellVerticalAlignment::Middle
        }
        crate::dtos::CellVerticalAlignment::Bottom => {
            common::entities::CellVerticalAlignment::Bottom
        }
    }
}

fn execute_set_table_cell_format(
    uow: &mut Box<dyn SetTableCellFormatUnitOfWorkTrait>,
    dto: &SetTableCellFormatDto,
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

    // Look up table cell by ID directly
    let cell_id = dto.cell_id as EntityId;
    let cell = uow
        .get_table_cell(&cell_id)?
        .ok_or_else(|| anyhow!("TableCell not found with id {}", dto.cell_id))?;

    // Set table cell format fields -- None means preserve existing value.
    let mut updated = cell.clone();
    if let Some(v) = dto.padding {
        updated.fmt_padding = Some(v);
    }
    if let Some(v) = dto.border {
        updated.fmt_border = Some(v);
    }
    if let Some(ref a) = dto.vertical_alignment {
        updated.fmt_vertical_alignment = Some(cell_vertical_alignment_to_entity(a));
    }
    if let Some(ref v) = dto.background_color {
        updated.fmt_background_color = Some(v.clone());
    }
    updated.updated_at = chrono::Utc::now();
    uow.update_table_cell(&updated)?;

    Ok(snapshot)
}

pub struct SetTableCellFormatUseCase {
    uow_factory: Box<dyn SetTableCellFormatUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<SetTableCellFormatDto>,
}

impl SetTableCellFormatUseCase {
    pub fn new(uow_factory: Box<dyn SetTableCellFormatUnitOfWorkFactoryTrait>) -> Self {
        SetTableCellFormatUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &SetTableCellFormatDto) -> Result<()> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let snapshot = execute_set_table_cell_format(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());

        uow.commit()?;
        Ok(())
    }
}

impl UndoRedoCommand for SetTableCellFormatUseCase {
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
        let snapshot = execute_set_table_cell_format(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
