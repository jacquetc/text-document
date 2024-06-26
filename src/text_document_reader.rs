use crate::TextDocument;

pub struct TextDocumentReader<'a> {
    text_document: &'a mut TextDocument,
}

impl TextDocumentReader<'_> {
    pub fn new(text_document: &mut TextDocument) -> TextDocumentReader {
        TextDocumentReader { text_document }
    }

    pub fn read_plain_text_file<T: AsRef<std::path::Path>>(
        &mut self,
        path: T,
    ) -> Result<(), conversion_feature::ImportFromPlainTextFileError> {
        let cursor_repository = self.text_document.cursor_repository();
        let mut document_repository = self.text_document.document_repository().clone();
        let mut paragraph_repository = self.text_document.paragraph_repository().clone();
        let mut paragraph_group_repository =
            self.text_document.paragraph_group_repository().clone();

        conversion_feature::import_plain_text_file(
            cursor_repository,
            &mut document_repository,
            &mut paragraph_repository,
            &mut paragraph_group_repository,
            path.as_ref(),
        )?;

        // This is a workaround to avoid borrowing issues
        self.text_document
            .set_document_repository(document_repository);
        self.text_document
            .set_paragraph_repository(paragraph_repository);
        self.text_document
            .set_paragraph_group_repository(paragraph_group_repository);

        Ok(())
    }
}
