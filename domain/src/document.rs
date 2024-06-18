use std::default;

use bitflags::bitflags;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum DocumentNode {
    Title(String),
    Section(Box<Section>),
    Paragraph(Vec<TextNode>),
    List(Vec<ListItem>),
    // ... other types of nodes
}

impl Default for DocumentNode {
    fn default() -> Self {
        DocumentNode::Paragraph(Vec::new())
    }
}

// Define a section with a content
#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct Section {
    pub content: Vec<DocumentNode>,
}

#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct ListItem {
    pub content: String,
}

// Define text nodes, which can be plain or formatted
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum TextNode {
    PlainText(PlainText),
    FormattedText(FormattedText),
}

impl Default for TextNode {
    fn default() -> Self {
        TextNode::PlainText(PlainText::default())
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct PlainText {
    pub content: String,
}

// Define formatted text with specific formatting attributes
#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct FormattedText {
    pub content: String,
    pub format: TextFormat,
}

#[derive(Debug, Eq, PartialEq, Clone, Default)]
enum FontStyle {
    #[default]
    Regular,
    Bold,
    Italic,
}

bitflags! {
    #[derive(Debug, Eq, PartialEq, Clone, Default)]
    struct TextDecoration: u32 {
        const NONE = 0;
        const UNDERLINE = 0b0001;
        const STRIKETHROUGH = 0b0010;
        // Add more flags here if needed
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Default)]
struct Font {
    family: String,
    size: u8,
}

#[derive(Debug, Eq, PartialEq, Clone, Default)]
struct Color {
    red: u8,
    green: u8,
    blue: u8,
}

// Define the format of the text (font, style, etc.)
#[derive(Debug, Eq, PartialEq, Clone, Default)]
struct TextFormat {
    style: FontStyle,
    decoration: TextDecoration,
    font: Font,
    color: Color,
}

// Define the root of the AST
#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct Document {
    pub nodes: Vec<DocumentNode>,
}

impl Document {
    pub fn new() -> Self {
        Document { nodes: Vec::new() }
    }
}
