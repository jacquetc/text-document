use common::contracts::repositories::DocumentRepositoryTrait;
use common::contracts::repositories::ParagraphRepositoryTrait;
use std::path::Path;
use thiserror::Error;

use crate::use_cases::get_plain_text_uc::{GetPlainTextError, GetPlainTextUseCase};

#[derive(Error, Debug)]
pub enum ExportToPlainTextFileError {
    #[error("Plain text export error: {source}")]
    PlainText {
        #[from]
        source: GetPlainTextError,
    },
    #[error("IO error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },
}

pub struct ExportToPlainTextFileUseCase<'a> {
    document_repository: &'a dyn DocumentRepositoryTrait,
    paragraph_repository: &'a dyn ParagraphRepositoryTrait,
}

impl<'a> ExportToPlainTextFileUseCase<'a> {
    pub fn new(
        document_repository: &'a dyn DocumentRepositoryTrait,
        paragraph_repository: &'a dyn ParagraphRepositoryTrait,
    ) -> ExportToPlainTextFileUseCase<'a> {
        ExportToPlainTextFileUseCase {
            document_repository,
            paragraph_repository,
        }
    }

    pub fn execute(&mut self, path: &Path) -> Result<(), ExportToPlainTextFileError> {
        let text = GetPlainTextUseCase::new(self.document_repository, self.paragraph_repository)
            .execute()?;

        std::fs::write(path, text)?;

        Ok(())
    }
}
