//! Get markdown use case.
//!
//! This module contains the use case for getting markdown from the document.

use common::contracts::repositories::DocumentRepositoryTrait;
use common::contracts::repositories::ParagraphRepositoryTrait;
use common::entities::document::{Node, Section};
use common::entities::paragraph::TextSlice;
#[allow(unused_imports)]
use im_rc::vector;
use std::collections::VecDeque;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GetMarkdownError {}

pub struct GetMarkdownUseCase<'a> {
    document_repository: &'a dyn DocumentRepositoryTrait,
    paragraph_repository: &'a dyn ParagraphRepositoryTrait,
}

impl<'a> GetMarkdownUseCase<'a> {
    pub fn new(
        document_repository: &'a dyn DocumentRepositoryTrait,
        paragraph_repository: &'a dyn ParagraphRepositoryTrait,
    ) -> GetMarkdownUseCase<'a> {
        GetMarkdownUseCase {
            document_repository,
            paragraph_repository,
        }
    }

    pub fn execute(&self) -> Result<String, GetMarkdownError> {
        let document_repository = self.document_repository;

        let document = document_repository.get();

        let text = document
            .nodes
            .iter()
            .map(|node| match node {
                Node::Section(section) => self.export_section(section),
                Node::Paragraph { paragraph_id } => {
                    self.get_markdown_from_paragraph_id(*paragraph_id)
                }
                Node::List(list) => list
                    .iter()
                    .map(|list_item: &common::entities::document::ListItem| {
                        let text = self.get_markdown_from_paragraph_id(list_item.paragraph_id);
                        let indent = "  ";
                        let mut caret = format!("- {}", indent.repeat(list_item.indent_level));
                        caret.push_str(&text);
                        caret
                    })
                    .collect::<Vec<String>>()
                    .join("\n"),
            })
            .collect::<Vec<String>>()
            .join("\n\n");

        Ok(text)
    }

    fn get_markdown_from_paragraph_id(&self, paragraph_id: usize) -> String {
        enum Markup {
            Bold,
            Italic,
            Strikethrough,
            Underline,
            CloseUnderline,
        }

        enum Part {
            PlainText(String),
            FormattedText(String),
            Markup(Markup),
        }

        let mut parts: VecDeque<Part> = VecDeque::new();

        let mut was_bold = false;
        let mut was_italic = false;
        let mut was_strikethrough = false;
        let mut was_underline = false;

        let mut have_closing_bold = false;
        let mut have_closing_italic = false;
        let mut have_closing_strikethrough = false;
        let mut have_closing_underline = false;

        let paragraph = self.paragraph_repository.get(paragraph_id).unwrap();
        paragraph.slices.iter().for_each(|slice| match slice {
            TextSlice::PlainText { content } => {
                // close any open markup
                if was_underline {
                    parts.push_back(Part::Markup(Markup::CloseUnderline));
                    was_underline = false;
                }
                if was_strikethrough {
                    parts.push_back(Part::Markup(Markup::Strikethrough));
                    was_strikethrough = false;
                }
                if was_italic {
                    parts.push_back(Part::Markup(Markup::Italic));
                    was_italic = false;
                }
                if was_bold {
                    parts.push_back(Part::Markup(Markup::Bold));
                    was_bold = false;
                }
                parts.push_back(Part::PlainText(content.clone()));
            }
            TextSlice::FormattedText { content, format } => {
                let content = Self::backslash_escape(content);
                if format.bold.unwrap_or(false) {
                    if !was_bold {
                        parts.push_back(Part::Markup(Markup::Bold));
                        was_bold = true;
                    }
                } else if was_bold {
                    have_closing_bold = true;
                    was_bold = false;
                }
                if format.italic.unwrap_or(false) {
                    if !was_italic {
                        parts.push_back(Part::Markup(Markup::Italic));
                        was_italic = true;
                    }
                } else if was_italic {
                    have_closing_italic = true;
                    was_italic = false;
                }
                if format.strikethrough.unwrap_or(false) {
                    if !was_strikethrough {
                        parts.push_back(Part::Markup(Markup::Strikethrough));
                        was_strikethrough = true;
                    }
                } else if was_strikethrough {
                    have_closing_strikethrough = true;
                    was_strikethrough = false;
                }
                if format.underline.unwrap_or(false) {
                    if !was_underline {
                        parts.push_back(Part::Markup(Markup::Underline));
                        was_underline = true;
                    }
                } else if was_underline {
                    have_closing_underline = true;
                    was_underline = false;
                }

                // closing markup (inverted order)
                if have_closing_underline {
                    parts.push_back(Part::Markup(Markup::CloseUnderline));
                    have_closing_underline = false;
                }
                if have_closing_strikethrough {
                    parts.push_back(Part::Markup(Markup::Strikethrough));
                    have_closing_strikethrough = false;
                }
                if have_closing_italic {
                    parts.push_back(Part::Markup(Markup::Italic));
                    have_closing_italic = false;
                }
                if have_closing_bold {
                    parts.push_back(Part::Markup(Markup::Bold));
                    have_closing_bold = false;
                }

                parts.push_back(Part::FormattedText(content));
            }
        });

        // close any open markup (inverted order)
        if was_underline {
            parts.push_back(Part::Markup(Markup::CloseUnderline));
        }
        if was_strikethrough {
            parts.push_back(Part::Markup(Markup::Strikethrough));
        }
        if was_italic {
            parts.push_back(Part::Markup(Markup::Italic));
        }
        if was_bold {
            parts.push_back(Part::Markup(Markup::Bold));
        }

        // render the parts

        parts
            .iter()
            .map(|part| match part {
                Part::PlainText(content) => content.to_string(),
                Part::FormattedText(content) => content.to_string(),
                Part::Markup(markup) => match markup {
                    Markup::Bold => "**".to_string(),
                    Markup::Italic => "_".to_string(),
                    Markup::Strikethrough => "~~".to_string(),
                    Markup::Underline => "<u>".to_string(),
                    Markup::CloseUnderline => "</u>".to_string(),
                },
            })
            .collect::<Vec<String>>()
            .join("")
    }

    fn backslash_escape(text: &str) -> String {
        // Escape the following characters: \ ` * _ { } [ ] ( ) # + - . !
        let from = "[";
        text.replace('\\', "\\\\")
            .replace('`', "\\`")
            .replace('*', "\\*")
            .replace('_', "\\_")
            .replace('{', "\\{")
            .replace('}', "\\}")
            .replace(from, "\\[")
            .replace(']', "\\]")
            .replace('(', "\\(")
            .replace(')', "\\)")
            .replace('#', "\\#")
            .replace('+', "\\+")
            .replace('-', "\\-")
            .replace('.', "\\.")
            .replace('!', "\\!")
    }

    fn export_section(&self, section: &Section) -> String {
        section
            .nodes
            .iter()
            .map(|node| match node {
                Node::Section(section) => self.export_section(section),
                Node::Paragraph { paragraph_id } => {
                    self.get_markdown_from_paragraph_id(*paragraph_id)
                }
                Node::List(list) => list
                    .iter()
                    .map(|list_item: &common::entities::document::ListItem| {
                        let text = self.get_markdown_from_paragraph_id(list_item.paragraph_id);
                        let indent = "  ";
                        let mut caret = format!("- {}", indent.repeat(list_item.indent_level));
                        caret.push_str(&text);
                        caret
                    })
                    .collect::<Vec<String>>()
                    .join("\n"),
            })
            .collect::<Vec<String>>()
            .join("\n\n")
    }
}

