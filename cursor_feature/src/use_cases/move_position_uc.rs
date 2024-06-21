use crate::dtos::{MoveMode, MoveOperation, MovePositionDTO};
use common::contracts::repositories::{
    CursorRepositoryTrait, DocumentRepositoryTrait, ParagraphGroupRepositoryTrait,
    ParagraphRepositoryTrait, RepositoryError,
};
use common::entities::cursor::Cursor;
use common::entities::paragraph;
use common::paragraph_operations;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MovePositionError {
    #[error("Cursor is at the start of the document")]
    StartOfDocument,
    #[error("Cursor is at the end of the document")]
    EndOfDocument,
    #[error("No previous paragraph found")]
    NoNextParagraph,
    #[error("No next paragraph found")]
    NoPreviousParagraph,
    #[error("No previous word found")]
    NoPreviousWord,
    #[error("No next word found")]
    NoNextWord,
    #[error("Internal error")]
    Internal {
        #[from]
        source: RepositoryError,
    },
}

pub struct MovePositionUseCase<'a> {
    cursor_repository: &'a dyn CursorRepositoryTrait,
    document_repository: &'a dyn DocumentRepositoryTrait,
    paragraph_repository: &'a dyn ParagraphRepositoryTrait,
    paragraph_group_repository: &'a dyn ParagraphGroupRepositoryTrait,
}

