use crate::{
    contracts::repositories::{DocumentRepositoryTrait, ParagraphGroupRepositoryTrait},
    entities::{
        document::{Node, Section},
        paragraph::Paragraph,
    },
};

fn ordered_paragraph_ids(document_repository: &dyn DocumentRepositoryTrait) -> Vec<usize> {
    fn recursively_get_paragraph_ids_from_section(section: &Section) -> Vec<usize> {
        section
            .nodes
            .iter()
            .flat_map(|node| match node {
                Node::Section(section) => recursively_get_paragraph_ids_from_section(section),
                Node::Paragraph { paragraph_id } => vec![*paragraph_id],
                Node::List(list_items) => list_items
                    .iter()
                    .map(|list_item| list_item.paragraph_id)
                    .collect::<Vec<usize>>(),
            })
            .collect()
    }

    let document = document_repository.get();

    document
        .nodes
        .iter()
        .flat_map(|node| match node {
            Node::Section(section) => recursively_get_paragraph_ids_from_section(section),
            Node::Paragraph { paragraph_id } => vec![*paragraph_id],
            Node::List(list_items) => list_items
                .iter()
                .map(|list_item| list_item.paragraph_id)
                .collect::<Vec<usize>>(),
        })
        .collect()
}

pub fn paragraph_position(
    paragraph_id: usize,
    document_repository: &dyn DocumentRepositoryTrait,
    paragraph_group_repository: &dyn ParagraphGroupRepositoryTrait,
) -> usize {
    let ordered_paragraph_ids = ordered_paragraph_ids(document_repository);
    let mut previous_paragraph_ids: Vec<Option<usize>> = ordered_paragraph_ids
        .iter()
        .take_while(|&id| *id != paragraph_id)
        .map(|&id| Some(id))
        .collect();

    let all_sizes_by_paragraph_ids: Vec<usize> = paragraph_group_repository
        .get_all()
        .iter()
        .filter_map(|group| {
            let mut sizes = Vec::new();
            for paragraph_id_opt in &mut previous_paragraph_ids {
                if let Some(paragraph_id) = paragraph_id_opt {
                    if group.char_count_per_paragraph.contains_key(paragraph_id) {
                        let id = *paragraph_id;
                        *paragraph_id_opt = None;
                        sizes.push(group.char_count_per_paragraph.get(&id).unwrap());
                    }
                }
            }
            if sizes.is_empty() {
                None
            } else {
                Some(sizes)
            }
        })
        .flatten()
        .copied()
        .collect();

    let position =
        all_sizes_by_paragraph_ids.iter().sum::<usize>() + all_sizes_by_paragraph_ids.len();

    position
}

pub fn paragraph_id_by_position(
    cursor_position: usize,
    document_repository: &dyn DocumentRepositoryTrait,
    paragraph_group_repository: &dyn ParagraphGroupRepositoryTrait,
) -> Option<usize> {
    let ordered_paragraph_ids = ordered_paragraph_ids(document_repository);
    let previous_paragraph_id_and_position = ordered_paragraph_ids
        .iter()
        .take_while(|&id| {
            paragraph_position(*id, document_repository, paragraph_group_repository)
                <= cursor_position
        })
        .last()
        .map(|&id| {
            (
                id,
                paragraph_position(id, document_repository, paragraph_group_repository),
            )
        });

    let current_paragraph = previous_paragraph_id_and_position?;
    Some(current_paragraph.0)
}

pub fn paragraph_position_by_cursor_position(
    cursor_position: usize,
    document_repository: &dyn DocumentRepositoryTrait,
    paragraph_group_repository: &dyn ParagraphGroupRepositoryTrait,
) -> usize {
    let ordered_paragraph_ids = ordered_paragraph_ids(document_repository);
    let previous_paragraph_id_and_position = ordered_paragraph_ids
        .iter()
        .take_while(|&id| {
            paragraph_position(*id, document_repository, paragraph_group_repository)
                <= cursor_position
        })
        .last()
        .map(|&id| {
            (
                id,
                paragraph_position(id, document_repository, paragraph_group_repository),
            )
        });

    let current_paragraph = previous_paragraph_id_and_position.unwrap();
    current_paragraph.1
}