#[cfg(test)]
mod tests {

    use common::contracts::repositories::RepositoryTrait;
    use common::entities::document::Document;
    use common::entities::document::Node;
    use common::entities::paragraph::Paragraph;
    use common::entities::paragraph::TextFormat;
    use common::repositories::document_repository::DocumentRepository;
    use common::repositories::paragraph_repository::ParagraphRepository;

    use super::*;

    #[test]
    fn test_export_to_markdown() {
        let mut document_repository = DocumentRepository::new();
        let mut paragraph_repository = ParagraphRepository::new();

        let document = Document {
            nodes: vector![
                Node::Paragraph {
                    paragraph_id: paragraph_repository.create(Paragraph::new(&[
                        TextSlice::FormattedText {
                            content: "First line".to_string(),
                            format: TextFormat {
                                bold: Some(true),
                                italic: Some(true),
                                ..TextFormat::default()
                            },
                        },
                    ])),
                },
                Node::Paragraph {
                    paragraph_id: paragraph_repository.create(Paragraph::new(&[
                        TextSlice::FormattedText {
                            content: "Second ".to_string(),
                            format: TextFormat {
                                bold: Some(true),
                                ..TextFormat::default()
                            },
                        },
                        TextSlice::FormattedText {
                            content: "line".to_string(),
                            format: TextFormat {
                                bold: Some(true),
                                strikethrough: Some(true),
                                ..TextFormat::default()
                            },
                        },
                    ])),
                },
                Node::Paragraph {
                    paragraph_id: paragraph_repository.create(Paragraph::new(&[
                        TextSlice::FormattedText {
                            content: "Third line".to_string(),
                            format: TextFormat {
                                bold: Some(true),
                                italic: Some(true),
                                ..TextFormat::default()
                            },
                        },
                    ])),
                },
                Node::Paragraph {
                    paragraph_id: paragraph_repository.create(Paragraph::new(&[
                        TextSlice::PlainText {
                            content: "Fourth".to_string(),
                        },
                        TextSlice::FormattedText {
                            content: " line".to_string(),
                            format: TextFormat {
                                underline: Some(true),
                                italic: Some(true),
                                ..TextFormat::default()
                            },
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

        let use_case = GetMarkdownUseCase::new(&document_repository, &paragraph_repository);

        let text =
            "**_First line_**\n\n**Second ~~line~~**\n\n**_Third line_**\n\nFourth_<u> line</u>_\n\n";

        let result = use_case.execute().expect("Error");

        assert_eq!(result, text);
    }
}
