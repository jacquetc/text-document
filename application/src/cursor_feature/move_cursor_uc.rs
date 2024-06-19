use contracts::dtos::cursor_dtos::MoveCursorDTO;
use contracts::persistence::CursorRepositoryTrait;
use entities::cursor::Cursor;

pub struct MoveCursorUseCase<'a> {
    cursor_repository: &'a mut dyn CursorRepositoryTrait,
}

impl<'a> MoveCursorUseCase<'a> {
    pub fn new(cursor_repository: &'a mut dyn CursorRepositoryTrait) -> MoveCursorUseCase {
        MoveCursorUseCase { cursor_repository }
    }

    pub fn execute(&mut self, dto: MoveCursorDTO) {
        let mut cursor = self.cursor_repository.get(dto.cursor_id);
    }
}
