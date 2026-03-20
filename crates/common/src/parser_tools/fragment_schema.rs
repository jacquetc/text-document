use serde::{Deserialize, Serialize};

use crate::entities::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentData {
    pub blocks: Vec<FragmentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentBlock {
    pub plain_text: String,
    pub elements: Vec<FragmentElement>,
    pub heading_level: Option<i64>,
    pub list: Option<FragmentList>,
    pub alignment: Option<Alignment>,
    pub indent: Option<i64>,
    pub text_indent: Option<i64>,
    pub marker: Option<MarkerType>,
    pub top_margin: Option<i64>,
    pub bottom_margin: Option<i64>,
    pub left_margin: Option<i64>,
    pub right_margin: Option<i64>,
    pub tab_positions: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentElement {
    pub content: InlineContent,
    pub fmt_font_family: Option<String>,
    pub fmt_font_point_size: Option<i64>,
    pub fmt_font_weight: Option<i64>,
    pub fmt_font_bold: Option<bool>,
    pub fmt_font_italic: Option<bool>,
    pub fmt_font_underline: Option<bool>,
    pub fmt_font_overline: Option<bool>,
    pub fmt_font_strikeout: Option<bool>,
    pub fmt_letter_spacing: Option<i64>,
    pub fmt_word_spacing: Option<i64>,
    pub fmt_anchor_href: Option<String>,
    pub fmt_anchor_names: Vec<String>,
    pub fmt_is_anchor: Option<bool>,
    pub fmt_tooltip: Option<String>,
    pub fmt_underline_style: Option<UnderlineStyle>,
    pub fmt_vertical_alignment: Option<CharVerticalAlignment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentList {
    pub style: ListStyle,
    pub indent: i64,
    pub prefix: String,
    pub suffix: String,
}

impl FragmentElement {
    pub fn from_entity(e: &InlineElement) -> Self {
        FragmentElement {
            content: e.content.clone(),
            fmt_font_family: e.fmt_font_family.clone(),
            fmt_font_point_size: e.fmt_font_point_size,
            fmt_font_weight: e.fmt_font_weight,
            fmt_font_bold: e.fmt_font_bold,
            fmt_font_italic: e.fmt_font_italic,
            fmt_font_underline: e.fmt_font_underline,
            fmt_font_overline: e.fmt_font_overline,
            fmt_font_strikeout: e.fmt_font_strikeout,
            fmt_letter_spacing: e.fmt_letter_spacing,
            fmt_word_spacing: e.fmt_word_spacing,
            fmt_anchor_href: e.fmt_anchor_href.clone(),
            fmt_anchor_names: e.fmt_anchor_names.clone(),
            fmt_is_anchor: e.fmt_is_anchor,
            fmt_tooltip: e.fmt_tooltip.clone(),
            fmt_underline_style: e.fmt_underline_style.clone(),
            fmt_vertical_alignment: e.fmt_vertical_alignment.clone(),
        }
    }

    pub fn to_entity(&self) -> InlineElement {
        InlineElement {
            id: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            content: self.content.clone(),
            fmt_font_family: self.fmt_font_family.clone(),
            fmt_font_point_size: self.fmt_font_point_size,
            fmt_font_weight: self.fmt_font_weight,
            fmt_font_bold: self.fmt_font_bold,
            fmt_font_italic: self.fmt_font_italic,
            fmt_font_underline: self.fmt_font_underline,
            fmt_font_overline: self.fmt_font_overline,
            fmt_font_strikeout: self.fmt_font_strikeout,
            fmt_letter_spacing: self.fmt_letter_spacing,
            fmt_word_spacing: self.fmt_word_spacing,
            fmt_anchor_href: self.fmt_anchor_href.clone(),
            fmt_anchor_names: self.fmt_anchor_names.clone(),
            fmt_is_anchor: self.fmt_is_anchor,
            fmt_tooltip: self.fmt_tooltip.clone(),
            fmt_underline_style: self.fmt_underline_style.clone(),
            fmt_vertical_alignment: self.fmt_vertical_alignment.clone(),
        }
    }
}

impl FragmentBlock {
    pub fn from_entity(block: &Block, elements: &[InlineElement], list: Option<&List>) -> Self {
        FragmentBlock {
            plain_text: block.plain_text.clone(),
            elements: elements.iter().map(FragmentElement::from_entity).collect(),
            heading_level: block.fmt_heading_level,
            list: list.map(FragmentList::from_entity),
            alignment: block.fmt_alignment.clone(),
            indent: block.fmt_indent,
            text_indent: block.fmt_text_indent,
            marker: block.fmt_marker.clone(),
            top_margin: block.fmt_top_margin,
            bottom_margin: block.fmt_bottom_margin,
            left_margin: block.fmt_left_margin,
            right_margin: block.fmt_right_margin,
            tab_positions: block.fmt_tab_positions.clone(),
        }
    }
}

impl FragmentList {
    pub fn from_entity(list: &List) -> Self {
        FragmentList {
            style: list.style.clone(),
            indent: list.indent,
            prefix: list.prefix.clone(),
            suffix: list.suffix.clone(),
        }
    }

    pub fn to_entity(&self) -> List {
        List {
            id: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            style: self.style.clone(),
            indent: self.indent,
            prefix: self.prefix.clone(),
            suffix: self.suffix.clone(),
        }
    }
}
