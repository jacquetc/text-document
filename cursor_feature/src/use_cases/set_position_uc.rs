use common::contracts::repositories::CursorRepositoryTrait;
use common::contracts::repositories::ParagraphGroupRepositoryTrait;
use common::entities::document::Node;
use common::repositories::document_repository;

pub struct SetPositionUseCase<'a> {
    cursor_repository: &'a dyn CursorRepositoryTrait,
    paragraph_group_repository: &'a dyn ParagraphGroupRepositoryTrait,
}

impl<'a> SetPositionUseCase<'a> {
    pub fn new(
        cursor_repository: &'a dyn CursorRepositoryTrait,
        paragraph_group_repository: &'a dyn ParagraphGroupRepositoryTrait,
    ) -> SetPositionUseCase<'a> {
        SetPositionUseCase {
            cursor_repository,
            paragraph_group_repository,
        }
    }

    pub fn execute(&self, cursor_id: usize, position: usize) {
        let position_max = self
            .paragraph_group_repository
            .get_all()
            .iter()
            .map(|pg| pg.char_count + pg.paragraph_count)
            .sum::<usize>()
            - 1;

        let position = if position > position_max {
            position_max
        } else {
            position
        };

        let mut cursor = self.cursor_repository.get(cursor_id).unwrap();
        cursor.position = position;
        let _ = self.cursor_repository.update(cursor);
    }
}
