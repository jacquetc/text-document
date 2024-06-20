use bitflags::bitflags;
use im_rc::Vector;

#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct Paragraph {
    pub id: usize,
    pub slices: Vector<Slice>,
}

// Define text nodes, which can be plain or formatted
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Slice {
    PlainText { content: String },
    FormattedText { content: String, format: TextFormat },
}

impl Default for Slice {
    fn default() -> Self {
        Slice::PlainText {
            content: String::new(),
        }
    }
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
pub struct TextFormat {
    style: FontStyle,
    decoration: TextDecoration,
    font: Font,
    color: Color,
}
