use bitflags::bitflags;

enum DocumentNode {
    Title(String),
    Section(Box<Section>),
    Paragraph(Vec<TextNode>),
    List(Vec<ListItem>),
    // ... other types of nodes
}

// Define a section with a heading and content
struct Section {
    heading: String,
    content: Vec<DocumentNode>,
}


struct ListItem {
    content: String,
}

// Define text nodes, which can be plain or formatted
enum TextNode {
    PlainText(String),
    FormattedText(FormattedText),
}

// Define formatted text with specific formatting attributes
struct FormattedText {
    content: String,
    format: TextFormat,
}

enum FontStyle {
    Regular,
    Bold,
    Italic,
}

bitflags! {
    struct TextDecoration: u32 {
        const NONE = 0;
        const UNDERLINE = 0b0001;
        const STRIKETHROUGH = 0b0010;
        // Add more flags here if needed
    }
}


struct Font {
    family: String,
    size: u8,
}

struct Color {
    red: u8,
    green: u8,
    blue: u8,
}

// Define the format of the text (font, style, etc.)
struct TextFormat {
    style: FontStyle,
    decoration: TextDecoration,
    font: Font,
    color: Color,
}

// Define the root of the AST
struct Document {
    nodes: Vec<DocumentNode>,
}