mod use_cases;

use crate::use_cases::export_to_plain_text_uc::ExportToPlainTextUseCase;
use crate::use_cases::import_from_plain_text_uc::ImportFromPlainTextUseCase;
use common::contracts::repositories::DocumentRepositoryTrait;
use common::contracts::repositories::ParagraphRepositoryTrait;
use common::contracts::repositories::ParagraphGroupRepositoryTrait;



pub fn get_plain_text(
    document_repository: &dyn DocumentRepositoryTrait,
    paragraph_repository: &dyn ParagraphRepositoryTrait,
) -> String {
    ExportToPlainTextUseCase::new(document_repository, paragraph_repository).execute()
}

pub fn set_plain_text(
    document_repository: &mut dyn DocumentRepositoryTrait,
    paragraph_repository: &mut dyn ParagraphRepositoryTrait,
    paragraph_group_repository: &mut dyn ParagraphGroupRepositoryTrait,
    text: &str,
) {
    let _ =
        ImportFromPlainTextUseCase::new(document_repository, paragraph_repository, paragraph_group_repository).execute(text);
}

pub fn get_markdown(
    document_repository: &dyn DocumentRepositoryTrait,
    paragraph_repository: &dyn ParagraphRepositoryTrait,
) -> String {
    ExportToPlainTextUseCase::new(document_repository, paragraph_repository).execute()
}

pub fn set_markdown(
    document_repository: &mut dyn DocumentRepositoryTrait,
    paragraph_repository: &mut dyn ParagraphRepositoryTrait,
    paragraph_group_repository: &mut dyn ParagraphGroupRepositoryTrait,
    markdown: &str,
) {
    let _ = ImportFromPlainTextUseCase::new(document_repository, paragraph_repository, paragraph_group_repository)
        .execute(markdown);
}
