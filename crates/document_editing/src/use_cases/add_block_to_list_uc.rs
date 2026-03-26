use crate::AddBlockToListDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::direct_access::block::block_repository::BlockRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::entities::{Block, Document, List, Root};
use common::snapshot::EntityTreeSnapshot;
use common::types::{EntityId, ROOT_ENTITY_ID};
use common::undo_redo::UndoRedoCommand;
use std::any::Any;

pub trait AddBlockToListUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn AddBlockToListUnitOfWorkTrait>;
}

#[macros::uow_action(entity = "Root", action = "Get")]
#[macros::uow_action(entity = "Root", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Get")]
#[macros::uow_action(entity = "Document", action = "Snapshot")]
#[macros::uow_action(entity = "Document", action = "Restore")]
#[macros::uow_action(entity = "Block", action = "Get")]
#[macros::uow_action(entity = "Block", action = "SetRelationship")]
#[macros::uow_action(entity = "List", action = "Get")]
pub trait AddBlockToListUnitOfWorkTrait: CommandUnitOfWork {}

fn execute_add_block_to_list(
    uow: &mut Box<dyn AddBlockToListUnitOfWorkTrait>,
    dto: &AddBlockToListDto,
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

    // Verify both block and list exist
    let block_id = dto.block_id as EntityId;
    let _block = uow
        .get_block(&block_id)?
        .ok_or_else(|| anyhow!("Block not found with id {}", dto.block_id))?;

    let list_id = dto.list_id as EntityId;
    let _list = uow
        .get_list(&list_id)?
        .ok_or_else(|| anyhow!("List not found with id {}", dto.list_id))?;

    // Set block's list relationship
    uow.set_block_relationship(&block_id, &BlockRelationshipField::List, &[list_id])?;

    Ok(snapshot)
}

pub struct AddBlockToListUseCase {
    uow_factory: Box<dyn AddBlockToListUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<AddBlockToListDto>,
}

impl AddBlockToListUseCase {
    pub fn new(uow_factory: Box<dyn AddBlockToListUnitOfWorkFactoryTrait>) -> Self {
        AddBlockToListUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &AddBlockToListDto) -> Result<()> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let snapshot = execute_add_block_to_list(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());

        uow.commit()?;
        Ok(())
    }
}

impl UndoRedoCommand for AddBlockToListUseCase {
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
        let snapshot = execute_add_block_to_list(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
