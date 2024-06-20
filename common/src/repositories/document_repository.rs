use crate::contracts::repositories::DocumentRepositoryTrait;
use crate::entities::document::Document;

#[derive(Debug, Default)]
pub struct DocumentRepository {
    content: Document,
}

impl DocumentRepository {
    pub fn new() -> Self {
        DocumentRepository {
            content: Document::new(),
        }
    }
}

impl DocumentRepositoryTrait for DocumentRepository {
    fn get(&self) -> &Document {
        &self.content
    }

    fn get_mut(&mut self) -> &mut Document {
        &mut self.content
    }

    fn update(&mut self, document: Document) {
        self.content = document;
    }
}
