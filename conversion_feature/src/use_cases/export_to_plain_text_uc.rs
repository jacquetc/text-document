use common::contracts::repositories::DocumentRepositoryTrait;
use common::entities::document::{DocumentNode, Section, TextNode};

pub struct ExportToPlainTextUseCase<'a> {
    document_repository: &'a dyn DocumentRepositoryTrait,
}

impl<'a> ExportToPlainTextUseCase<'a> {
    pub fn new(document_repository: &'a dyn DocumentRepositoryTrait) -> ExportToPlainTextUseCase {
        ExportToPlainTextUseCase {
            document_repository,
        }
    }

    pub fn execute(&self) -> String {
        let document_repository = self.document_repository;

        let document = document_repository.get();

        document
            .nodes
            .iter()
            .map(|node| match node {
                DocumentNode::Title(text) => text.clone(),
                DocumentNode::Section(section) => Self::export_section(section),
                DocumentNode::Paragraph(paragraph) => paragraph
                    .iter()
                    .map(|text_node| match text_node {
                        TextNode::PlainText(plain_text) => plain_text.content.clone(),
                        TextNode::FormattedText(formatted_text) => formatted_text.content.clone(),
                    })
                    .collect::<Vec<String>>()
                    .join(""),
                DocumentNode::List(list) => list
                    .iter()
                    .map(|list_item| {
                        let text = list_item.content.clone();
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

    fn export_section(section: &Section) -> String {
        section
            .content
            .iter()
            .map(|node| match node {
                DocumentNode::Title(text) => text.clone(),
                DocumentNode::Paragraph(paragraph) => paragraph
                    .iter()
                    .map(|text_node| match text_node {
                        TextNode::PlainText(plain_text) => plain_text.content.clone(),
                        TextNode::FormattedText(formatted_text) => formatted_text.content.clone(),
                    })
                    .collect::<Vec<String>>()
                    .join(""),
                DocumentNode::List(list) => list
                    .iter()
                    .map(|list_item| list_item.content.clone())
                    .collect::<Vec<String>>()
                    .join("\n"),
                DocumentNode::Section(section) => Self::export_section(section),
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::entities::document::{Document, DocumentNode, Section, TextNode, ListItem, PlainText};

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
    
    #[test]
    fn test_export_to_plain_text() {
        let mut document = Document::new();
        document.nodes.push(DocumentNode::Title("Title".to_string()));
        document.nodes.push(DocumentNode::Section(Box::new(Section {
            content: vec![
                DocumentNode::Title("Section Title".to_string()),
                DocumentNode::Paragraph(vec![TextNode::PlainText(PlainText {
                    content: "Paragraph".to_string(),
                })]),
                DocumentNode::List(vec![ListItem {
                    content: "List item".to_string(),
                }]),
            ],
        })));
        document.nodes.push(DocumentNode::Paragraph(vec![TextNode::PlainText(PlainText {
            content: "Paragraph".to_string(),
        })]));
        document.nodes.push(DocumentNode::List(vec![ListItem {
            content: "List item".to_string(),
        }]));

        let document_repository = DummyDocumentRepository { content: document };

        let use_case = ExportToPlainTextUseCase::new(&document_repository);
        let result = use_case.execute();

        let expected = "Title\nSection Title\nParagraph\n• List item\nParagraph\n• List item";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_export_to_plain_text_empty() {
        let document = Document::new();
        let document_repository = DummyDocumentRepository { content: document };

        let use_case = ExportToPlainTextUseCase::new(&document_repository);
        let result = use_case.execute();

        let expected = "";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_export_to_plain_text_nested_sections() {
        let mut document = Document::new();
        document.nodes.push(DocumentNode::Section(Box::new(Section {
            content: vec![
                DocumentNode::Title("Title".to_string()),
                DocumentNode::Section(Box::new(Section {
                    content: vec![
                        DocumentNode::Title("Section Title".to_string()),
                        DocumentNode::Paragraph(vec![TextNode::PlainText(PlainText {
                            content: "Paragraph".to_string(),
                        })]),
                        DocumentNode::List(vec![ListItem {
                            content: "List item".to_string(),
                        }]),
                    ],
                })),
                DocumentNode::Paragraph(vec![TextNode::PlainText(PlainText {
                    content: "Paragraph".to_string(),
                })]),
                DocumentNode::List(vec![ListItem {
                    content: "List item".to_string(),
                }]),
            ],
        })));

        let document_repository = DummyDocumentRepository { content: document };

        let use_case = ExportToPlainTextUseCase::new(&document_repository);
        let result = use_case.execute();

        let expected = "Title\nSection Title\nParagraph\n• List item\nParagraph\n• List item";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_export_to_plain_text_nested_sections_empty() {
        let mut document = Document::new();
        document.nodes.push(DocumentNode::Section(Box::new(Section {
            content: vec![
                DocumentNode::Title("Title".to_string()),
                DocumentNode::Section(Box::new(Section {
                    content: vec![],
                })),
                DocumentNode::Paragraph(vec![]),
                DocumentNode::List(vec![]),
            ],
        })));

        let document_repository = DummyDocumentRepository { content: document };

        let use_case = ExportToPlainTextUseCase::new(&document_repository);
        let result = use_case.execute();

        let expected = "Title";
        assert_eq!(result, expected);
    }

}