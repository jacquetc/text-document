use common::repositories::cursor_repository::CursorRepository;
use common::repositories::document_repository::DocumentRepository;
use common::repositories::paragraph_repository::ParagraphRepository;
use common::repositories::paragraph_group_repository::ParagraphGroupRepository;

pub struct TextDocument {
    cursor_repository: CursorRepository,
    document_repository: DocumentRepository,
    paragraph_repository: ParagraphRepository,
    paragraph_group_repository: ParagraphGroupRepository,
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
            paragraph_repository: ParagraphRepository::new(),
            paragraph_group_repository: ParagraphGroupRepository::new(),
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

    pub(crate) fn get_paragraph_repository(&self) -> &ParagraphRepository {
        &self.paragraph_repository
    }

    pub(crate) fn get_paragraph_repository_mut(&mut self) -> &mut ParagraphRepository {
        &mut self.paragraph_repository
    }

    pub(crate) fn get_paragraph_group_repository(&self) -> &ParagraphGroupRepository {
        &self.paragraph_group_repository
    }

    pub(crate) fn get_paragraph_group_repository_mut(&mut self) -> &mut ParagraphGroupRepository {
        &mut self.paragraph_group_repository
    }

    pub fn get_plain_text(&self) -> String {
        conversion_feature::get_plain_text(&self.document_repository, &self.paragraph_repository)
    }

    pub fn set_plain_text<T: AsRef<str>>(&mut self, text: T) {
        conversion_feature::set_plain_text(
            &mut self.document_repository,
            &mut self.paragraph_repository,
            &mut self.paragraph_group_repository,
            text.as_ref(),
        );
    }

    pub fn get_markdown(&self) -> String {
        conversion_feature::get_markdown(&self.document_repository, &self.paragraph_repository)
    }

    pub fn set_markdown<T: AsRef<str>>(&mut self, markdown: T) {
        conversion_feature::set_markdown(
            &mut self.document_repository,
            &mut self.paragraph_repository,
            &mut self.paragraph_group_repository,
            markdown.as_ref(),
        );
    }
}
