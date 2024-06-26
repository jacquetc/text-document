use im_rc::HashMap;

use crate::contracts::repositories::{ParagraphRepositoryTrait, RepositoryError, RepositoryTrait};
use crate::entities::paragraph::Paragraph;

#[derive(Debug, Clone, Default)]
pub struct ParagraphRepository {
    paragraphs: HashMap<usize, Paragraph>,
}

impl ParagraphRepository {
    pub fn new() -> Self {
        ParagraphRepository {
            paragraphs: HashMap::new(),
        }
    }
}

impl ParagraphRepositoryTrait for ParagraphRepository {}

impl RepositoryTrait<Paragraph> for ParagraphRepository {
    fn create(&mut self, entity: Paragraph) -> usize {
        let id = self.paragraphs.len();
        let mut entity = entity;
        entity.id = id;
        self.paragraphs.insert(id, entity);
        id
    }

    fn update(&mut self, entity: Paragraph) -> Result<(), RepositoryError> {
        let id = entity.id;
        if !self.paragraphs.contains_key(&id) {
            return Err(RepositoryError::IdNotFound);
        }
        self.paragraphs.insert(id, entity);
        Ok(())
    }

    fn get(&self, id: usize) -> Option<&Paragraph> {
        self.paragraphs.get(&id)
    }

    fn get_slice(&self, ids: Vec<usize>) -> Vec<&Paragraph> {
        ids.iter()
            .filter_map(|id| self.paragraphs.get(id))
            .collect()
    }

    fn get_mut(&mut self, id: usize) -> Option<&mut Paragraph> {
        self.paragraphs.get_mut(&id)
    }

    fn remove(&mut self, id: usize) -> Option<Paragraph> {
        self.paragraphs.remove(&id)
    }

    fn get_all(&self) -> Vec<&Paragraph> {
        self.paragraphs.values().collect()
    }

    fn clear(&mut self) {
        self.paragraphs.clear();
    }

    fn is_empty(&self) -> bool {
        self.paragraphs.is_empty()
    }

    fn len(&self) -> usize {
        self.paragraphs.len()
    }
}
