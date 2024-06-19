use contracts::persistence::DocumentRepositoryTrait;

use entities::document::PlainText;
use entities::document::{Document, DocumentNode, TextNode};

pub struct ImportFromPlainTextUseCase<'a> {
    document_repository: &'a mut dyn DocumentRepositoryTrait,
}

impl<'a> ImportFromPlainTextUseCase<'a> {
    pub fn new(
        document_repository: &'a mut dyn DocumentRepositoryTrait,
    ) -> ImportFromPlainTextUseCase {
        ImportFromPlainTextUseCase {
            document_repository,
        }
    }

    pub fn execute(&mut self, text: &str) -> Result<(), String> {
        let mut document_nodes: Vec<DocumentNode> = text
            .lines()
            .map(|line| {
                DocumentNode::Paragraph(vec![TextNode::PlainText(PlainText {
                    content: line.to_string(),
                })])
            })
            .collect();

        if text.ends_with('\n') {
            let last_node = DocumentNode::Paragraph(vec![TextNode::PlainText(PlainText {
                content: "".to_string(),
            })]);
            document_nodes.push(last_node);
        }

        let document = Document {
            nodes: document_nodes,
        };

        self.document_repository.update(document);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use entities::document::DocumentNode;

    struct DummyDocumentRepository {
        content: Document,
    }

    impl DocumentRepositoryTrait for DummyDocumentRepository {
        fn get(&self) -> &Document {
            &self.content
        }

        fn get_mut(&mut self) -> &mut Document {
            &mut self.content
        }

        fn update(&mut self, document: Document) {
            self.content = document;
        }
    }

    impl DummyDocumentRepository {
        fn new() -> Self {
            DummyDocumentRepository {
                content: Document::new(),
            }
        }
    }

    #[test]
    fn test_import_from_plain_text() {
        let document = Document::new();
        let mut document_repository = DummyDocumentRepository { content: document };
        let mut import_from_plain_text_uc =
            ImportFromPlainTextUseCase::new(&mut document_repository);

        let text = "line 1\nline 2\nline 3\n";
        let result = import_from_plain_text_uc.execute(text);
        assert!(result.is_ok());

        let document = document_repository.get();
        assert_eq!(document.nodes.len(), 4);
        assert_eq!(
            document.nodes[0],
            DocumentNode::Paragraph(vec![TextNode::PlainText(PlainText {
                content: "line 1".to_string()
            })])
        );
        assert_eq!(
            document.nodes[1],
            DocumentNode::Paragraph(vec![TextNode::PlainText(PlainText {
                content: "line 2".to_string()
            })])
        );
        assert_eq!(
            document.nodes[2],
            DocumentNode::Paragraph(vec![TextNode::PlainText(PlainText {
                content: "line 3".to_string()
            })])
        );
        assert_eq!(
            document.nodes[3],
            DocumentNode::Paragraph(vec![TextNode::PlainText(PlainText {
                content: "".to_string()
            })])
        );
    }
}
