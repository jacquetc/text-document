pub mod dtos;
mod use_cases;

use crate::use_cases::create_cursor_uc::CreateCursorUseCase;
use crate::use_cases::delete_cursor_uc::DeleteCursorUseCase;
use crate::use_cases::move_position_uc::MovePositionUseCase;
use common::contracts::repositories::CursorRepositoryTrait;
use common::contracts::repositories::DocumentRepositoryTrait;
use common::contracts::repositories::ParagraphGroupRepositoryTrait;
use common::contracts::repositories::ParagraphRepositoryTrait;
use dtos::SetPositionDTO;
use use_cases::get_position_uc::GetPositionUseCase;
pub use use_cases::move_position_uc::MovePositionError;
use use_cases::set_position_uc::SetPositionUseCase;

pub fn create_cursor(cursor_repository: &dyn CursorRepositoryTrait) -> usize {
    CreateCursorUseCase::new(cursor_repository).execute()
}

pub fn delete_cursor(cursor_repository: &dyn CursorRepositoryTrait, cursor_id: usize) {
    DeleteCursorUseCase::new(cursor_repository).execute(cursor_id);
}

pub fn move_position(
    cursor_repository: &dyn CursorRepositoryTrait,
    document_repository: &dyn DocumentRepositoryTrait,
    paragraph_repository: &dyn ParagraphRepositoryTrait,
    paragraph_group_repository: &dyn ParagraphGroupRepositoryTrait,
    cursor_id: usize,
    dto: dtos::MovePositionDTO,
) -> Result<(), MovePositionError> {
    MovePositionUseCase::new(
        cursor_repository,
        document_repository,
        paragraph_repository,
        paragraph_group_repository,
    )
    .execute(cursor_id, dto)
}

pub fn get_position(cursor_repository: &dyn CursorRepositoryTrait, cursor_id: usize) -> usize {
    GetPositionUseCase::new(cursor_repository).execute(cursor_id)
}

pub fn set_position(
    cursor_repository: &dyn CursorRepositoryTrait,
    paragraph_group_repository: &dyn ParagraphGroupRepositoryTrait,
    cursor_id: usize,
    dto: SetPositionDTO,
) {
    SetPositionUseCase::new(cursor_repository, paragraph_group_repository).execute(cursor_id, dto);
}
