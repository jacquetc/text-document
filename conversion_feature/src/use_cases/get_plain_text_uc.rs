use common::contracts::repositories::DocumentRepositoryTrait;
use common::contracts::repositories::ParagraphRepositoryTrait;
use common::entities::document::{Node, Section};
use common::entities::paragraph::TextSlice;
#[allow(unused_imports)]
use im_rc::vector;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GetPlainTextError {}

pub struct GetPlainTextUseCase<'a> {
    document_repository: &'a dyn DocumentRepositoryTrait,
    paragraph_repository: &'a dyn ParagraphRepositoryTrait,
}

impl<'a> GetPlainTextUseCase<'a> {
    pub fn new(
        document_repository: &'a dyn DocumentRepositoryTrait,
        paragraph_repository: &'a dyn ParagraphRepositoryTrait,
    ) -> GetPlainTextUseCase<'a> {
        GetPlainTextUseCase {
            document_repository,
            paragraph_repository,
        }
    }

    pub fn execute(&self) -> Result<String, GetPlainTextError> {
        let document_repository = self.document_repository;

        let document = document_repository.get();

        let text = document
            .nodes
            .iter()
            .map(|node| match node {
                Node::Section(section) => self.export_section(section),
                Node::Paragraph { paragraph_id } => {
                    self.get_plain_text_from_paragraph_id(*paragraph_id)
                }
                Node::List(list) => list
                    .iter()
                    .map(|list_item: &common::entities::document::ListItem| {
                        let text = self.get_plain_text_from_paragraph_id(list_item.paragraph_id);
                        let mut caret = "• ".to_string();
                        caret.push_str(&text);
                        caret
                    })
                    .collect::<Vec<String>>()
                    .join("\n"),
            })
            .collect::<Vec<String>>()
            .join("\n");

        Ok(text)
    }

    fn get_plain_text_from_paragraph_id(&self, paragraph_id: usize) -> String {
        let paragraph = self.paragraph_repository.get(paragraph_id).unwrap();
        paragraph
            .slices
            .iter()
            .map(|slice| match slice {
                TextSlice::PlainText { content } => content.clone(),
                TextSlice::FormattedText { content, format: _ } => content.clone(),
            })
            .collect::<Vec<String>>()
            .join("")
    }

    fn export_section(&self, section: &Section) -> String {
        section
            .nodes
            .iter()
            .map(|node| match node {
                Node::Section(section) => self.export_section(section),
                Node::Paragraph { paragraph_id } => {
                    self.get_plain_text_from_paragraph_id(*paragraph_id)
                }
                Node::List(list) => list
                    .iter()
                    .map(|list_item: &common::entities::document::ListItem| {
                        let text = self.get_plain_text_from_paragraph_id(list_item.paragraph_id);
                        let mut caret = "• ".to_string();
                        caret.push_str(&text);
                        caret
                    })
                    .collect::<Vec<String>>()
                    .join("\n"),
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {

    use common::contracts::repositories::RepositoryTrait;
    use common::entities::document::Document;
    use common::entities::document::Node;
    use common::entities::paragraph::Paragraph;
    use common::repositories::document_repository::DocumentRepository;
    use common::repositories::paragraph_repository::ParagraphRepository;

    use super::*;

    #[test]
    fn test_export_to_plain_text() {
        let mut document_repository = DocumentRepository::new();
        let mut paragraph_repository = ParagraphRepository::new();

        let document = Document {
            nodes: vector![
                Node::Paragraph {
                    paragraph_id: paragraph_repository.create(Paragraph::new(&[
                        TextSlice::PlainText {
                            content: "First line".to_string(),
                        },
                    ])),
                },
                Node::Paragraph {
                    paragraph_id: paragraph_repository.create(Paragraph::new(&[
                        TextSlice::PlainText {
                            content: "Second line".to_string(),
                        },
                    ])),
                },
                Node::Paragraph {
                    paragraph_id: paragraph_repository.create(Paragraph::new(&[
                        TextSlice::PlainText {
                            content: "Third line".to_string(),
                        },
                    ])),
                },
                Node::Paragraph {
                    paragraph_id: paragraph_repository.create(Paragraph::new(&[
                        TextSlice::PlainText {
                            content: "".to_string(),
                        },
                    ])),
                },
            ],
        };

        document_repository.update(document);

        let use_case = GetPlainTextUseCase::new(&document_repository, &paragraph_repository);

        let text = "First line\nSecond line\nThird line\n";

        let result = use_case.execute().expect("Error");

        assert_eq!(result, text);
    }
}
