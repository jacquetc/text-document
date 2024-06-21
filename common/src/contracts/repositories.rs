use crate::entities::cursor::Cursor;
use crate::entities::document::Document;
use crate::entities::paragraph::Paragraph;
use crate::entities::paragraph_group::ParagraphGroup;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("Id not found.")]
    IdNotFound,
}

pub trait RepositoryTrait<T> {
    fn create(&mut self, entity: T) -> usize;
    fn update(&mut self, entity: T) -> Result<(), RepositoryError>;
    fn get(&self, id: usize) -> Option<&T>;
    fn get_slice(&self, ids: Vec<usize>) -> Vec<&T>;
    fn remove(&mut self, id: usize) -> Option<T>;
    fn get_all(&self) -> Vec<&T>;
    fn clear(&mut self);
    fn is_empty(&self) -> bool;
    fn len(&self) -> usize;
}

pub trait ParagraphRepositoryTrait: RepositoryTrait<Paragraph> {}

pub trait ParagraphGroupRepositoryTrait: RepositoryTrait<ParagraphGroup> {
    fn add_paragraph_to_a_group(&mut self, paragraph: &mut Paragraph);
    fn remove_paragraph_from_a_group(&mut self, paragraph: &Paragraph);
    fn update_paragraph_group(&mut self, old_paragraph: &Paragraph, new_paragraph: &Paragraph);
}

pub trait CursorRepositoryTrait {
    fn create(&self, entity: Cursor) -> usize;
    fn update(&self, entity: Cursor) -> Result<(), RepositoryError>;
    fn get(&self, id: usize) -> Option<Cursor>;
    fn get_slice(&self, ids: Vec<usize>) -> Vec<Cursor>;
    fn remove(&self, id: usize) -> Option<Cursor>;
    fn get_all(&self) -> Vec<Cursor>;
    fn clear(&mut self);
    fn is_empty(&self) -> bool;
    fn len(&self) -> usize;
}

pub trait DocumentRepositoryTrait {
    fn get(&self) -> &Document;
    fn get_mut(&mut self) -> &mut Document;
    fn update(&mut self, document: Document);
}
