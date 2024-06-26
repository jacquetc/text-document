use crate::use_cases::set_plain_text_uc::{SetPlainTextError, SetPlainTextUseCase};
use common::contracts::repositories::CursorRepositoryTrait;
use common::contracts::repositories::DocumentRepositoryTrait;
use common::contracts::repositories::ParagraphGroupRepositoryTrait;
use common::contracts::repositories::ParagraphRepositoryTrait;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ImportFromPlainTextFileError {
    #[error("Plain text import error: {source}")]
    PlainText {
        #[from]
        source: SetPlainTextError,
    },
    #[error("IO error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },
}

pub struct ImportFromPlainTextFileUseCase<'a> {
    cursor_repository: &'a dyn CursorRepositoryTrait,
    document_repository: &'a mut dyn DocumentRepositoryTrait,
    paragraph_repository: &'a mut dyn ParagraphRepositoryTrait,
    paragraph_group_repository: &'a mut dyn ParagraphGroupRepositoryTrait,
}

impl<'a> ImportFromPlainTextFileUseCase<'a> {
    pub fn new(
        cursor_repository: &'a dyn CursorRepositoryTrait,
        document_repository: &'a mut dyn DocumentRepositoryTrait,
        paragraph_repository: &'a mut dyn ParagraphRepositoryTrait,
        paragraph_group_repository: &'a mut dyn ParagraphGroupRepositoryTrait,
    ) -> ImportFromPlainTextFileUseCase<'a> {
        ImportFromPlainTextFileUseCase {
            cursor_repository,
            document_repository,
            paragraph_repository,
            paragraph_group_repository,
        }
    }

    pub fn execute(&mut self, path: &Path) -> Result<(), ImportFromPlainTextFileError> {
        let text = std::fs::read_to_string(path)?;

        SetPlainTextUseCase::new(
            self.cursor_repository,
            self.document_repository,
            self.paragraph_repository,
            self.paragraph_group_repository,
        )
        .execute(text.as_ref())?;

        Ok(())
    }
}
