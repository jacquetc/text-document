#[derive(Default, PartialEq, Clone)]
pub struct Font {
    pub weight: Option<Weight>,
    pub style: Option<Style>,
    pub underline: Option<bool>,
    pub strike_out: Option<bool>,
    pub size: Option<FontSize>,
    pub capitalisation: Option<Capitalisation>,
    pub families: Option<Vec<String>>,
    pub letter_spacing: Option<isize>,
    pub letter_spacing_type: Option<SpacingType>,
    /// Sets the word spacing for the font to spacing. Word spacing changes the default spacing between individual words. A positive value increases the word spacing by a corresponding amount of pixels, while a negative value decreases the inter-word spacing accordingly.
    pub word_spacing: Option<isize>,
}

impl Font {
    pub fn new() -> Self {
        Font {
            ..Default::default()
        }
    }

    pub fn set_bold(&mut self) {
        self.weight = Some(Weight::Bold)
    }

    pub fn bold(&self) -> bool {
        self.weight >= Some(Weight::Bold)
    }

    pub fn set_italic(&mut self) {
        self.style = Some(Style::Italic)
    }

    pub fn italic(&self) -> bool {
        self.style >= Some(Style::Italic)
    }

    pub fn family(&self) -> Option<&String> {
        if let Some(families) = &self.families {
            return families.first();
        } else {
            None
        }
    }

    pub fn to_string(&self) -> String {
        "".to_string()
    }
}

#[derive(PartialEq, Clone, Copy)]
pub struct FontSize {
    size_type: SizeType,
    size: usize,
}

impl PartialOrd for FontSize {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.size_type.eq(&other.size_type) {
            self.size.partial_cmp(&other.size)
        } else {
            None
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum SizeType {
    Point,
    Pixel,
}

pub enum UnderlineStyle {}
#[derive(PartialEq, Clone, Copy)]
pub enum Capitalisation {
    MixedCase,
    AllUppercase,
    AllLowercase,
    SmallCaps,
    Capitalize,
}

impl Default for Capitalisation {
    fn default() -> Self {
        Capitalisation::MixedCase
    }
}

#[derive(PartialEq, PartialOrd, Clone, Copy)]
pub enum Style {
    /// Normal glyphs used in unstyled text.
    Normal,
    /// Italic glyphs that are specifically designed for the purpose of representing italicized text.
    Italic,
    /// Glyphs with an italic appearance that are typically based on the unstyled glyphs, but are not fine-tuned for the purpose of representing italicized text.
    Oblique,
}

impl Default for Style {
    fn default() -> Self {
        Style::Normal
    }
}

/// Spacing between letters
#[derive(PartialEq, PartialOrd, Clone, Copy)]
pub enum SpacingType {
    /// A value of 100 will keep the spacing unchanged; a value of 200 will enlarge the spacing after a character by the width of the character itself.
    PercentageSpacing,
    /// A positive value increases the letter spacing by the corresponding pixels; a negative value decreases the spacing.
    AbsoluteSpacing,
}

impl Default for SpacingType {
    fn default() -> Self {
        SpacingType::PercentageSpacing
    }
}

/// Predefined font weights. Compatible with OpenType. A weight of 1 will be thin, whilst 1000 will be extremely black.
#[derive(PartialEq, PartialOrd, Clone, Copy)]
pub enum Weight {
    Thin = 100,
    ExtraLight = 200,
    Light = 300,
    Normal = 400,
    Medium = 500,
    DemiBold = 600,
    Bold = 700,
    ExtraBold = 800,
    Black = 900,
}

impl Default for Weight {
    fn default() -> Self {
        Weight::Normal
    }
}
