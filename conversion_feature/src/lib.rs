mod use_cases;

pub use crate::use_cases::export_to_plain_text_file_uc::ExportToPlainTextFileError;
use crate::use_cases::export_to_plain_text_file_uc::ExportToPlainTextFileUseCase;
pub use crate::use_cases::get_markdown_uc::GetMarkdownError;
use crate::use_cases::get_markdown_uc::GetMarkdownUseCase;
pub use crate::use_cases::get_plain_text_uc::GetPlainTextError;
use crate::use_cases::get_plain_text_uc::GetPlainTextUseCase;
pub use crate::use_cases::import_from_plain_text_file_uc::ImportFromPlainTextFileError;
use crate::use_cases::import_from_plain_text_file_uc::ImportFromPlainTextFileUseCase;
pub use crate::use_cases::set_markdown_uc::SetMarkdownError;
use crate::use_cases::set_markdown_uc::SetMarkdownUseCase;
pub use crate::use_cases::set_plain_text_uc::SetPlainTextError;
use crate::use_cases::set_plain_text_uc::SetPlainTextUseCase;

use common::contracts::repositories::CursorRepositoryTrait;
use common::contracts::repositories::DocumentRepositoryTrait;
use common::contracts::repositories::ParagraphGroupRepositoryTrait;
use common::contracts::repositories::ParagraphRepositoryTrait;

pub fn get_plain_text(
    document_repository: &dyn DocumentRepositoryTrait,
    paragraph_repository: &dyn ParagraphRepositoryTrait,
) -> Result<String, GetPlainTextError> {
    GetPlainTextUseCase::new(document_repository, paragraph_repository).execute()
}

pub fn export_plain_text_file(
    document_repository: &dyn DocumentRepositoryTrait,
    paragraph_repository: &dyn ParagraphRepositoryTrait,
    path: &std::path::Path,
) -> Result<(), ExportToPlainTextFileError> {
    ExportToPlainTextFileUseCase::new(document_repository, paragraph_repository).execute(path)
}

pub fn set_plain_text(
    cursor_repository: &dyn CursorRepositoryTrait,
    document_repository: &mut dyn DocumentRepositoryTrait,
    paragraph_repository: &mut dyn ParagraphRepositoryTrait,
    paragraph_group_repository: &mut dyn ParagraphGroupRepositoryTrait,
    text: &str,
) -> Result<(), SetPlainTextError> {
    SetPlainTextUseCase::new(
        cursor_repository,
        document_repository,
        paragraph_repository,
        paragraph_group_repository,
    )
    .execute(text)
}

pub fn import_plain_text_file(
    cursor_repository: &dyn CursorRepositoryTrait,
    document_repository: &mut dyn DocumentRepositoryTrait,
    paragraph_repository: &mut dyn ParagraphRepositoryTrait,
    paragraph_group_repository: &mut dyn ParagraphGroupRepositoryTrait,
    path: &std::path::Path,
) -> Result<(), ImportFromPlainTextFileError> {
    ImportFromPlainTextFileUseCase::new(
        cursor_repository,
        document_repository,
        paragraph_repository,
        paragraph_group_repository,
    )
    .execute(path)
}

pub fn get_markdown(
    document_repository: &dyn DocumentRepositoryTrait,
    paragraph_repository: &dyn ParagraphRepositoryTrait,
) -> Result<String, GetMarkdownError> {
    GetMarkdownUseCase::new(document_repository, paragraph_repository).execute()
}

pub fn set_markdown(
    cursor_repository: &dyn CursorRepositoryTrait,
    document_repository: &mut dyn DocumentRepositoryTrait,
    paragraph_repository: &mut dyn ParagraphRepositoryTrait,
    paragraph_group_repository: &mut dyn ParagraphGroupRepositoryTrait,
    markdown: &str,
) -> Result<(), SetMarkdownError> {
    SetMarkdownUseCase::new(
        cursor_repository,
        document_repository,
        paragraph_repository,
        paragraph_group_repository,
    )
    .execute(markdown)
}
