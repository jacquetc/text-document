use crate::font::Font;
use crate::text_document::Tab;

#[derive(Clone, PartialEq)]
pub enum Format {
    FrameFormat(FrameFormat),
    CharFormat(CharFormat),
    BlockFormat(BlockFormat),
    ImageFormat(ImageFormat),
}

#[derive(Default, Clone, PartialEq)]
pub struct FrameFormat {
    pub height: Option<usize>,
    pub width: Option<usize>,
    pub top_margin: Option<usize>,
    pub bottom_margin: Option<usize>,
    pub left_margin: Option<usize>,
    pub right_margin: Option<usize>,
    pub padding: Option<usize>,
    pub border: Option<usize>,
    pub position: Option<Position>,
}

impl FrameFormat {}

#[derive(Clone, Copy, PartialEq)]
pub enum Position {
    InFlow,
    FloatLeft,
    FloatRight,
}

#[derive(Default, Clone, PartialEq)]
pub struct CharFormat {
    pub anchor_href: Option<String>,
    pub anchor_names: Option<Vec<String>>,
    pub is_anchor: Option<bool>,
    pub font: Font,
    //pub text_outline: Pen
    pub tool_tip: Option<String>,
    //pub underline_color: color
    pub underline_style: Option<UnderlineStyle>,
    pub vertical_alignment: Option<CharVerticalAlignment>,
}

impl CharFormat {
    pub fn new() -> Self {
        CharFormat {
            ..Default::default()
        }
    }
}

impl std::ops::Deref for CharFormat {
    type Target = Font;
    fn deref(&self) -> &Self::Target {
        &self.font
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum CharVerticalAlignment {
    AlignNormal,
    AlignSuperScript,
    AlignSubScript,
    AlignMiddle,
    AlignBottom,
    AlignTop,
    AlignBaseline,
}

#[derive(Clone, Copy, PartialEq)]
pub enum UnderlineStyle {
    NoUnderline,
    SingleUnderline,
    DashUnderline,
    DotLine,
    DashDotLine,
    DashDotDotLine,
    WaveUnderline,
    SpellCheckUnderline,
}

#[derive(Clone, PartialEq)]
pub struct BlockFormat {
    pub alignment: Option<Alignment>,
    pub top_margin: Option<usize>,
    pub bottom_margin: Option<usize>,
    pub left_margin: Option<usize>,
    pub right_margin: Option<usize>,
    pub heading_level: Option<u8>,
    pub indent: Option<u8>,
    pub text_indent: Option<usize>,
    pub tab_positions: Option<Vec<Tab>>,
    pub marker: Option<MarkerType>,
}

impl BlockFormat {
    pub fn new() -> Self {
        BlockFormat {
            ..Default::default()
        }
    }
}

impl Default for BlockFormat {
    fn default() -> Self {
        Self {
            alignment: Some(Alignment::AlignLeft),
            top_margin: Default::default(),
            bottom_margin: Default::default(),
            left_margin: Default::default(),
            right_margin: Default::default(),
            heading_level: Default::default(),
            indent: Default::default(),
            text_indent: Default::default(),
            tab_positions: Default::default(),
            marker: Some(MarkerType::NoMarker),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Alignment {
    AlignLeft,
    AlignRight,
    AlignHCenter,
    AlignJustify,
}

#[derive(Clone, Copy, PartialEq)]
pub enum MarkerType {
    NoMarker,
    Unchecked,
    Checked,
}

#[derive(Default, Clone, PartialEq)]
pub struct ImageFormat {
    char_format: CharFormat,
    pub height: Option<usize>,
    pub width: Option<usize>,
    pub quality: Option<u8>,
    pub name: Option<String>,
}

impl ImageFormat {
    pub fn new() -> Self {
        ImageFormat {
            ..Default::default()
        }
    }
}

impl std::ops::Deref for ImageFormat {
    type Target = CharFormat;
    fn deref(&self) -> &Self::Target {
        &self.char_format
    }
}
