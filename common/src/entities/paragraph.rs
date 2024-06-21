use bitflags::bitflags;
use im_rc::Vector;

#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct Paragraph {
    pub id: usize,
    pub slices: Vector<TextSlice>,
    pub paragraph_group_id: usize,
}

impl Paragraph {
    pub fn new(slices: &[TextSlice]) -> Self {
        Paragraph {
            id: 0,
            slices: Vector::from(slices),
            paragraph_group_id: 0,
        }
    }

    // Calculate the number of characters in the paragraph
    pub fn get_char_count(&self) -> usize {
        self.slices.iter().map(|slice| slice.get_char_count()).sum()
    }

    // Calculate the number of words in the paragraph
    pub fn get_word_count(&self) -> usize {
        self.get_text().split_whitespace().count()
    }

    pub fn get_text(&self) -> String {
        self.slices
            .iter()
            .map(|slice| match slice {
                TextSlice::PlainText { content } => content.clone(),
                TextSlice::FormattedText { content, .. } => content.clone(),
            })
            .collect::<Vec<String>>()
            .join("")
    }

    pub fn get_slice_count(&self) -> usize {
        self.slices.len()
    }

    pub fn get_slice(&self, index: usize) -> Option<&TextSlice> {
        self.slices.get(index)
    }

    pub fn get_slice_mut(&mut self, index: usize) -> Option<&mut TextSlice> {
        self.slices.get_mut(index)
    }

    pub fn insert_slice(&mut self, index: usize, slice: TextSlice) {
        self.slices.insert(index, slice);
    }

    pub fn remove_slice(&mut self, index: usize) {
        self.slices.remove(index);
    }

    pub fn append_slice(&mut self, slice: TextSlice) {
        self.slices.push_back(slice);
    }

    pub fn prepend_slice(&mut self, slice: TextSlice) {
        self.slices.push_front(slice);
    }

    pub fn clear_slices(&mut self) {
        self.slices.clear();
    }

    // Get the slice at the specified position in the paragraph
    pub fn get_slice_at_relative_position(&self, position: usize) -> Option<(usize, &TextSlice)> {
        if position < self.get_char_count() {
            let mut char_count = 0;
            for (index, slice) in self.slices.iter().enumerate() {
                let slice_char_count = slice.get_char_count();
                if char_count + slice_char_count > position {
                    return Some((index, slice));
                }
                char_count += slice_char_count;
            }
        }
        None
    }

    // Get a mutable slice at the specified position in the paragraph
    pub fn get_slice_at_relative_position_mut(
        &mut self,
        position: usize,
    ) -> Option<(usize, &mut TextSlice)> {
        if position < self.get_char_count() {
            let mut char_count = 0;
            for (index, slice) in self.slices.iter_mut().enumerate() {
                let slice_char_count = slice.get_char_count();
                if char_count + slice_char_count > position {
                    return Some((index, slice));
                }
                char_count += slice_char_count;
            }
        }
        None
    }
}

// Define text nodes, which can be plain or formatted
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum TextSlice {
    PlainText { content: String },
    FormattedText { content: String, format: TextFormat },
}

impl Default for TextSlice {
    fn default() -> Self {
        TextSlice::PlainText {
            content: String::new(),
        }
    }
}

impl TextSlice {
    pub fn get_char_count(&self) -> usize {
        match self {
            TextSlice::PlainText { content } => content.chars().count(),
            TextSlice::FormattedText { content, .. } => content.chars().count(),
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
