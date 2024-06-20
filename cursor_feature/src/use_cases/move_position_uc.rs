use crate::dtos::MovePositionDTO;
use common::contracts::repositories::{CursorRepositoryTrait, DocumentRepositoryTrait};
use common::entities::cursor::Cursor;

pub struct MovePositionUseCase<'a> {
    cursor_repository: &'a mut dyn CursorRepositoryTrait,
    document_repository: &'a dyn DocumentRepositoryTrait,
}

impl<'a> MovePositionUseCase<'a> {
    pub fn new(
        cursor_repository: &'a mut dyn CursorRepositoryTrait,
        document_repository: &'a dyn DocumentRepositoryTrait,
    ) -> MovePositionUseCase<'a> {
        MovePositionUseCase {
            cursor_repository,
            document_repository,
        }
    }

    pub fn execute(&mut self, cursor_id: usize, dto: MovePositionDTO) {
        let mut cursor = self.cursor_repository.get(cursor_id);
    }
}
