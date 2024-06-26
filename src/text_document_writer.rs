use crate::TextDocument;

pub struct TextDocumentWriter<'a> {
    text_document: &'a TextDocument,
}

impl TextDocumentWriter<'_> {
    pub fn new(text_document: &TextDocument) -> TextDocumentWriter {
        TextDocumentWriter { text_document }
    }

    pub fn write_plain_text_file<T: AsRef<std::path::Path>>(
        &self,
        path: T,
    ) -> Result<(), conversion_feature::ExportToPlainTextFileError> {
        let document_repository = self.text_document.document_repository();
        let paragraph_repository = self.text_document.paragraph_repository();

        conversion_feature::export_plain_text_file(
            document_repository,
            paragraph_repository,
            path.as_ref(),
        )
    }
}
