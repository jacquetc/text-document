//! Conversion helpers between public API types and backend DTOs.
//!
//! The backend uses `i64` for all positions/sizes. The public API uses `usize`.
//! All Option mapping between public format structs and backend DTOs lives here.

use crate::{
    BlockFormat, BlockInfo, DocumentStats, FindMatch, FindOptions, FrameFormat, TextFormat,
};
use frontend::common::entities::{
    Alignment, CharVerticalAlignment, MarkerType, UnderlineStyle,
};

// ── Position conversion ─────────────────────────────────────────

pub fn to_i64(v: usize) -> i64 {
    debug_assert!(
        v <= i64::MAX as usize,
        "position overflow: {v}"
    );
    v as i64
}

pub fn to_usize(v: i64) -> usize {
    debug_assert!(v >= 0, "negative position: {v}");
    v.max(0) as usize
}

fn opt_u32_to_i64(v: Option<u32>) -> i64 {
    v.map(|x| x as i64).unwrap_or(0)
}

fn opt_u8_to_i64(v: Option<u8>) -> i64 {
    v.map(|x| x as i64).unwrap_or(0)
}

fn opt_i32_to_i64(v: Option<i32>) -> i64 {
    v.map(|x| x as i64).unwrap_or(0)
}

fn opt_bool(v: Option<bool>) -> bool {
    v.unwrap_or(false)
}

// ── DocumentStats ───────────────────────────────────────────────

impl From<&frontend::document_inspection::DocumentStatsDto> for DocumentStats {
    fn from(dto: &frontend::document_inspection::DocumentStatsDto) -> Self {
        Self {
            character_count: to_usize(dto.character_count),
            word_count: to_usize(dto.word_count),
            block_count: to_usize(dto.block_count),
            frame_count: to_usize(dto.frame_count),
            image_count: to_usize(dto.image_count),
            list_count: to_usize(dto.list_count),
        }
    }
}

// ── BlockInfo ───────────────────────────────────────────────────

impl From<&frontend::document_inspection::BlockInfoDto> for BlockInfo {
    fn from(dto: &frontend::document_inspection::BlockInfoDto) -> Self {
        Self {
            block_id: to_usize(dto.block_id),
            block_number: to_usize(dto.block_number),
            start: to_usize(dto.block_start),
            length: to_usize(dto.block_length),
        }
    }
}

// ── FindMatch / FindOptions ─────────────────────────────────────

impl FindOptions {
    pub(crate) fn to_find_text_dto(
        &self,
        query: &str,
        start_position: usize,
    ) -> frontend::document_search::FindTextDto {
        frontend::document_search::FindTextDto {
            query: query.into(),
            case_sensitive: self.case_sensitive,
            whole_word: self.whole_word,
            use_regex: self.use_regex,
            search_backward: self.search_backward,
            start_position: to_i64(start_position),
        }
    }

    pub(crate) fn to_find_all_dto(&self, query: &str) -> frontend::document_search::FindAllDto {
        frontend::document_search::FindAllDto {
            query: query.into(),
            case_sensitive: self.case_sensitive,
            whole_word: self.whole_word,
            use_regex: self.use_regex,
        }
    }

    pub(crate) fn to_replace_dto(
        &self,
        query: &str,
        replacement: &str,
        replace_all: bool,
    ) -> frontend::document_search::ReplaceTextDto {
        frontend::document_search::ReplaceTextDto {
            query: query.into(),
            replacement: replacement.into(),
            case_sensitive: self.case_sensitive,
            whole_word: self.whole_word,
            use_regex: self.use_regex,
            replace_all,
        }
    }
}

pub fn find_result_to_match(dto: &frontend::document_search::FindResultDto) -> Option<FindMatch> {
    if dto.found {
        Some(FindMatch {
            position: to_usize(dto.position),
            length: to_usize(dto.length),
        })
    } else {
        None
    }
}

pub fn find_all_to_matches(dto: &frontend::document_search::FindAllResultDto) -> Vec<FindMatch> {
    dto.positions
        .iter()
        .zip(dto.lengths.iter())
        .map(|(&pos, &len)| FindMatch {
            position: to_usize(pos),
            length: to_usize(len),
        })
        .collect()
}

// ── TextFormat → SetTextFormatDto ───────────────────────────────

