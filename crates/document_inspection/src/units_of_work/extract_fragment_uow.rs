use crate::use_cases::extract_fragment_uc::{
    ExtractFragmentUnitOfWorkFactoryTrait, ExtractFragmentUnitOfWorkTrait,
};
use anyhow::{Ok, Result};
use common::database::QueryUnitOfWork;
use common::database::{db_context::DbContext, transactions::Transaction};
#[allow(unused_imports)]
use common::entities::{Block, Document, Frame, InlineElement, List, Root};
#[allow(unused_imports)]
use common::types;
#[allow(unused_imports)]
use common::types::EntityId;
use std::cell::RefCell;

// Unit of work for ExtractFragment

pub struct ExtractFragmentUnitOfWork {
    context: DbContext,
    transaction: RefCell<Option<Transaction>>,
}

impl ExtractFragmentUnitOfWork {
    pub fn new(db_context: &DbContext) -> Self {
        ExtractFragmentUnitOfWork {
            context: db_context.clone(),
            transaction: RefCell::new(None),
        }
    }
}

impl QueryUnitOfWork for ExtractFragmentUnitOfWork {
    fn begin_transaction(&self) -> Result<()> {
        self.transaction
            .replace(Some(Transaction::begin_read_transaction(&self.context)?));
        Ok(())
    }

    fn end_transaction(&self) -> Result<()> {
        self.transaction.take().unwrap().end_read_transaction()?;
        Ok(())
    }
}

#[macros::uow_action(entity = "Root", action = "GetRO")]
#[macros::uow_action(entity = "Root", action = "GetRelationshipRO")]
#[macros::uow_action(entity = "Document", action = "GetRelationshipRO")]
#[macros::uow_action(entity = "Frame", action = "GetRelationshipRO")]
#[macros::uow_action(entity = "Block", action = "GetMultiRO")]
#[macros::uow_action(entity = "Block", action = "GetRelationshipRO")]
#[macros::uow_action(entity = "InlineElement", action = "GetMultiRO")]
#[macros::uow_action(entity = "List", action = "GetRO")]
impl ExtractFragmentUnitOfWorkTrait for ExtractFragmentUnitOfWork {}

pub struct ExtractFragmentUnitOfWorkFactory {
    context: DbContext,
}

impl ExtractFragmentUnitOfWorkFactory {
    pub fn new(db_context: &DbContext) -> Self {
        ExtractFragmentUnitOfWorkFactory {
            context: db_context.clone(),
        }
    }
}

impl ExtractFragmentUnitOfWorkFactoryTrait for ExtractFragmentUnitOfWorkFactory {
    fn create(&self) -> Box<dyn ExtractFragmentUnitOfWorkTrait> {
        Box::new(ExtractFragmentUnitOfWork::new(&self.context))
    }
}
