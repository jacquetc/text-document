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
        let _ = text_document.set_plain_text("");
        text_document
    }

    pub(crate) fn cursor_repository(&self) -> &CursorRepository {
        &self.cursor_repository
    }

    pub(crate) fn set_document_repository(&mut self, document_repository: DocumentRepository) {
        self.document_repository = document_repository;
    }

    pub(crate) fn document_repository(&self) -> &DocumentRepository {
        &self.document_repository
    }

    pub(crate) fn set_paragraph_repository(&mut self, paragraph_repository: ParagraphRepository) {
        self.paragraph_repository = paragraph_repository;
    }

    pub(crate) fn paragraph_repository(&self) -> &ParagraphRepository {
        &self.paragraph_repository
    }

    pub(crate) fn set_paragraph_group_repository(
        &mut self,
        paragraph_group_repository: ParagraphGroupRepository,
    ) {
        self.paragraph_group_repository = paragraph_group_repository;
    }

    pub(crate) fn paragraph_group_repository(&self) -> &ParagraphGroupRepository {
        &self.paragraph_group_repository
    }

    pub fn get_plain_text(&self) -> String {
        conversion_feature::get_plain_text(&self.document_repository, &self.paragraph_repository)
            .unwrap()
    }

    pub fn set_plain_text<T: AsRef<str>>(
        &mut self,
        text: T,
    ) -> Result<(), conversion_feature::SetPlainTextError> {
        conversion_feature::set_plain_text(
            &self.cursor_repository,
            &mut self.document_repository,
            &mut self.paragraph_repository,
            &mut self.paragraph_group_repository,
            text.as_ref(),
        )
    }

    pub fn get_markdown_text(&self) -> String {
        conversion_feature::get_markdown(&self.document_repository, &self.paragraph_repository)
            .unwrap()
    }

    pub fn set_markdown<T: AsRef<str>>(
        &mut self,
        text: T,
    ) -> Result<(), conversion_feature::SetMarkdownError> {
        conversion_feature::set_markdown(
            &self.cursor_repository,
            &mut self.document_repository,
            &mut self.paragraph_repository,
            &mut self.paragraph_group_repository,
            text.as_ref(),
        )
    }
}
