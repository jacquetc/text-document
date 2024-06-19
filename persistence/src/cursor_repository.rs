use contracts::persistence::CursorRepositoryTrait;
use contracts::persistence::RepositoryError;
use entities::cursor::Cursor;
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
    fn create(&self, cursor: Cursor) -> usize {
        let id = if let Some(free_id) = self.free_id_list.borrow_mut().pop() {
            free_id
        } else {
            let id = self.free_id.get();
            self.free_id.set(id + 1);
            id
        };
        self.cursors.borrow_mut().insert(id, cursor);
        id
    }

    fn update(&self, cursor: Cursor) -> Result<(), RepositoryError> {
        let id = cursor.id;
        if !self.cursors.borrow().contains_key(&id) {
            return Err(RepositoryError::IdNotFound);
        }
        self.cursors.borrow_mut().insert(id, cursor);
        Ok(())
    }

    fn get(&self, id: usize) -> Option<Cursor> {
        self.cursors.borrow().get(&id).copied()
    }

    fn remove(&mut self, id: usize) -> Option<Cursor> {
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
}
