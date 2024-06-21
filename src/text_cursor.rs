use common::repositories::cursor_repository::CursorRepository;
use common::repositories::document_repository::DocumentRepository;
use common::repositories::paragraph_group_repository::ParagraphGroupRepository;
use common::repositories::paragraph_repository::ParagraphRepository;
use cursor_feature::dtos::{MovePositionDTO, SetPositionDTO};
use cursor_feature::MovePositionError;

use crate::TextDocument;

pub struct TextCursor<'a> {
    id: usize,
    cursor_repository: &'a CursorRepository,
    document_repository: &'a DocumentRepository,
    paragraph_repository: &'a ParagraphRepository,
    paragraph_group_repository: &'a ParagraphGroupRepository,
}

impl TextCursor<'_> {
    pub fn new(text_document: &TextDocument) -> TextCursor {
        let cursor_repository = text_document.get_cursor_repository();
        let document_repository = text_document.get_document_repository();
        let paragraph_repository = text_document.get_paragraph_repository();
        let paragraph_group_repository = text_document.get_paragraph_group_repository();

        let id = cursor_feature::create_cursor(cursor_repository);

        TextCursor {
            id,
            cursor_repository,
            document_repository,
            paragraph_repository,
            paragraph_group_repository,
        }
    }

    pub fn get_position(&self) -> usize {
        cursor_feature::get_position(self.cursor_repository, self.id)
    }

    pub fn set_position(&mut self, dto: SetPositionDTO) {
        cursor_feature::set_position(
            self.cursor_repository,
            self.paragraph_group_repository,
            self.id,
            dto,
        );
    }

    pub fn move_position(&mut self, dto: MovePositionDTO) -> Result<(), MovePositionError> {
        cursor_feature::move_position(
            self.cursor_repository,
            self.document_repository,
            self.paragraph_repository,
            self.paragraph_group_repository,
            self.id,
            dto,
        )
    }
}

impl Drop for TextCursor<'_> {
    fn drop(&mut self) {
        cursor_feature::delete_cursor(self.cursor_repository, self.id);
    }
}
