use crate::database::db_context::DbContext;
use crate::database::hashmap_store::HashMapStore;
use anyhow::{Ok, Result, bail};
use std::sync::Arc;

use crate::types;

pub struct Transaction {
    store: Arc<HashMapStore>,
    is_write: bool,
}

impl Transaction {
    pub fn begin_write_transaction(db_context: &DbContext) -> Result<Transaction> {
        Ok(Transaction {
            store: Arc::clone(db_context.get_store()),
            is_write: true,
        })
    }

    pub fn begin_read_transaction(db_context: &DbContext) -> Result<Transaction> {
        Ok(Transaction {
            store: Arc::clone(db_context.get_store()),
            is_write: false,
        })
    }

    pub fn commit(&mut self) -> Result<()> {
        if !self.is_write {
            bail!("Cannot commit a read transaction");
        }
        // HashMap store commits are immediate — nothing to do
        Ok(())
    }

    pub fn rollback(&mut self) -> Result<()> {
        if !self.is_write {
            bail!("Cannot rollback a read transaction");
        }
        // No-op: mutations are already applied in-place.
        // Real rollback is done via restore_to_savepoint.
        Ok(())
    }

    pub fn end_read_transaction(&mut self) -> Result<()> {
        if self.is_write {
            bail!("Cannot end a write transaction as read");
        }
        Ok(())
    }

    pub fn get_store(&self) -> &HashMapStore {
        &self.store
    }

    pub fn create_savepoint(&self) -> Result<types::Savepoint> {
        if !self.is_write {
            bail!("Cannot create savepoint on a read transaction");
        }
        Ok(self.store.create_savepoint())
    }

    pub fn restore_to_savepoint(&mut self, savepoint: types::Savepoint) -> Result<()> {
        if !self.is_write {
            bail!("Cannot restore savepoint on a read transaction");
        }
        self.store.restore_savepoint(savepoint);
        Ok(())
    }
}