pub fn previous_paragraph_id(
    paragraph_id: usize,
    document_repository: &dyn DocumentRepositoryTrait,
) -> Option<usize> {
    let ordered_paragraph_ids = ordered_paragraph_ids(document_repository);

    let index = ordered_paragraph_ids
        .iter()
        .position(|&id| id == paragraph_id)?;
    if index == 0 {
        None
    } else {
        Some(ordered_paragraph_ids[index - 1])
    }
}

pub fn next_paragraph_id(
    paragraph_id: usize,
    document_repository: &dyn DocumentRepositoryTrait,
) -> Option<usize> {
    let ordered_paragraph_ids = ordered_paragraph_ids(document_repository);

    let index = ordered_paragraph_ids
        .iter()
        .position(|&id| id == paragraph_id)?;
    if index == ordered_paragraph_ids.len() - 1 {
        None
    } else {
        Some(ordered_paragraph_ids[index + 1])
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Word {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

pub fn words(paragraph: &Paragraph) -> Vec<Word> {
    let text = paragraph.text();
    let mut words = Vec::new();
    let mut start = 0;
    for (index, c) in text.char_indices() {
        if c.is_whitespace() {
            if start < index {
                words.push(Word {
                    text: text[start..index].to_string(),
                    start,
                    end: index,
                });
            }
            start = index + c.len_utf8();
        }
    }
    if start < text.len() {
        words.push(Word {
            text: text[start..].to_string(),
            start,
            end: text.len(),
        });
    }
    words
}

#[cfg(test)]
mod tests {
    use crate::contracts::repositories::RepositoryTrait;
    use crate::entities::document::{Document, Node, Section};
    use crate::entities::paragraph::{Paragraph, TextSlice};
    use crate::repositories::document_repository::DocumentRepository;
    use crate::repositories::paragraph_group_repository::ParagraphGroupRepository;
    use crate::repositories::paragraph_repository::ParagraphRepository;
    use im_rc::vector;

    use super::*;

    #[test]
    fn test_paragraph_position() {
        let mut document_repository = DocumentRepository::new();
        let mut paragraph_repository = ParagraphRepository::new();
        let mut paragraph_group_repository = ParagraphGroupRepository::new();

        let paragraph1 = Paragraph::new(&[TextSlice::PlainText {
            content: "First line".to_string(),
        }]);
        let paragraph2 = Paragraph::new(&[TextSlice::PlainText {
            content: "Second line".to_string(),
        }]);
        let paragraph3 = Paragraph::new(&[TextSlice::PlainText {
            content: "Third line".to_string(),
        }]);
        let paragraph4 = Paragraph::new(&[TextSlice::PlainText {
            content: "Fourth line".to_string(),
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
            nodes: vector![
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
            nodes: vector![
                Node::Section(Box::new(section)),
                Node::Paragraph {
                    paragraph_id: paragraph4_id,
                },
            ],
        };

        document_repository.update(document);

        let position = paragraph_position(
            paragraph2_id,
            &document_repository,
            &paragraph_group_repository,
        );

        assert_eq!(position, 11);

        let position = paragraph_position(
            paragraph3_id,
            &document_repository,
            &paragraph_group_repository,
        );

        assert_eq!(position, 23);

        let position = paragraph_position(
            paragraph4_id,
            &document_repository,
            &paragraph_group_repository,
        );

        assert_eq!(position, 34);
    }

    #[test]
    fn test_get_words() {
        let paragraph = Paragraph::new(&[TextSlice::PlainText {
            content: "First   line".to_string(),
        }]);

        let words = words(&paragraph);

        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "First");
        assert_eq!(words[0].start, 0);
        assert_eq!(words[0].end, 5);
        assert_eq!(words[1].text, "line");
        assert_eq!(words[1].start, 8);
        assert_eq!(words[1].end, 12);
    }
}
