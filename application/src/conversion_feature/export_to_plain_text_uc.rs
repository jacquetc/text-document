use contracts::persistence::DocumentRepositoryTrait;
use entities::document::PlainText;
use entities::document::{DocumentNode, Section, TextNode};

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
