mod use_cases;

use crate::use_cases::export_to_plain_text_uc::ExportToPlainTextUseCase;
use crate::use_cases::import_from_plain_text_uc::ImportFromPlainTextUseCase;
use common::contracts::repositories::DocumentRepositoryTrait;


pub fn get_plain_text(document_repository: &dyn DocumentRepositoryTrait) -> String {
    ExportToPlainTextUseCase::new(document_repository).execute()
}

pub fn set_plain_text(document_repository: &mut dyn DocumentRepositoryTrait, text: &str) {
    let _ = ImportFromPlainTextUseCase::new(document_repository).execute(text);
}

pub fn get_markdown(document_repository: &dyn DocumentRepositoryTrait) -> String {
    ExportToPlainTextUseCase::new(document_repository).execute()
}

pub fn set_markdown(document_repository: &mut dyn DocumentRepositoryTrait, markdown: &str) {
    let _ = ImportFromPlainTextUseCase::new(document_repository).execute(markdown);
}