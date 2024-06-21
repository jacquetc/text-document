use common::contracts::repositories::CursorRepositoryTrait;

pub struct GetPositionUseCase<'a> {
    cursor_repository: &'a dyn CursorRepositoryTrait,
}

impl<'a> GetPositionUseCase<'a> {
    pub fn new(cursor_repository: &'a dyn CursorRepositoryTrait) -> GetPositionUseCase {
        GetPositionUseCase { cursor_repository }
    }

    pub fn execute(&self, cursor_id: usize) -> usize {
        self.cursor_repository.get(cursor_id).unwrap().position
    }
}
