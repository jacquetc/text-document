use common::contracts::repositories::CursorRepositoryTrait;
use common::contracts::repositories::DocumentRepositoryTrait;
use common::contracts::repositories::ParagraphGroupRepositoryTrait;
use common::contracts::repositories::ParagraphRepositoryTrait;

use crate::dtos::TextType;
use crate::dtos::InsertTextDTO;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum InsertTextError {
    #[error("ConversionError")]
    ConversionError,
}

pub struct InsertTextUseCase<'a> {
    cursor_repository: &'a dyn CursorRepositoryTrait,
    document_repository: &'a mut dyn DocumentRepositoryTrait,
    paragraph_repository: &'a mut dyn ParagraphRepositoryTrait,
    paragraph_group_repository: &'a mut dyn ParagraphGroupRepositoryTrait,
}

impl<'a> InsertTextUseCase<'a> {
    pub fn new(
        cursor_repository: &'a dyn CursorRepositoryTrait,
        document_repository: &'a mut dyn DocumentRepositoryTrait,
        paragraph_repository: &'a mut dyn ParagraphRepositoryTrait,
        paragraph_group_repository: &'a mut dyn ParagraphGroupRepositoryTrait,
    ) -> InsertTextUseCase<'a> {
        InsertTextUseCase {
            cursor_repository,
            document_repository,
            paragraph_repository,
            paragraph_group_repository
        }
    }

    pub fn execute(&self, cursor_id: usize, dto: InsertTextDTO)  -> Result<(), InsertTextError> {
        match dto.text_type {
            TextType::PlainText => {
                unimplemented!()
                //dto.text.lines()
            },
            TextType::Markdown => todo!(),
            TextType::Html => todo!(),
        }
    }
}