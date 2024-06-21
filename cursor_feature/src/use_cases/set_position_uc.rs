use common::contracts::repositories::CursorRepositoryTrait;
use common::contracts::repositories::ParagraphGroupRepositoryTrait;

use crate::dtos::MoveMode;
use crate::dtos::SetPositionDTO;

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

    pub fn execute(&self, cursor_id: usize, dto: SetPositionDTO) {
        let position_max = self
            .paragraph_group_repository
            .get_all()
            .iter()
            .map(|pg| pg.char_count + pg.paragraph_count)
            .sum::<usize>()
            - 1;

        let new_position = if dto.position > position_max {
            position_max
        } else {
            dto.position
        };

        let mut cursor = self.cursor_repository.get(cursor_id).unwrap();

        match dto.mode {
            MoveMode::MoveAnchorWithCursor => {
                cursor.position = new_position;
                cursor.anchor_position = None;
            }
            MoveMode::MoveCursorOnly => match cursor.anchor_position {
                None => {
                    cursor.anchor_position = Some(cursor.position);
                    cursor.position = new_position;
                }
                Some(_anchor_position) => {
                    cursor.position = new_position;
                }
            },
        }
        let _ = self.cursor_repository.update(cursor);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use common::contracts::repositories::RepositoryTrait;
    use common::entities::cursor::Cursor;
    use common::entities::paragraph_group::ParagraphGroup;
    use common::repositories::cursor_repository::CursorRepository;
    use common::repositories::paragraph_group_repository::ParagraphGroupRepository;

    #[test]
    fn test_set_position() {
        let cursor_repository = CursorRepository::new();
        let mut paragraph_group_repository = ParagraphGroupRepository::new();

        let cursor = Cursor::new();
        let cursor_id = cursor_repository.create(cursor);

        let pg1 = ParagraphGroup {
            id: 1,
            paragraph_count: 2,
            char_count_per_paragraph: HashMap::new(),
            char_count: 100,
            word_count: 20,
        };
        let pg2 = ParagraphGroup {
            id: 2,
            paragraph_count: 3,
            char_count_per_paragraph: HashMap::new(),
            char_count: 100,
            word_count: 30,
        };

        let _ = paragraph_group_repository.create(pg1);
        let _ = paragraph_group_repository.create(pg2);

        let dto = SetPositionDTO {
            position: 0,
            mode: MoveMode::MoveAnchorWithCursor,
        };

        let use_case: SetPositionUseCase =
            SetPositionUseCase::new(&cursor_repository, &paragraph_group_repository);
        use_case.execute(cursor_id, dto);

        let cursor = cursor_repository.get(cursor_id).unwrap();
        assert_eq!(cursor.position, 0);
        assert_eq!(cursor.anchor_position, None);
    }

    #[test]
    fn test_set_position_with_anchor() {
        let cursor_repository = CursorRepository::new();
        let mut paragraph_group_repository = ParagraphGroupRepository::new();

        let cursor = Cursor::new();
        let cursor_id = cursor_repository.create(cursor);

        let pg1 = ParagraphGroup {
            id: 1,
            paragraph_count: 2,
            char_count_per_paragraph: HashMap::new(),
            char_count: 100,
            word_count: 20,
        };
        let pg2 = ParagraphGroup {
            id: 2,
            paragraph_count: 3,
            char_count_per_paragraph: HashMap::new(),
            char_count: 100,
            word_count: 30,
        };

        let _ = paragraph_group_repository.create(pg1);
        let _ = paragraph_group_repository.create(pg2);

        let dto = SetPositionDTO {
            position: 0,
            mode: MoveMode::MoveCursorOnly,
        };

        let use_case: SetPositionUseCase =
            SetPositionUseCase::new(&cursor_repository, &paragraph_group_repository);
        use_case.execute(cursor_id, dto);

        let cursor = cursor_repository.get(cursor_id).unwrap();
        assert_eq!(cursor.position, 0);
        assert_eq!(cursor.anchor_position, Some(0));
    }

    #[test]
    fn test_set_position_with_anchor_and_move_cursor() {
        let cursor_repository = CursorRepository::new();
        let mut paragraph_group_repository = ParagraphGroupRepository::new();

        let cursor = Cursor::new();
        let cursor_id = cursor_repository.create(cursor);

        let pg1 = ParagraphGroup {
            id: 1,
            paragraph_count: 2,
            char_count_per_paragraph: HashMap::new(),
            char_count: 100,
            word_count: 20,
        };
        let pg2 = ParagraphGroup {
            id: 2,
            paragraph_count: 3,
            char_count_per_paragraph: HashMap::new(),
            char_count: 100,
            word_count: 30,
        };

        let _ = paragraph_group_repository.create(pg1);
        let _ = paragraph_group_repository.create(pg2);

        let dto = SetPositionDTO {
            position: 0,
            mode: MoveMode::MoveCursorOnly,
        };

        let use_case: SetPositionUseCase =
            SetPositionUseCase::new(&cursor_repository, &paragraph_group_repository);
        use_case.execute(cursor_id, dto);

        let cursor = cursor_repository.get(cursor_id).unwrap();
        assert_eq!(cursor.position, 0);
        assert_eq!(cursor.anchor_position, Some(0));

        let dto = SetPositionDTO {
            position: 1,
            mode: MoveMode::MoveCursorOnly,
        };

        use_case.execute(cursor_id, dto);

        let cursor = cursor_repository.get(cursor_id).unwrap();
        assert_eq!(cursor.position, 1);
        assert_eq!(cursor.anchor_position, Some(0));
    }
}
