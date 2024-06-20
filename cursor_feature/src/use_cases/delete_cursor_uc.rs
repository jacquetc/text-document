use common::contracts::repositories::CursorRepositoryTrait;
use common::entities::cursor::Cursor;

pub struct DeleteCursorUseCase<'a> {
    cursor_repository: &'a dyn CursorRepositoryTrait,
}

impl<'a> DeleteCursorUseCase<'a> {
    pub fn new(cursor_repository: &'a dyn CursorRepositoryTrait) -> DeleteCursorUseCase {
        DeleteCursorUseCase { cursor_repository }
    }

    pub fn execute(&self, cursor_id: usize) {
        self.cursor_repository.remove(cursor_id);
    }
}