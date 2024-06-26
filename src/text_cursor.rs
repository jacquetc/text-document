use std::result;

use cursor_feature::dtos::{MovePositionDTO, SetPositionDTO};
use cursor_feature::MovePositionError;

use crate::TextDocument;

pub struct TextCursor<'a> {
    id: usize,
    text_document: &'a TextDocument,
}

impl TextCursor<'_> {
    pub fn new(text_document: &TextDocument) -> TextCursor {
        let id = cursor_feature::create_cursor(text_document.cursor_repository());
        TextCursor { id, text_document }
    }

    pub fn anchor(&self) -> usize {
        shared_impl::anchor(self.text_document, self.id)
    }

    pub fn at_paragraph_end(&self) -> bool {
        shared_impl::at_paragraph_end(self.text_document, self.id)
    }
    
    pub fn at_paragraph_start(&self) -> bool {
        shared_impl::at_paragraph_start(self.text_document, self.id)
    }

    pub fn at_end(&self) -> bool {
        shared_impl::at_end(self.text_document, self.id)
    }
    
    pub fn at_start(&self) -> bool {
        shared_impl::at_start(self.text_document, self.id)
    }

    pub fn has_selection(&self) -> bool {
        shared_impl::has_selection(self.text_document, self.id)
    }

    pub fn position(&self) -> usize {
        shared_impl::position(self.text_document, self.id)
    }

    pub fn set_position(&self, dto: SetPositionDTO) {
        shared_impl::set_position(self.text_document, self.id, dto)
    }

    pub fn move_position(&self, dto: MovePositionDTO) -> Result<(), MovePositionError> {
        shared_impl::move_position(self.text_document, self.id, dto)
    }

    pub fn selected_text(&self) -> Option<String> {
        shared_impl::selected_text(self.text_document, self.id)
    }

}

impl Drop for TextCursor<'_> {
    fn drop(&mut self) {
        cursor_feature::delete_cursor(self.text_document.cursor_repository(), self.id);
    }
}

pub struct TextCursorMut<'a> {
    id: usize,
    text_document: &'a mut TextDocument,
}

impl TextCursorMut<'_> {
    pub fn new(text_document: & mut TextDocument) -> TextCursorMut {
        let id = cursor_feature::create_cursor(text_document.cursor_repository());
        TextCursorMut { id, text_document }
    }

    pub fn anchor(&self) -> usize {
        shared_impl::anchor(self.text_document, self.id)
    }

    pub fn insert_text() -> Result<(), String>{
        unimplemented!()
    }

}

mod shared_impl {
    use crate::TextDocument;
    use common::contracts::repositories::CursorRepositoryTrait;
    use cursor_feature::dtos::{MovePositionDTO, SetPositionDTO};
    use cursor_feature::MovePositionError;

    pub(super) fn anchor(text_document: &TextDocument, cursor_id: usize) -> usize {
        let cursor_repository = text_document.cursor_repository();
        let cursor = cursor_repository.get(cursor_id).unwrap();
        match cursor.anchor_position {
            Some(anchor_position) => anchor_position,
            None => cursor.position,
        }
    }
    
    pub(super)  fn at_paragraph_end(text_document: &TextDocument, cursor_id: usize) -> bool {
        unimplemented!()
    }
    
    pub(super)  fn at_paragraph_start(text_document: &TextDocument, cursor_id: usize) -> bool {
        unimplemented!()
    }

    pub(super)  fn at_end(text_document: &TextDocument, cursor_id: usize) -> bool {
        unimplemented!()
    }
    
    pub(super)  fn at_start(text_document: &TextDocument, cursor_id: usize) -> bool {
        unimplemented!()
    }

    pub(super)  fn has_selection(text_document: &TextDocument, cursor_id: usize) -> bool {
        unimplemented!()
    }

    
    pub fn position(text_document: &TextDocument, cursor_id: usize) -> usize {
        cursor_feature::get_position(text_document.cursor_repository(), cursor_id)
    }

    pub fn set_position(text_document: &TextDocument, cursor_id: usize, dto: SetPositionDTO) {
        cursor_feature::set_position(
            text_document.cursor_repository(),
            text_document.paragraph_group_repository(),
            cursor_id,
            dto,
        );
    }

    pub fn move_position(text_document: &TextDocument, cursor_id: usize, dto: MovePositionDTO) -> Result<(), MovePositionError> {
        let document_repository = text_document.document_repository().clone();
        let paragraph_repository = text_document.paragraph_repository().clone();
        let paragraph_group_repository = text_document.paragraph_group_repository().clone();

        cursor_feature::move_position(
            text_document.cursor_repository(),
            &document_repository,
            &paragraph_repository,
            &paragraph_group_repository,
            cursor_id,
            dto,
        )
    }

    pub(super)  fn selected_text(text_document: &TextDocument, cursor_id: usize) -> Option<String> {
        unimplemented!()
    }

}