use common::contracts::repositories::DocumentRepositoryTrait;
use common::contracts::repositories::ParagraphGroupRepositoryTrait;
use common::contracts::repositories::ParagraphRepositoryTrait;

use common::entities::document::{Document, Node};
use common::entities::paragraph::{Paragraph, TextSlice};

pub struct ImportFromPlainTextUseCase<'a> {
    document_repository: &'a mut dyn DocumentRepositoryTrait,
    paragraph_repository: &'a mut dyn ParagraphRepositoryTrait,
    paragraph_group_repository: &'a mut dyn ParagraphGroupRepositoryTrait,
}

impl<'a> ImportFromPlainTextUseCase<'a> {
    pub fn new(
        document_repository: &'a mut dyn DocumentRepositoryTrait,
        paragraph_repository: &'a mut dyn ParagraphRepositoryTrait,
        paragraph_group_repository: &'a mut dyn ParagraphGroupRepositoryTrait,
    ) -> ImportFromPlainTextUseCase<'a> {
        ImportFromPlainTextUseCase {
            document_repository,
            paragraph_repository,
            paragraph_group_repository,
        }
    }

    pub fn execute(&mut self, text: &str) -> Result<(), String> {
        let mut document = Document::new();
        self.paragraph_group_repository.clear();
        self.paragraph_repository.clear();

        text.lines().for_each(|line| {
            let slice = TextSlice::PlainText {
                content: line.to_string(),
            };

            let mut paragraph = Paragraph::new(&[slice]);
            self.paragraph_group_repository
                .add_paragraph_to_a_group(&mut paragraph);

            document.nodes.push(Node::Paragraph {
                paragraph_id: self.paragraph_repository.create(paragraph),
            });
        });

        if text.ends_with('\n') {
            let slice = TextSlice::PlainText {
                content: "".to_string(),
            };

            let mut paragraph = Paragraph::new(&[slice]);
            self.paragraph_group_repository
                .add_paragraph_to_a_group(&mut paragraph);

            document.nodes.push(Node::Paragraph {
                paragraph_id: self.paragraph_repository.create(paragraph),
            });
        }

        self.document_repository.update(document);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use common::contracts::repositories::RepositoryTrait;
    use common::repositories::document_repository::DocumentRepository;
    use common::repositories::paragraph_group_repository::ParagraphGroupRepository;
    use common::repositories::paragraph_repository::ParagraphRepository;

    use super::*;

    #[test]
    fn test_import_from_plain_text() {
        let mut document_repository = DocumentRepository::new();
        let mut paragraph_repository = ParagraphRepository::new();
        let mut paragraph_group_repository = ParagraphGroupRepository::new();

        let mut use_case = ImportFromPlainTextUseCase::new(
            &mut document_repository,
            &mut paragraph_repository,
            &mut paragraph_group_repository,
        );

        let text = "First line\nSecond line\nThird line\n";

        let result = use_case.execute(text);

        assert!(result.is_ok());

        let document = document_repository.get();

        assert_eq!(document.nodes.len(), 4);

        let paragraph_ids: Vec<usize> = document
            .nodes
            .iter()
            .map(|node| {
                if let Node::Paragraph { paragraph_id } = node {
                    *paragraph_id
                } else {
                    panic!("Expected a paragraph node");
                }
            })
            .collect();

        let paragraphs = paragraph_ids
            .iter()
            .map(|&id| paragraph_repository.get(id))
            .collect::<Vec<_>>();

        assert_eq!(paragraphs.len(), 4);

        assert_eq!(paragraphs[0].unwrap().slices.len(), 1);
        match &paragraphs[0].unwrap().slices[0] {
            TextSlice::PlainText { content } => assert!(content == "First line"),
            _ => unreachable!(),
        }

        assert_eq!(paragraphs[1].unwrap().slices.len(), 1);
        match &paragraphs[1].unwrap().slices[0] {
            TextSlice::PlainText { content } => assert!(content == "Second line"),
            _ => unreachable!(),
        }

        assert_eq!(paragraphs[2].unwrap().slices.len(), 1);
        match &paragraphs[2].unwrap().slices[0] {
            TextSlice::PlainText { content } => assert!(content == "Third line"),
            _ => unreachable!(),
        }

        assert_eq!(paragraphs[3].unwrap().slices.len(), 1);
        match &paragraphs[3].unwrap().slices[0] {
            TextSlice::PlainText { content } => assert!(content.is_empty()),
            _ => unreachable!(),
        }
    }
}
