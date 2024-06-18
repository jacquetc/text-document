use domain::cursor::Cursor;
use domain::document::Document;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("Id not found.")]
    IdNotFound,
}

pub trait RepositoryProviderTrait {
    fn get_document_repository(&self) -> &dyn DocumentRepositoryTrait;
    fn get_document_repository_mut(&mut self) -> &mut dyn DocumentRepositoryTrait;
    fn get_cursor_repository(&self) -> &dyn CursorRepositoryTrait;
    fn get_cursor_repository_mut(&mut self) -> &mut dyn CursorRepositoryTrait;
}

pub trait DocumentRepositoryTrait {
    fn get(&self) -> &Document;
    fn get_mut(&mut self) -> &mut Document;
    fn update(&mut self, document: Document);
}

pub trait CursorRepositoryTrait {
    fn create(&self, cursor: Cursor) -> usize;
    fn update(&self, cursor: Cursor) -> Result<(), RepositoryError>;
    fn get(&self, id: usize) -> Option<Cursor>;
    fn remove(&mut self, id: usize) -> Option<Cursor>;
    fn get_all(&self) -> Vec<Cursor>;
    fn clear(&mut self);
    fn is_empty(&self) -> bool;
}
