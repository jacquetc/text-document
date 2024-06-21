use common::repositories::cursor_repository::CursorRepository;
use common::repositories::document_repository::DocumentRepository;
use common::repositories::paragraph_group_repository::ParagraphGroupRepository;
use common::repositories::paragraph_repository::ParagraphRepository;

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
        let mut text_document = TextDocument {
            cursor_repository: CursorRepository::new(),
            document_repository: DocumentRepository::new(),
            paragraph_repository: ParagraphRepository::new(),
            paragraph_group_repository: ParagraphGroupRepository::new(),
        };

        // Initialize the document with an empty paragraph
        text_document.set_plain_text("");
        text_document
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
            &self.cursor_repository,
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
            &self.cursor_repository,
            &mut self.document_repository,
            &mut self.paragraph_repository,
            &mut self.paragraph_group_repository,
            markdown.as_ref(),
        );
    }
}
