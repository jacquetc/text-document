use common::repositories::cursor_repository::CursorRepository;
use common::repositories::document_repository::DocumentRepository;

use crate::TextDocument;

pub struct TextCursor<'a> {
    id: usize,
    cursor_repository: &'a CursorRepository,
    document_repository: &'a DocumentRepository,
}

impl TextCursor<'_> {
    pub fn new(text_document: &TextDocument) -> TextCursor {
        let cursor_repository = text_document.get_cursor_repository();
        let document_repository = text_document.get_document_repository();

        let id = cursor_feature::create_cursor(cursor_repository);

        TextCursor {
            id,
            cursor_repository,
            document_repository,
        }
    }
}

impl Drop for TextCursor<'_> {
    fn drop(&mut self) {
        cursor_feature::delete_cursor(self.cursor_repository, self.id);
    }
}
