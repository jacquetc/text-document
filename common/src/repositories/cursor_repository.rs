use crate::contracts::repositories::CursorRepositoryTrait;
use crate::contracts::repositories::RepositoryError;
use crate::entities::cursor::Cursor;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct CursorRepository {
    cursors: RefCell<HashMap<usize, Cursor>>,
    free_id: Cell<usize>,
    free_id_list: RefCell<Vec<usize>>,
}

impl CursorRepository {
    pub fn new() -> CursorRepository {
        CursorRepository::default()
    }
}

impl CursorRepositoryTrait for CursorRepository {
    fn create(&self, entity: Cursor) -> usize {
        let id = if let Some(free_id) = self.free_id_list.borrow_mut().pop() {
            free_id
        } else {
            let id = self.free_id.get();
            self.free_id.set(id + 1);
            id
        };
        let mut entity = entity;
        entity.id = id;
        self.cursors.borrow_mut().insert(id, entity);
        id
    }

    fn update(&self, entity: Cursor) -> Result<(), RepositoryError> {
        let id = entity.id;
        if !self.cursors.borrow().contains_key(&id) {
            return Err(RepositoryError::IdNotFound);
        }
        self.cursors.borrow_mut().insert(id, entity);
        Ok(())
    }

    fn get(&self, id: usize) -> Option<Cursor> {
        self.cursors.borrow().get(&id).copied()
    }

    fn get_slice(&self, ids: Vec<usize>) -> Vec<Cursor> {
        ids.iter()
            .filter_map(|id| self.cursors.borrow().get(id).copied())
            .collect()
    }

    fn remove(&self, id: usize) -> Option<Cursor> {
        self.cursors.borrow_mut().remove(&id).map(|cursor| {
            self.free_id_list.borrow_mut().push(id);
            cursor
        })
    }

    fn get_all(&self) -> Vec<Cursor> {
        self.cursors.borrow().values().copied().collect()
    }

    fn clear(&mut self) {
        self.cursors.borrow_mut().clear();
    }

    fn is_empty(&self) -> bool {
        self.cursors.borrow().is_empty()
    }

    fn len(&self) -> usize {
        self.cursors.borrow().len()
    }
}
