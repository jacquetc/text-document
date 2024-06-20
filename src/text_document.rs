use common::repositories::cursor_repository::CursorRepository;
use common::repositories::document_repository::DocumentRepository;

pub struct TextDocument {
    cursor_repository: CursorRepository,
    document_repository: DocumentRepository,
}

impl Default for TextDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl TextDocument {
    pub fn new() -> TextDocument {
        TextDocument {
            cursor_repository: CursorRepository::new(),
            document_repository: DocumentRepository::new(),
        }
    }

    pub(crate) fn get_cursor_repository(&self) -> &CursorRepository {
        &self.cursor_repository
    }

    pub(crate) fn get_cursor_repository_mut(&mut self) -> &mut CursorRepository {
        &mut self.cursor_repository
    }

    pub(crate) fn get_document_repository(&self) -> &DocumentRepository {
        &self.document_repository
    }

    pub(crate) fn get_document_repository_mut(&mut self) -> &mut DocumentRepository {
        &mut self.document_repository
    }

    pub fn get_plain_text(&self) -> String {
        conversion_feature::get_plain_text(&self.document_repository)
    }

    pub fn set_plain_text<T: AsRef<str>>(&mut self, text: T) {
        conversion_feature::set_plain_text(&mut self.document_repository, text.as_ref());
    }

    pub fn get_markdown(&self) -> String {
        conversion_feature::get_markdown(&self.document_repository)
    }

    pub fn set_markdown<T: AsRef<str>>(&mut self, markdown: T) {
        conversion_feature::set_markdown(&mut self.document_repository, markdown.as_ref());
    }
}

