use contracts::persistence::CursorRepositoryTrait;
use entities::cursor::Cursor;

pub struct CreateCursorUseCase<'a> {
    cursor_repository: &'a dyn CursorRepositoryTrait,
}

impl<'a> CreateCursorUseCase<'a> {
    pub fn new(cursor_repository: &'a dyn CursorRepositoryTrait) -> CreateCursorUseCase {
        CreateCursorUseCase { cursor_repository }
    }

    pub fn execute(&self) -> usize {
        self.cursor_repository.create(Cursor::new())
    }
}
