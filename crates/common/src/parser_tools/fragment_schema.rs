use serde::{Deserialize, Serialize};

use crate::entities::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentData {
    pub blocks: Vec<FragmentBlock>,
    /// Table fragments extracted from cell selections. Empty for text-only fragments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tables: Vec<FragmentTable>,
}

/// A table (or rectangular sub-region) captured from a cell selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentTable {
    pub rows: usize,
    pub columns: usize,
    pub cells: Vec<FragmentTableCell>,
    /// Index into the parent `FragmentData::blocks` at which this table
    /// should be inserted.  Blocks `[0..index)` come before the table,
    /// blocks `[index..]` come after.  Default `0` for backward compat.
    #[serde(default)]
    pub block_insert_index: usize,
    // ── Table-level formatting ────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fmt_border: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fmt_cell_spacing: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fmt_cell_padding: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fmt_width: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fmt_alignment: Option<Alignment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_widths: Vec<i64>,
}

/// One cell within a [`FragmentTable`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentTableCell {
    pub row: usize,
    pub column: usize,
    pub row_span: usize,
    pub column_span: usize,
    pub blocks: Vec<FragmentBlock>,
    // ── Cell-level formatting ─────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fmt_padding: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fmt_border: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fmt_vertical_alignment: Option<CellVerticalAlignment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fmt_background_color: Option<String>,
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
    pub line_height: Option<i64>,
    pub non_breakable_lines: Option<bool>,
    pub direction: Option<TextDirection>,
    pub background_color: Option<String>,
    pub is_code_block: Option<bool>,
    pub code_language: Option<String>,
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
    /// Returns `true` when this block carries no block-level formatting,
    /// meaning its content is purely inline.
    pub fn is_inline_only(&self) -> bool {
        self.heading_level.is_none()
            && self.list.is_none()
            && self.alignment.is_none()
            && self.indent.unwrap_or(0) == 0
            && self.text_indent.unwrap_or(0) == 0
            && self.marker.is_none()
            && self.top_margin.is_none()
            && self.bottom_margin.is_none()
            && self.left_margin.is_none()
            && self.right_margin.is_none()
            && self.line_height.is_none()
            && self.non_breakable_lines.is_none()
            && self.direction.is_none()
            && self.background_color.is_none()
            && self.is_code_block.is_none()
            && self.code_language.is_none()
    }

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
            line_height: block.fmt_line_height,
            non_breakable_lines: block.fmt_non_breakable_lines,
            direction: block.fmt_direction.clone(),
            background_color: block.fmt_background_color.clone(),
            is_code_block: block.fmt_is_code_block,
            code_language: block.fmt_code_language.clone(),
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
