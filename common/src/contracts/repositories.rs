use crate::entities::cursor::Cursor;
use crate::entities::document::Document;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("Id not found.")]
    IdNotFound,
}


pub trait CursorRepositoryTrait {
    fn create(&self, cursor: Cursor) -> usize;
    fn update(&self, cursor: Cursor) -> Result<(), RepositoryError>;
    fn get(&self, id: usize) -> Option<Cursor>;
    fn remove(&self, id: usize) -> Option<Cursor>;
    fn get_all(&self) -> Vec<Cursor>;
    fn clear(&mut self);
    fn is_empty(&self) -> bool;
}

pub trait DocumentRepositoryTrait {
    fn get(&self) -> &Document;
    fn get_mut(&mut self) -> &mut Document;
    fn update(&mut self, document: Document);
}