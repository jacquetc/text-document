mod use_cases;
pub mod dtos;

use crate::use_cases::create_cursor_uc::CreateCursorUseCase;
use crate::use_cases::delete_cursor_uc::DeleteCursorUseCase;
use crate::use_cases::move_position_uc::MovePositionUseCase;
use common::contracts::repositories::CursorRepositoryTrait;
use common::contracts::repositories::DocumentRepositoryTrait;

pub fn create_cursor(cursor_repository: &dyn CursorRepositoryTrait) -> usize {
    CreateCursorUseCase::new(cursor_repository).execute()
}

pub fn delete_cursor(cursor_repository: &dyn CursorRepositoryTrait, cursor_id: usize) {
    DeleteCursorUseCase::new(cursor_repository).execute(cursor_id);
}

pub fn move_position(cursor_repository: &mut dyn CursorRepositoryTrait, document_repository: &dyn DocumentRepositoryTrait, cursor_id: usize, dto: dtos::MovePositionDTO) {
    MovePositionUseCase::new(cursor_repository, document_repository).execute(cursor_id, dto);
}