impl<'a> MovePositionUseCase<'a> {
    pub fn new(
        cursor_repository: &'a dyn CursorRepositoryTrait,
        document_repository: &'a dyn DocumentRepositoryTrait,
        paragraph_repository: &'a dyn ParagraphRepositoryTrait,
        paragraph_group_repository: &'a dyn ParagraphGroupRepositoryTrait,
    ) -> MovePositionUseCase<'a> {
        MovePositionUseCase {
            cursor_repository,
            document_repository,
            paragraph_repository,
            paragraph_group_repository,
        }
    }

    pub fn execute(&self, cursor_id: usize, dto: MovePositionDTO) -> Result<(), MovePositionError> {
        let position_max = self
            .paragraph_group_repository
            .get_all()
            .iter()
            .map(|pg| pg.char_count + pg.paragraph_count)
            .sum::<usize>()
            - 1;

        let mut cursor = self.cursor_repository.get(cursor_id).unwrap();

        match dto.operation {
            MoveOperation::NoMove => Ok(()),
            MoveOperation::Start => match dto.mode {
                MoveMode::MoveAnchorWithCursor => {
                    cursor.position = 0;
                    cursor.anchor_position = None;
                    self.cursor_repository.update(cursor)?;
                    Ok(())
                }
                MoveMode::MoveCursorOnly => {
                    if cursor.anchor_position.is_none() {
                        cursor.anchor_position = Some(cursor.position);
                    }
                    cursor.position = 0;
                    self.cursor_repository.update(cursor)?;
                    Ok(())
                }
            },
            MoveOperation::StartOfParagraph => {
                let old_cursor_position = cursor.position;
                cursor.position = paragraph_operations::get_paragraph_position_by_cursor_position(
                    old_cursor_position,
                    self.document_repository,
                    self.paragraph_group_repository,
                );

                match dto.mode {
                    MoveMode::MoveAnchorWithCursor => {
                        cursor.anchor_position = None;
                        self.cursor_repository.update(cursor)?;
                        Ok(())
                    }
                    MoveMode::MoveCursorOnly => {
                        if cursor.anchor_position.is_none() {
                            cursor.anchor_position = Some(old_cursor_position);
                        }
                        self.cursor_repository.update(cursor)?;
                        Ok(())
                    }
                }
            }
            MoveOperation::StartOfWord => {
                let cursor_current_position = cursor.position;

                let paragraph_id = paragraph_operations::get_paragraph_id_by_position(
                    cursor.position,
                    self.document_repository,
                    self.paragraph_group_repository,
                )
                .ok_or(MovePositionError::NoPreviousWord)?;

                let paragraph_position = paragraph_operations::paragraph_position(
                    paragraph_id,
                    self.document_repository,
                    self.paragraph_group_repository,
                );

                let paragraph = self.paragraph_repository.get(paragraph_id).unwrap();

                let paragraph_plain_text = paragraph.get_text();
                let cursor_relative_position = cursor_current_position - paragraph_position;

                let mut word_start_position = 0;
                let mut _word_end_position = 0;
                let mut reached_cursor_position = false;

                for (i, c) in paragraph_plain_text.chars().enumerate() {
                    if c.is_whitespace() {
                        if !reached_cursor_position {
                            word_start_position = i + 1;
                        }
                        if reached_cursor_position {
                            _word_end_position = i;
                            break;
                        }
                    }
                    if i == cursor_relative_position {
                        reached_cursor_position = true;
                    }
                    if reached_cursor_position {
                        _word_end_position = i + 1;
                    }
                }

                let new_position = paragraph_position + word_start_position;

                match dto.mode {
                    MoveMode::MoveAnchorWithCursor => {
                        cursor.position = new_position;
                        cursor.anchor_position = None;
                        self.cursor_repository.update(cursor)?;
                        Ok(())
                    }
                    MoveMode::MoveCursorOnly => {
                        if cursor.anchor_position.is_none() {
                            cursor.anchor_position = Some(cursor.position);
                        }
                        cursor.position = new_position;
                        self.cursor_repository.update(cursor)?;
                        Ok(())
                    }
                }
            }
            MoveOperation::PreviousParagraph => {
                for _ in 0..dto.count {
                    let cursor_position = cursor.position;
                    let previous_paragraph_id = paragraph_operations::get_previous_paragraph_id(
                        cursor_position,
                        self.document_repository,
                    )
                    .ok_or(MovePositionError::NoPreviousParagraph)?;

                    let paragraph_position = paragraph_operations::paragraph_position(
                        previous_paragraph_id,
                        self.document_repository,
                        self.paragraph_group_repository,
                    );

                    let new_position = paragraph_position;

                    match dto.mode {
                        MoveMode::MoveAnchorWithCursor => {
                            cursor.position = new_position;
                            cursor.anchor_position = None;
                            self.cursor_repository.update(cursor)?;
                        }
                        MoveMode::MoveCursorOnly => {
                            if cursor.anchor_position.is_none() {
                                cursor.anchor_position = Some(cursor.position);
                            }
                            cursor.position = new_position;
                            self.cursor_repository.update(cursor)?;
                        }
                    }
                }
                Ok(())
            }

            MoveOperation::PreviousCharacter => {
                for _ in 0..dto.count {
                    match dto.mode {
                        MoveMode::MoveAnchorWithCursor => {
                            if cursor.position == 0 {
                                return Err(MovePositionError::StartOfDocument);
                            }
                            cursor.position -= 1;
                            cursor.anchor_position = None;
                            self.cursor_repository.update(cursor)?;
                        }
                        MoveMode::MoveCursorOnly => {
                            if cursor.position == 0 {
                                return Err(MovePositionError::StartOfDocument);
                            }
                            if cursor.anchor_position.is_none() {
                                cursor.anchor_position = Some(cursor.position);
                            }
                            cursor.position -= 1;
                            self.cursor_repository.update(cursor)?;
                        }
                    }
                }
                Ok(())
            }
            MoveOperation::PreviousWord => {
                for _ in 0..dto.count {
                    let cursor_current_position = cursor.position;

                    let paragraph_id = paragraph_operations::get_paragraph_id_by_position(
                        cursor.position,
                        self.document_repository,
                        self.paragraph_group_repository,
                    )
                    .ok_or(MovePositionError::NoPreviousWord)?;

                    let paragraph_position = paragraph_operations::paragraph_position(
                        paragraph_id,
                        self.document_repository,
                        self.paragraph_group_repository,
                    );

                    let paragraph = self.paragraph_repository.get(paragraph_id).unwrap();
                    let words = paragraph_operations::get_words(paragraph);
                    let have_words_in_paragraph = !words.is_empty();
                    let cursor_relative_position = cursor_current_position - paragraph_position;

                    fn recursively_find_previous_word_in_previous_paragraph(
                        document_repository: &dyn DocumentRepositoryTrait,
                        paragraph_group_repository: &dyn ParagraphGroupRepositoryTrait,
                        paragraph_repository: &dyn ParagraphRepositoryTrait,
                        cursor: &mut Cursor,
                    ) -> Result<usize, MovePositionError> {
                        let previous_paragraph_id =
                            paragraph_operations::get_previous_paragraph_id(
                                cursor.position,
                                document_repository,
                            )
                            .ok_or(MovePositionError::NoPreviousWord)?;

                        let paragraph_position = paragraph_operations::paragraph_position(
                            previous_paragraph_id,
                            document_repository,
                            paragraph_group_repository,
                        );

                        let paragraph = paragraph_repository.get(previous_paragraph_id).unwrap();

                        let words = paragraph_operations::get_words(paragraph);

                        let have_words_in_paragraph: bool = !words.is_empty();

                        if have_words_in_paragraph {
                            let last_word = words.last().unwrap();

                            let new_position = paragraph_position + last_word.start;

                            Ok(new_position)
                        } else {
                            cursor.position = paragraph_position;
                            cursor.anchor_position = None;
                            recursively_find_previous_word_in_previous_paragraph(
                                document_repository,
                                paragraph_group_repository,
                                paragraph_repository,
                                cursor,
                            )
                        }
                    }

                    let new_position;

                    if have_words_in_paragraph {
                        let current_word = words
                            .iter()
                            .enumerate()
                            .find(|(_, word)| {
                                let start = word.start;
                                let end = word.end;
                                start <= cursor_relative_position && cursor_relative_position < end
                            })
                            .unwrap();

                        if current_word.0 == 0 {
                            new_position = recursively_find_previous_word_in_previous_paragraph(
                                self.document_repository,
                                self.paragraph_group_repository,
                                self.paragraph_repository,
                                &mut cursor,
                            )?;
                        } else {
                            let previous_word = &words[current_word.0 - 1];

                            new_position = paragraph_position + previous_word.start;
                        }
                    } else {
                        new_position = recursively_find_previous_word_in_previous_paragraph(
                            self.document_repository,
                            self.paragraph_group_repository,
                            self.paragraph_repository,
                            &mut cursor,
                        )?;
                    }

                    match dto.mode {
                        MoveMode::MoveAnchorWithCursor => {
                            cursor.position = new_position;
                            cursor.anchor_position = None;
                            self.cursor_repository.update(cursor)?;
                        }
                        MoveMode::MoveCursorOnly => {
                            if cursor.anchor_position.is_none() {
                                cursor.anchor_position = Some(cursor.position);
                            }
                            cursor.position = new_position;
                            self.cursor_repository.update(cursor)?;
                        }
                    }
                }

                Ok(())
            }

            MoveOperation::End => match dto.mode {
                MoveMode::MoveAnchorWithCursor => {
                    if cursor.position == position_max {
                        return Err(MovePositionError::EndOfDocument);
                    }

                    cursor.position = position_max;
                    cursor.anchor_position = None;
                    self.cursor_repository.update(cursor)?;
                    Ok(())
                }
                MoveMode::MoveCursorOnly => {
                    if cursor.position == position_max {
                        return Err(MovePositionError::EndOfDocument);
                    }
                    if cursor.anchor_position.is_none() {
                        cursor.anchor_position = Some(cursor.position);
                    }
                    cursor.position = position_max;
                    self.cursor_repository.update(cursor)?;
                    Ok(())
                }
            },
            MoveOperation::EndOfWord => {
                let cursor_current_position = cursor.position;

                let paragraph_id = paragraph_operations::get_paragraph_id_by_position(
                    cursor.position,
                    self.document_repository,
                    self.paragraph_group_repository,
                )
                .ok_or(MovePositionError::NoPreviousWord)?;

                let paragraph_position = paragraph_operations::paragraph_position(
                    paragraph_id,
                    self.document_repository,
                    self.paragraph_group_repository,
                );

                let paragraph = self.paragraph_repository.get(paragraph_id).unwrap();

                let paragraph_plain_text = paragraph.get_text();
                let cursor_relative_position = cursor_current_position - paragraph_position;

                let mut _word_start_position = 0;
                let mut word_end_position = 0;
                let mut reached_cursor_position = false;

                for (i, c) in paragraph_plain_text.chars().enumerate() {
                    if c.is_whitespace() {
                        if !reached_cursor_position {
                            _word_start_position = i + 1;
                        }
                        if reached_cursor_position {
                            word_end_position = i;
                            break;
                        }
                    }
                    if i == cursor_relative_position {
                        reached_cursor_position = true;
                    }
                    if reached_cursor_position {
                        word_end_position = i + 1;
                    }
                }

                let new_position = paragraph_position + word_end_position;

                match dto.mode {
                    MoveMode::MoveAnchorWithCursor => {
                        cursor.position = new_position;
                        cursor.anchor_position = None;
                        self.cursor_repository.update(cursor)?;
                        Ok(())
                    }
                    MoveMode::MoveCursorOnly => {
                        if cursor.anchor_position.is_none() {
                            cursor.anchor_position = Some(cursor.position);
                        }
                        cursor.position = new_position;
                        self.cursor_repository.update(cursor)?;
                        Ok(())
                    }
                }
            }
            MoveOperation::EndOfParagraph => {
                let cursor_position = cursor.position;
                let paragraph_id = paragraph_operations::get_paragraph_id_by_position(
                    cursor_position,
                    self.document_repository,
                    self.paragraph_group_repository,
                )
                .expect("No paragraph found");
                let paragraph_position = paragraph_operations::paragraph_position(
                    paragraph_id,
                    self.document_repository,
                    self.paragraph_group_repository,
                );
                let paragraph_size = self
                    .paragraph_repository
                    .get(paragraph_id)
                    .unwrap()
                    .get_char_count();

                let new_position = paragraph_position + paragraph_size;

                match dto.mode {
                    MoveMode::MoveAnchorWithCursor => {
                        cursor.position = new_position;
                        cursor.anchor_position = None;
                        self.cursor_repository.update(cursor)?;
                        Ok(())
                    }
                    MoveMode::MoveCursorOnly => {
                        if cursor.anchor_position.is_none() {
                            cursor.anchor_position = Some(cursor.position);
                        }
                        cursor.position = new_position;
                        self.cursor_repository.update(cursor)?;
                        Ok(())
                    }
                }
            }

            MoveOperation::NextParagraph => {
                for _ in 0..dto.count {
                    let cursor_position = cursor.position;
                    let next_paragraph_id = paragraph_operations::get_next_paragraph_id(
                        cursor_position,
                        self.document_repository,
                    )
                    .ok_or(MovePositionError::NoNextParagraph)?;

                    let paragraph_position = paragraph_operations::paragraph_position(
                        next_paragraph_id,
                        self.document_repository,
                        self.paragraph_group_repository,
                    );

                    let new_position = paragraph_position;

                    match dto.mode {
                        MoveMode::MoveAnchorWithCursor => {
                            cursor.position = new_position;
                            cursor.anchor_position = None;
                            self.cursor_repository.update(cursor)?;
                        }
                        MoveMode::MoveCursorOnly => {
                            if cursor.anchor_position.is_none() {
                                cursor.anchor_position = Some(cursor.position);
                            }
                            cursor.position = new_position;
                            self.cursor_repository.update(cursor)?;
                        }
                    }
                }
                Ok(())
            }

            MoveOperation::NextCharacter => {
                for _ in 0..dto.count {
                    match dto.mode {
                        MoveMode::MoveAnchorWithCursor => {
                            if cursor.position == position_max {
                                return Err(MovePositionError::EndOfDocument);
                            }
                            cursor.position += 1;
                            cursor.anchor_position = None;
                            self.cursor_repository.update(cursor)?;
                        }
                        MoveMode::MoveCursorOnly => {
                            if cursor.position == position_max {
                                return Err(MovePositionError::EndOfDocument);
                            }
                            if cursor.anchor_position.is_none() {
                                cursor.anchor_position = Some(cursor.position);
                            }
                            cursor.position += 1;
                            self.cursor_repository.update(cursor)?;
                        }
                    }
                }
                Ok(())
            }
            MoveOperation::NextWord => {
                for _ in 0..dto.count {
                    let cursor_current_position = cursor.position;

                    let paragraph_id = paragraph_operations::get_paragraph_id_by_position(
                        cursor.position,
                        self.document_repository,
                        self.paragraph_group_repository,
                    )
                    .ok_or(MovePositionError::NoNextWord)?;

                    let paragraph_position = paragraph_operations::paragraph_position(
                        paragraph_id,
                        self.document_repository,
                        self.paragraph_group_repository,
                    );

                    let paragraph = self.paragraph_repository.get(paragraph_id).unwrap();
                    let words = paragraph_operations::get_words(paragraph);
                    let have_words_in_paragraph = !words.is_empty();
                    let cursor_relative_position = cursor_current_position - paragraph_position;

                    fn recursively_find_next_word_in_next_paragraph(
                        document_repository: &dyn DocumentRepositoryTrait,
                        paragraph_group_repository: &dyn ParagraphGroupRepositoryTrait,
                        paragraph_repository: &dyn ParagraphRepositoryTrait,
                        cursor: &mut Cursor,
                    ) -> Result<usize, MovePositionError> {
                        let next_paragraph_id = paragraph_operations::get_next_paragraph_id(
                            cursor.position,
                            document_repository,
                        )
                        .ok_or(MovePositionError::NoNextWord)?;

                        let paragraph_position = paragraph_operations::paragraph_position(
                            next_paragraph_id,
                            document_repository,
                            paragraph_group_repository,
                        );

                        let paragraph = paragraph_repository.get(next_paragraph_id).unwrap();

                        let words = paragraph_operations::get_words(paragraph);

                        let have_words_in_paragraph: bool = !words.is_empty();

                        if have_words_in_paragraph {
                            let first_word = words.first().unwrap();

                            let new_position = paragraph_position + first_word.start;

                            Ok(new_position)
                        } else {
                            cursor.position = paragraph_position;
                            cursor.anchor_position = None;
                            recursively_find_next_word_in_next_paragraph(
                                document_repository,
                                paragraph_group_repository,
                                paragraph_repository,
                                cursor,
                            )
                        }
                    }

                    let new_position;

                    if have_words_in_paragraph {
                        let current_word = words
                            .iter()
                            .enumerate()
                            .find(|(_, word)| {
                                let start = word.start;
                                let end = word.end;
                                start <= cursor_relative_position && cursor_relative_position < end
                            })
                            .unwrap();

                        if current_word.0 == words.len() - 1 {
                            new_position = recursively_find_next_word_in_next_paragraph(
                                self.document_repository,
                                self.paragraph_group_repository,
                                self.paragraph_repository,
                                &mut cursor,
                            )?;
                        } else {
                            let next_word = &words[current_word.0 + 1];

                            new_position = paragraph_position + next_word.start;
                        }
                    } else {
                        new_position = recursively_find_next_word_in_next_paragraph(
                            self.document_repository,
                            self.paragraph_group_repository,
                            self.paragraph_repository,
                            &mut cursor,
                        )?;
                    }

                    match dto.mode {
                        MoveMode::MoveAnchorWithCursor => {
                            cursor.position = new_position;
                            cursor.anchor_position = None;
                            self.cursor_repository.update(cursor)?;
                        }
                        MoveMode::MoveCursorOnly => {
                            if cursor.anchor_position.is_none() {
                                cursor.anchor_position = Some(cursor.position);
                            }
                            cursor.position = new_position;
                            self.cursor_repository.update(cursor)?;
                        }
                    }
                }
                Ok(())
            }
            MoveOperation::NextCell => todo!(),
            MoveOperation::PreviousCell => todo!(),
            MoveOperation::NextRow => todo!(),
            MoveOperation::PreviousRow => todo!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::entities::cursor::Cursor;
    use common::entities::document::{Document, Node, Section};
    use common::entities::paragraph::{Paragraph, TextSlice};
    use common::repositories::cursor_repository::CursorRepository;
    use common::repositories::document_repository::DocumentRepository;
    use common::repositories::paragraph_group_repository::ParagraphGroupRepository;
    use common::repositories::paragraph_repository::ParagraphRepository;

    fn setup<'a>(
        cursor_repository: &'a dyn CursorRepositoryTrait,
        document_repository: &'a mut dyn DocumentRepositoryTrait,
        paragraph_repository: &'a mut dyn ParagraphRepositoryTrait,
        paragraph_group_repository: &'a mut dyn ParagraphGroupRepositoryTrait,
    ) -> MovePositionUseCase<'a> {
        let paragraph1 = Paragraph::new(&[TextSlice::PlainText {
            content: "First line".to_string(),
        }]);
        let paragraph2 = Paragraph::new(&[TextSlice::PlainText {
            content: "Second lin".to_string(),
        }]);
        let paragraph3 = Paragraph::new(&[TextSlice::PlainText {
            content: "Third line".to_string(),
        }]);
        let paragraph4 = Paragraph::new(&[TextSlice::PlainText {
            content: "Fourth lin".to_string(),
        }]);

        let paragraph1_id = paragraph_repository.create(paragraph1);
        let paragraph2_id = paragraph_repository.create(paragraph2);
        let paragraph3_id = paragraph_repository.create(paragraph3);
        let paragraph4_id = paragraph_repository.create(paragraph4);

        paragraph_group_repository
            .add_paragraph_to_a_group(paragraph_repository.get_mut(paragraph1_id).unwrap());
        paragraph_group_repository
            .add_paragraph_to_a_group(paragraph_repository.get_mut(paragraph2_id).unwrap());
        paragraph_group_repository
            .add_paragraph_to_a_group(paragraph_repository.get_mut(paragraph3_id).unwrap());
        paragraph_group_repository
            .add_paragraph_to_a_group(paragraph_repository.get_mut(paragraph4_id).unwrap());

        let section = Section {
            nodes: vec![
                Node::Paragraph {
                    paragraph_id: paragraph1_id,
                },
                Node::Paragraph {
                    paragraph_id: paragraph2_id,
                },
                Node::Paragraph {
                    paragraph_id: paragraph3_id,
                },
            ],
        };

        let document = Document {
            nodes: vec![
                Node::Section(Box::new(section)),
                Node::Paragraph {
                    paragraph_id: paragraph4_id,
                },
            ],
        };

        document_repository.update(document);

        let use_case = MovePositionUseCase::new(
            cursor_repository,
            document_repository,
            paragraph_repository,
            paragraph_group_repository,
        );

        use_case
    }

    #[test]
    fn test_move_start() {
        let cursor_repository = CursorRepository::new();
        let mut document_repository = DocumentRepository::new();
        let mut paragraph_repository = ParagraphRepository::new();
        let mut paragraph_group_repository = ParagraphGroupRepository::new();

        let use_case = setup(
            &cursor_repository,
            &mut document_repository,
            &mut paragraph_repository,
            &mut paragraph_group_repository,
        );

        let dto = MovePositionDTO {
            operation: MoveOperation::Start,
            mode: MoveMode::MoveAnchorWithCursor,
            count: 1,
        };

        let cursor_id = cursor_repository.create(Cursor::new());

        use_case.execute(cursor_id, dto).unwrap();

        let cursor = cursor_repository.get(cursor_id).unwrap();

        assert_eq!(cursor.position, 0);
        assert_eq!(cursor.anchor_position, None);

        let dto = MovePositionDTO {
            operation: MoveOperation::Start,
            mode: MoveMode::MoveAnchorWithCursor,
            count: 1,
        };

        let cursor_id = cursor_repository.create(Cursor {
            id: 0,
            position: 10,
            anchor_position: Some(10),
        });

        use_case.execute(cursor_id, dto).unwrap();

        let cursor = cursor_repository.get(cursor_id).unwrap();

        assert_eq!(cursor.position, 0);
        assert_eq!(cursor.anchor_position, None);
    }

    #[test]
    fn test_move_start_of_paragraph() {
        let cursor_repository = CursorRepository::new();
        let mut document_repository = DocumentRepository::new();
        let mut paragraph_repository = ParagraphRepository::new();
        let mut paragraph_group_repository = ParagraphGroupRepository::new();

        let use_case = setup(
            &cursor_repository,
            &mut document_repository,
            &mut paragraph_repository,
            &mut paragraph_group_repository,
        );

        let dto = MovePositionDTO {
            operation: MoveOperation::StartOfParagraph,
            mode: MoveMode::MoveAnchorWithCursor,
            count: 1,
        };

        let cursor_id = cursor_repository.create(Cursor {
            id: 0,
            position: 10,
            anchor_position: Some(10),
        });

        use_case.execute(cursor_id, dto).unwrap();

        let cursor = cursor_repository.get(cursor_id).unwrap();

        assert_eq!(cursor.position, 0);
        assert_eq!(cursor.anchor_position, None);

        let dto = MovePositionDTO {
            operation: MoveOperation::StartOfParagraph,
            mode: MoveMode::MoveCursorOnly,
            count: 1,
        };

        let cursor_id = cursor_repository.create(Cursor {
            id: 0,
            position: 20,
            anchor_position: Some(10),
        });

        use_case.execute(cursor_id, dto).unwrap();

        let cursor = cursor_repository.get(cursor_id).unwrap();

        assert_eq!(cursor.position, 11);
        assert_eq!(cursor.anchor_position, Some(10));
    }

    #[test]
    fn test_move_start_of_word() {
        let cursor_repository = CursorRepository::new();
        let mut document_repository = DocumentRepository::new();
        let mut paragraph_repository = ParagraphRepository::new();
        let mut paragraph_group_repository = ParagraphGroupRepository::new();

        let use_case = setup(
            &cursor_repository,
            &mut document_repository,
            &mut paragraph_repository,
            &mut paragraph_group_repository,
        );

        // Test moving to the start of the first word

        let dto = MovePositionDTO {
            operation: MoveOperation::StartOfWord,
            mode: MoveMode::MoveAnchorWithCursor,
            count: 1,
        };

        let cursor_id = cursor_repository.create(Cursor {
            id: 0,
            position: 2,
            anchor_position: Some(10),
        });

        use_case.execute(cursor_id, dto).unwrap();

        let cursor = cursor_repository.get(cursor_id).unwrap();

        assert_eq!(cursor.position, 0);
        assert_eq!(cursor.anchor_position, None);

        // test with cursor only

        let dto = MovePositionDTO {
            operation: MoveOperation::StartOfWord,
            mode: MoveMode::MoveCursorOnly,
            count: 1,
        };

        let cursor_id = cursor_repository.create(Cursor {
            id: 0,
            position: 20,
            anchor_position: Some(10),
        });

        use_case.execute(cursor_id, dto).unwrap();

        let cursor = cursor_repository.get(cursor_id).unwrap();

        assert_eq!(cursor.position, 18);
        assert_eq!(cursor.anchor_position, Some(10));

        // Test moving to the start of the second word

        let dto = MovePositionDTO {
            operation: MoveOperation::StartOfWord,
            mode: MoveMode::MoveAnchorWithCursor,
            count: 1,
        };

        let cursor_id = cursor_repository.create(Cursor {
            id: 0,
            position: 6,
            anchor_position: Some(10),
        });

        use_case.execute(cursor_id, dto).unwrap();

        let cursor = cursor_repository.get(cursor_id).unwrap();

        assert_eq!(cursor.position, 6);
        assert_eq!(cursor.anchor_position, None);
    }

    #[test]
    fn test_move_end_of_word() {
        let cursor_repository = CursorRepository::new();
        let mut document_repository = DocumentRepository::new();
        let mut paragraph_repository = ParagraphRepository::new();
        let mut paragraph_group_repository = ParagraphGroupRepository::new();

        let use_case = setup(
            &cursor_repository,
            &mut document_repository,
            &mut paragraph_repository,
            &mut paragraph_group_repository,
        );

        // Test moving to the end of the first word

        let dto = MovePositionDTO {
            operation: MoveOperation::EndOfWord,
            mode: MoveMode::MoveAnchorWithCursor,
            count: 1,
        };

        let cursor_id = cursor_repository.create(Cursor {
            id: 0,
            position: 2,
            anchor_position: Some(10),
        });

        use_case.execute(cursor_id, dto).unwrap();

        let cursor = cursor_repository.get(cursor_id).unwrap();

        assert_eq!(cursor.position, 5);
        assert_eq!(cursor.anchor_position, None);

        // test with cursor only

        let dto = MovePositionDTO {
            operation: MoveOperation::EndOfWord,
            mode: MoveMode::MoveCursorOnly,
            count: 1,
        };

        let cursor_id = cursor_repository.create(Cursor {
            id: 0,
            position: 20,
            anchor_position: Some(10),
        });

        use_case.execute(cursor_id, dto).unwrap();

        let cursor = cursor_repository.get(cursor_id).unwrap();

        assert_eq!(cursor.position, 21);
        assert_eq!(cursor.anchor_position, Some(10));

        // Test moving to the end of the second word

        let dto = MovePositionDTO {
            operation: MoveOperation::EndOfWord,
            mode: MoveMode::MoveAnchorWithCursor,
            count: 1,
        };

        let cursor_id = cursor_repository.create(Cursor {
            id: 0,
            position: 6,
            anchor_position: Some(10),
        });

        use_case.execute(cursor_id, dto).unwrap();

        let cursor = cursor_repository.get(cursor_id).unwrap();

        assert_eq!(cursor.position, 10);
        assert_eq!(cursor.anchor_position, None);
    }
}
