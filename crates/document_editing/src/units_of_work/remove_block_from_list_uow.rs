use crate::use_cases::remove_block_from_list_uc::{
    RemoveBlockFromListUnitOfWorkFactoryTrait, RemoveBlockFromListUnitOfWorkTrait,
};
use anyhow::{Ok, Result};
use common::database::CommandUnitOfWork;
use common::database::{db_context::DbContext, transactions::Transaction};
#[allow(unused_imports)]
use common::entities::{Block, Document, List, Root};
use common::event::{AllEvent, DirectAccessEntity, Event, EventBuffer, EventHub, Origin};
#[allow(unused_imports)]
use common::types;
#[allow(unused_imports)]
use common::types::EntityId;
use std::cell::RefCell;
use std::sync::Arc;

pub struct RemoveBlockFromListUnitOfWork {
    context: DbContext,
    transaction: Option<Transaction>,
    event_hub: Arc<EventHub>,
    event_buffer: RefCell<EventBuffer>,
}

impl RemoveBlockFromListUnitOfWork {
    pub fn new(db_context: &DbContext, event_hub: &Arc<EventHub>) -> Self {
        RemoveBlockFromListUnitOfWork {
            context: db_context.clone(),
            transaction: None,
            event_hub: event_hub.clone(),
            event_buffer: RefCell::new(EventBuffer::new()),
        }
    }
}

impl CommandUnitOfWork for RemoveBlockFromListUnitOfWork {
    fn begin_transaction(&mut self) -> Result<()> {
        self.transaction = Some(Transaction::begin_write_transaction(&self.context)?);
        self.event_buffer.get_mut().begin_buffering();
        Ok(())
    }

    fn commit(&mut self) -> Result<()> {
        self.transaction.take().unwrap().commit()?;
        for event in self.event_buffer.get_mut().flush() {
            self.event_hub.send_event(event);
        }
        Ok(())
    }

    fn rollback(&mut self) -> Result<()> {
        self.transaction.take().unwrap().rollback()?;
        self.event_buffer.get_mut().discard();
        Ok(())
    }

    fn create_savepoint(&self) -> Result<types::Savepoint> {
        self.transaction.as_ref().unwrap().create_savepoint()
    }

    fn restore_to_savepoint(&mut self, savepoint: types::Savepoint) -> Result<()> {
        let mut transaction = self.transaction.take().unwrap();
        transaction.restore_to_savepoint(savepoint)?;

        self.event_buffer.get_mut().discard();

        self.event_hub.send_event(Event {
            origin: Origin::DirectAccess(DirectAccessEntity::All(AllEvent::Reset)),
            ids: vec![],
            data: None,
        });

        self.transaction = Some(transaction);

        Ok(())
    }
}

#[macros::uow_action(entity = "Root", action = "Get")]
#[macros::uow_action(entity = "Root", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Get")]
#[macros::uow_action(entity = "Document", action = "Snapshot")]
#[macros::uow_action(entity = "Document", action = "Restore")]
#[macros::uow_action(entity = "Block", action = "Get")]
#[macros::uow_action(entity = "Block", action = "GetAll")]
#[macros::uow_action(entity = "Block", action = "SetRelationship")]
#[macros::uow_action(entity = "List", action = "Get")]
#[macros::uow_action(entity = "List", action = "Remove")]
impl RemoveBlockFromListUnitOfWorkTrait for RemoveBlockFromListUnitOfWork {}

pub struct RemoveBlockFromListUnitOfWorkFactory {
    context: DbContext,
    event_hub: Arc<EventHub>,
}

impl RemoveBlockFromListUnitOfWorkFactory {
    pub fn new(db_context: &DbContext, event_hub: &Arc<EventHub>) -> Self {
        RemoveBlockFromListUnitOfWorkFactory {
            context: db_context.clone(),
            event_hub: event_hub.clone(),
        }
    }
}

impl RemoveBlockFromListUnitOfWorkFactoryTrait for RemoveBlockFromListUnitOfWorkFactory {
    fn create(&self) -> Box<dyn RemoveBlockFromListUnitOfWorkTrait> {
        Box::new(RemoveBlockFromListUnitOfWork::new(
            &self.context,
            &self.event_hub,
        ))
    }
}
