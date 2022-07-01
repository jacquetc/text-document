use crate::font::Font;
use crate::text_document::Tab;
use crate::ModelError;

pub(crate) type FormatChangeResult = Result<Option<()>, ModelError>;

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Format {
    FrameFormat(FrameFormat),
    CharFormat(CharFormat),
    BlockFormat(BlockFormat),
    ImageFormat(ImageFormat),
}

pub(crate) trait IsFormat {
    fn merge_with(&mut self, other_format: &Self) -> FormatChangeResult
    where
        Self: Sized;
}

#[derive(Default, Clone, Eq, PartialEq, Debug)]
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

impl FrameFormat {
    pub fn new() -> Self {
        FrameFormat {
            ..Default::default()
        }
    }
}

impl IsFormat for FrameFormat {
    fn merge_with(&mut self, other_format: &Self) -> FormatChangeResult
    where
        Self: Sized,
    {
        if let Some(value) = other_format.height {
            self.height = Some(value);
        }
        if let Some(value) = other_format.width {
            self.width = Some(value);
        }
        if let Some(value) = other_format.top_margin {
            self.top_margin = Some(value);
        }
        if let Some(value) = other_format.bottom_margin {
            self.bottom_margin = Some(value);
        }
        if let Some(value) = other_format.left_margin {
            self.left_margin = Some(value);
        }
        if let Some(value) = other_format.right_margin {
            self.right_margin = Some(value);
        }
        if let Some(value) = other_format.padding {
            self.padding = Some(value);
        }
        if let Some(value) = other_format.border {
            self.border = Some(value);
        }
        if let Some(value) = other_format.position {
            self.position = Some(value);
        }

        Ok(Some(()))
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum Position {
    InFlow,
    FloatLeft,
    FloatRight,
}

#[derive(Default, Clone, Eq, PartialEq, Debug)]
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

impl IsFormat for CharFormat {
    fn merge_with(&mut self, other_format: &Self) -> FormatChangeResult
    where
        Self: Sized,
    {
        if let Some(value) = &other_format.anchor_href {
            self.anchor_href = Some(value.clone());
        }

        if let Some(value) = &other_format.anchor_names {
            self.anchor_names = Some(value.clone());
        }

        if let Some(value) = other_format.is_anchor {
            self.is_anchor = Some(value);
        }

        self.font.merge_with(&other_format.font)?;

        if let Some(value) = &other_format.tool_tip {
            self.tool_tip = Some(value.clone());
        }

        if let Some(value) = other_format.underline_style {
            self.underline_style = Some(value);
        }

        if let Some(value) = other_format.vertical_alignment {
            self.vertical_alignment = Some(value);
        }

        Ok(Some(()))
    }
}

impl std::ops::Deref for CharFormat {
    type Target = Font;
    fn deref(&self) -> &Self::Target {
        &self.font
    }
}

impl std::ops::DerefMut for CharFormat {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.font
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum CharVerticalAlignment {
    AlignNormal,
    AlignSuperScript,
    AlignSubScript,
    AlignMiddle,
    AlignBottom,
    AlignTop,
    AlignBaseline,
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
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

#[derive(Clone, Eq, PartialEq, Debug, Default)]
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

impl IsFormat for BlockFormat {
    fn merge_with(&mut self, other_format: &Self) -> FormatChangeResult
    where
        Self: Sized,
    {
        if let Some(value) = other_format.alignment {
            self.alignment = Some(value);
        }
        if let Some(value) = other_format.top_margin {
            self.top_margin = Some(value);
        }
        if let Some(value) = other_format.bottom_margin {
            self.bottom_margin = Some(value);
        }
        if let Some(value) = other_format.left_margin {
            self.left_margin = Some(value);
        }
        if let Some(value) = other_format.right_margin {
            self.right_margin = Some(value);
        }
        if let Some(value) = other_format.heading_level {
            self.heading_level = Some(value);
        }

        if let Some(value) = other_format.indent {
            self.indent = Some(value);
        }

        if let Some(value) = other_format.text_indent {
            self.text_indent = Some(value);
        }

        if let Some(value) = &other_format.tab_positions {
            self.tab_positions = Some(value.clone());
        }

        if let Some(value) = other_format.marker {
            self.marker = Some(value);
        }

        Ok(Some(()))
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum Alignment {
    AlignLeft,
    AlignRight,
    AlignHCenter,
    AlignJustify,
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum MarkerType {
    NoMarker,
    Unchecked,
    Checked,
}

#[derive(Default, Clone, Eq, PartialEq, Debug)]
pub struct ImageFormat {
    pub(crate) char_format: CharFormat,
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

impl IsFormat for ImageFormat {
    /// Merge with the other format. The other format fields, if filled, overwrite the fileds of the first format
    fn merge_with(&mut self, other_format: &Self) -> FormatChangeResult
    where
        Self: Sized,
    {
        self.char_format.merge_with(&other_format.char_format)?;

        if let Some(value) = other_format.height {
            self.height = Some(value)
        }

        if let Some(value) = other_format.width {
            self.width = Some(value)
        }

        if let Some(value) = other_format.quality {
            self.quality = Some(value)
        }

        if let Some(value) = other_format.name.clone() {
            self.name = Some(value)
        }

        Ok(Some(()))
    }
}

impl std::ops::Deref for ImageFormat {
    type Target = CharFormat;
    fn deref(&self) -> &Self::Target {
        &self.char_format
    }
}

pub(crate) trait FormattedElement<F: IsFormat> {
    fn format(&self) -> F;

    fn set_format(&self, format: &F) -> FormatChangeResult;

    fn merge_format(&self, format: &F) -> FormatChangeResult;
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn merge_image_formats() {
        let mut first = ImageFormat::new();
        first.width = Some(40);
        let mut second = ImageFormat::new();
        second.height = Some(10);

        first.merge_with(&second).unwrap();

        assert_eq!(first.width, Some(40));
        assert_eq!(first.height, Some(10));
    }

    #[test]
    fn merge_block_formats() {
        let mut first = BlockFormat::new();
        first.alignment = Some(Alignment::AlignRight);
        let mut second = BlockFormat::new();
        second.left_margin = Some(10);

        first.merge_with(&second).unwrap();

        assert_eq!(first.alignment, Some(Alignment::AlignRight));
        assert_eq!(first.left_margin, Some(10));
    }

    #[test]
    fn merge_frame_formats() {
        let mut first = FrameFormat::new();
        first.position = Some(Position::FloatLeft);
        let mut second = FrameFormat::new();
        second.height = Some(10);

        first.merge_with(&second).unwrap();

        assert_eq!(first.position, Some(Position::FloatLeft));
        assert_eq!(first.height, Some(10));
    }

    #[test]
    fn merge_char_foramts() {
        let mut first = CharFormat::new();
        first.letter_spacing = Some(40);
        let mut second = CharFormat::new();
        second.underline = Some(true);

        first.merge_with(&second).unwrap();

        assert_eq!(first.letter_spacing, Some(40));
        assert_eq!(first.underline, Some(true));
    }
}