impl TextFormat {
    pub(crate) fn to_set_dto(
        &self,
        position: usize,
        anchor: usize,
    ) -> frontend::document_formatting::SetTextFormatDto {
        frontend::document_formatting::SetTextFormatDto {
            position: to_i64(position),
            anchor: to_i64(anchor),
            font_family: self.font_family.clone().unwrap_or_default(),
            font_point_size: opt_u32_to_i64(self.font_point_size),
            font_weight: opt_u32_to_i64(self.font_weight),
            font_bold: opt_bool(self.font_bold),
            font_italic: opt_bool(self.font_italic),
            font_underline: opt_bool(self.font_underline),
            font_overline: opt_bool(self.font_overline),
            font_strikeout: opt_bool(self.font_strikeout),
            letter_spacing: opt_i32_to_i64(self.letter_spacing),
            word_spacing: opt_i32_to_i64(self.word_spacing),
            underline_style: self
                .underline_style
                .clone()
                .unwrap_or(UnderlineStyle::NoUnderline),
            vertical_alignment: self
                .vertical_alignment
                .clone()
                .unwrap_or(CharVerticalAlignment::Normal),
        }
    }

    pub(crate) fn to_merge_dto(
        &self,
        position: usize,
        anchor: usize,
    ) -> frontend::document_formatting::MergeTextFormatDto {
        frontend::document_formatting::MergeTextFormatDto {
            position: to_i64(position),
            anchor: to_i64(anchor),
            font_family: self.font_family.clone(),
            font_bold: self.font_bold,
            font_italic: self.font_italic,
            font_underline: self.font_underline,
        }
    }
}

// ── InlineElement entity → TextFormat ───────────────────────────

impl From<&frontend::inline_element::dtos::InlineElementDto> for TextFormat {
    fn from(el: &frontend::inline_element::dtos::InlineElementDto) -> Self {
        Self {
            font_family: el.fmt_font_family.clone(),
            font_point_size: el.fmt_font_point_size.map(|v| v as u32),
            font_weight: el.fmt_font_weight.map(|v| v as u32),
            font_bold: el.fmt_font_bold,
            font_italic: el.fmt_font_italic,
            font_underline: el.fmt_font_underline,
            font_overline: el.fmt_font_overline,
            font_strikeout: el.fmt_font_strikeout,
            letter_spacing: el.fmt_letter_spacing.map(|v| v as i32),
            word_spacing: el.fmt_word_spacing.map(|v| v as i32),
            underline_style: el.fmt_underline_style.clone(),
            vertical_alignment: el.fmt_vertical_alignment.clone(),
            anchor_href: el.fmt_anchor_href.clone(),
            anchor_names: el.fmt_anchor_names.clone(),
            is_anchor: el.fmt_is_anchor,
            tooltip: el.fmt_tooltip.clone(),
        }
    }
}

// ── BlockFormat ─────────────────────────────────────────────────

impl BlockFormat {
    pub(crate) fn to_set_dto(
        &self,
        position: usize,
        anchor: usize,
    ) -> frontend::document_formatting::SetBlockFormatDto {
        frontend::document_formatting::SetBlockFormatDto {
            position: to_i64(position),
            anchor: to_i64(anchor),
            alignment: self
                .alignment
                .clone()
                .unwrap_or(Alignment::Left),
            heading_level: opt_u8_to_i64(self.heading_level),
            indent: opt_u8_to_i64(self.indent),
            marker: self
                .marker
                .clone()
                .unwrap_or(MarkerType::NoMarker),
        }
    }
}

impl From<&frontend::block::dtos::BlockDto> for BlockFormat {
    fn from(b: &frontend::block::dtos::BlockDto) -> Self {
        Self {
            alignment: b.fmt_alignment.clone(),
            top_margin: b.fmt_top_margin.map(|v| v as i32),
            bottom_margin: b.fmt_bottom_margin.map(|v| v as i32),
            left_margin: b.fmt_left_margin.map(|v| v as i32),
            right_margin: b.fmt_right_margin.map(|v| v as i32),
            heading_level: b.fmt_heading_level.map(|v| v as u8),
            indent: b.fmt_indent.map(|v| v as u8),
            text_indent: b.fmt_text_indent.map(|v| v as i32),
            marker: b.fmt_marker.clone(),
            tab_positions: b.fmt_tab_positions.iter().map(|&v| v as i32).collect(),
        }
    }
}

// ── FrameFormat ─────────────────────────────────────────────────

impl FrameFormat {
    pub(crate) fn to_set_dto(
        &self,
        position: usize,
        anchor: usize,
        frame_id: usize,
    ) -> frontend::document_formatting::SetFrameFormatDto {
        frontend::document_formatting::SetFrameFormatDto {
            position: to_i64(position),
            anchor: to_i64(anchor),
            frame_id: to_i64(frame_id),
            height: opt_i32_to_i64(self.height),
            width: opt_i32_to_i64(self.width),
            top_margin: opt_i32_to_i64(self.top_margin),
            bottom_margin: opt_i32_to_i64(self.bottom_margin),
            left_margin: opt_i32_to_i64(self.left_margin),
            right_margin: opt_i32_to_i64(self.right_margin),
            padding: opt_i32_to_i64(self.padding),
            border: opt_i32_to_i64(self.border),
        }
    }
}
