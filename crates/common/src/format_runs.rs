//! Per-block character formatting as sorted, non-overlapping byte spans.
//!
//! Phase 1 of the rope migration: replaces the `InlineElement`-per-run
//! model. Each block carries a `Vec<FormatRun>` (formatting) and a
//! `Vec<ImageAnchor>` (image positions). The block's `plain_text` is
//! the authoritative character source for byte offsets used by both.
//!
//! Invariants are documented on [`FormatRun`] and enforced by
//! [`debug_assert_well_formed`] and by [`splice_range`] / [`shift_after`]
//! which rebuild the run list while preserving them.

use crate::entities::{CharVerticalAlignment, UnderlineStyle};
use serde::{Deserialize, Serialize};

/// Content type for an inline segment: text, image, or empty.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub enum InlineContent {
    #[default]
    Empty,
    Text(String),
    Image {
        name: String,
        width: i64,
        height: i64,
        quality: i64,
    },
}

/// A lean view type representing one inline segment (text or image) with its
/// associated formatting. Replaces the legacy `InlineElement` entity struct
/// (which included id, created_at, updated_at). Used by readers to consume
/// per-element data synthesized from `(plain_text, format_runs, block_images)`.
///
/// Fields mirror the old `InlineElement.fmt_*` naming for backward compatibility
/// in reader logic.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct InlineSegment {
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

/// Character-level formatting for a contiguous byte span.
///
/// Field names mirror `InlineElement.fmt_*` from `entities.rs` so callers
/// can move between the old per-element model and this per-run model
/// without renaming.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterFormat {
    pub font_family: Option<String>,
    pub font_point_size: Option<i64>,
    pub font_weight: Option<i64>,
    pub font_bold: Option<bool>,
    pub font_italic: Option<bool>,
    pub font_underline: Option<bool>,
    pub font_overline: Option<bool>,
    pub font_strikeout: Option<bool>,
    pub letter_spacing: Option<i64>,
    pub word_spacing: Option<i64>,
    pub anchor_href: Option<String>,
    pub anchor_names: Vec<String>,
    pub is_anchor: Option<bool>,
    pub tooltip: Option<String>,
    pub underline_style: Option<UnderlineStyle>,
    pub vertical_alignment: Option<CharVerticalAlignment>,
}

/// One run of identical character formatting inside a block. Byte offsets
/// are relative to the block's `plain_text` (Phase 1) or to the block's
/// rope range (Phase 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatRun {
    pub byte_start: u32,
    pub byte_end: u32,
    pub format: CharacterFormat,
}

/// An image embedded at a specific byte position inside a block. In
/// Phase 1 the byte position is an index into the block's `plain_text`;
/// in Phase 2 it points at the U+FFFC sentinel character in the rope.
///
/// Images carry their own [`CharacterFormat`] because vertical alignment
/// and anchor metadata apply per inline run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageAnchor {
    pub byte_offset: u32,
    pub name: String,
    pub width: i64,
    pub height: i64,
    pub quality: i64,
    pub format: CharacterFormat,
}

/// Debug-only invariant check. Run from `debug_assert!` callsites in
/// the use cases that mutate format runs. Cheap: O(n) where n is the
/// run count (typically < 100 per block in real prose).
///
/// # Invariants
/// 1. Runs are sorted by `byte_start` ascending.
/// 2. Each run has `byte_start < byte_end`.
/// 3. Runs are non-overlapping: `runs[i].byte_end <= runs[i+1].byte_start`.
/// 4. The last run's `byte_end` does not exceed `block_text_len`.
/// 5. Adjacent runs with identical format are coalesced (no two
///    consecutive runs satisfy `byte_end == next.byte_start &&
///    format == next.format`).
pub fn debug_assert_well_formed(runs: &[FormatRun], block_text_len: usize) {
    if runs.is_empty() {
        return;
    }
    for run in runs {
        debug_assert!(
            run.byte_start < run.byte_end,
            "format run is empty or reversed: {run:?}"
        );
    }
    for i in 0..runs.len() - 1 {
        debug_assert!(
            runs[i].byte_end <= runs[i + 1].byte_start,
            "format runs overlap or unsorted at {i}: {:?} then {:?}",
            runs[i],
            runs[i + 1]
        );
        debug_assert!(
            !(runs[i].byte_end == runs[i + 1].byte_start
                && runs[i].format == runs[i + 1].format),
            "adjacent identical format runs at {i} not coalesced: {:?}",
            runs[i]
        );
    }
    debug_assert!(
        runs.last().unwrap().byte_end as usize <= block_text_len,
        "last format run {:?} exceeds block text len {block_text_len}",
        runs.last().unwrap()
    );
}

/// Merge adjacent runs that have identical formatting. O(n).
pub fn coalesce_in_place(runs: &mut Vec<FormatRun>) {
    if runs.len() < 2 {
        return;
    }
    let mut write = 0usize;
    for read in 1..runs.len() {
        if runs[write].byte_end == runs[read].byte_start
            && runs[write].format == runs[read].format
        {
            runs[write].byte_end = runs[read].byte_end;
        } else {
            write += 1;
            if write != read {
                runs[write] = runs[read].clone();
            }
        }
    }
    runs.truncate(write + 1);
}

/// Replace the runs covering `range` with `replacement`, preserving the
/// invariants. Runs that straddle the range boundary are clipped on
/// either side; runs fully contained are removed.
///
/// The replacement byte ranges must lie within `range` and themselves
/// be well-formed (sorted, non-overlapping). The function does NOT
/// shift bytes after `range.end` — callers wanting to splice in a
/// different-length text must call [`shift_after`] first or after,
/// depending on whether the text length is changing.
pub fn splice_range(
    runs: &mut Vec<FormatRun>,
    range: std::ops::Range<u32>,
    replacement: Vec<FormatRun>,
) {
    debug_assert!(range.start <= range.end);
    for r in &replacement {
        debug_assert!(r.byte_start >= range.start && r.byte_end <= range.end);
    }

    let mut result: Vec<FormatRun> = Vec::with_capacity(runs.len() + replacement.len());

    // Keep / clip everything strictly before range.start.
    for run in runs.iter() {
        if run.byte_end <= range.start {
            result.push(run.clone());
        } else if run.byte_start < range.start {
            // Run straddles range.start: keep the left part.
            result.push(FormatRun {
                byte_start: run.byte_start,
                byte_end: range.start,
                format: run.format.clone(),
            });
        }
    }

    // Insert the replacement runs.
    result.extend(replacement);

    // Keep / clip everything starting at or after range.end.
    for run in runs.iter() {
        if run.byte_start >= range.end {
            result.push(run.clone());
        } else if run.byte_end > range.end {
            // Run straddles range.end: keep the right part.
            result.push(FormatRun {
                byte_start: range.end,
                byte_end: run.byte_end,
                format: run.format.clone(),
            });
        }
    }

    coalesce_in_place(&mut result);
    *runs = result;
}

/// Shift the byte offsets of every run whose `byte_start >= threshold`
/// by `delta`. Used after a text insert/delete to keep downstream runs
/// in sync with the new block text. Runs strictly before the threshold
/// are unaffected; runs that straddle the threshold are left alone
/// (the caller should have spliced them first).
///
/// Panics in debug mode if `delta` would underflow a run's offset.
pub fn shift_after(runs: &mut Vec<FormatRun>, threshold: u32, delta: i32) {
    for run in runs.iter_mut() {
        if run.byte_start >= threshold {
            let new_start = (run.byte_start as i64) + (delta as i64);
            let new_end = (run.byte_end as i64) + (delta as i64);
            debug_assert!(new_start >= 0 && new_end >= new_start);
            run.byte_start = new_start as u32;
            run.byte_end = new_end as u32;
        }
    }
}

/// Synthesize a stable per-fragment id from a block id and byte offset
/// within that block. Used to preserve the `element_id` field in
/// `FragmentContent::{Text, Image}` once the InlineElement entity is
/// removed. Two fragments at the same (block_id, byte_start) always
/// produce the same id; a fragment that moves to a new byte_start
/// (e.g. due to an insert upstream) gets a new id.
///
/// Bit layout (u64): bit 62 = synth tag (so synthesized ids never
/// collide with real entity ids issued by the store's counter, which
/// start at 1 and grow upward). Bits 32..62 = block id (1 billion
/// blocks per document, 30 bits). Bottom 32 bits = byte offset (4 GB
/// per block). The top bit stays zero so the value fits in positive
/// i64 range — public DTOs expose element_id as i64.
pub fn synth_element_id(block_id: u64, byte_start: u32) -> u64 {
    const SYNTH_TAG: u64 = 0x4000_0000_0000_0000;
    SYNTH_TAG | ((block_id & 0x3FFF_FFFF) << 32) | (byte_start as u64)
}

/// Same as `shift_after` for image anchors. Anchors AT the threshold are
/// shifted (treated as part of the inserted region's right side).
pub fn shift_images_after(images: &mut [ImageAnchor], threshold: u32, delta: i32) {
    for img in images.iter_mut() {
        if img.byte_offset >= threshold {
            let new_off = (img.byte_offset as i64) + (delta as i64);
            debug_assert!(new_off >= 0);
            img.byte_offset = new_off as u32;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// Composite helpers used by writer use cases. These keep the per-block
// run / image vectors well-formed under insert / delete / split.
// ─────────────────────────────────────────────────────────────────────

/// Apply an "insert `inserted_bytes` of text at `byte_offset`" mutation
/// to a block's runs in place. Runs strictly before the offset are
/// unchanged; runs strictly after are shifted by +inserted_bytes; runs
/// that straddle the offset are extended (the inserted text inherits
/// the surrounding run's format — Qt / ProseMirror convention).
pub fn shift_runs_for_insert(runs: &mut Vec<FormatRun>, byte_offset: u32, inserted_bytes: u32) {
    if inserted_bytes == 0 {
        return;
    }
    for run in runs.iter_mut() {
        if run.byte_start >= byte_offset {
            run.byte_start += inserted_bytes;
            run.byte_end += inserted_bytes;
        } else if run.byte_end >= byte_offset {
            // Run straddles the insertion point, or its right edge sits
            // exactly on it. In both cases the inserted text inherits
            // this run's format (Qt convention).
            run.byte_end += inserted_bytes;
        }
    }
}

/// Apply a "delete byte range `[byte_start..byte_end)`" mutation to a
/// block's runs. Splices the range with empty replacement (clipping
/// straddling runs) and shifts everything past `byte_end` back by the
/// deleted length. Adjacent runs that end up equal-format are coalesced.
pub fn shift_runs_for_delete(runs: &mut Vec<FormatRun>, byte_start: u32, byte_end: u32) {
    if byte_end <= byte_start {
        return;
    }
    splice_range(runs, byte_start..byte_end, Vec::new());
    let delta = (byte_end - byte_start) as i32;
    shift_after(runs, byte_end, -delta);
    // The shift can make a left-clipped run abut a shifted trailing run
    // with identical format; coalesce once more to restore the invariant.
    coalesce_in_place(runs);
}

/// Apply an "insert" shift to a block's image anchors. Anchors at or
/// past the offset move forward by `inserted_bytes`.
pub fn shift_images_for_insert(
    images: &mut [ImageAnchor],
    byte_offset: u32,
    inserted_bytes: u32,
) {
    if inserted_bytes == 0 {
        return;
    }
    for img in images.iter_mut() {
        if img.byte_offset >= byte_offset {
            img.byte_offset += inserted_bytes;
        }
    }
}

/// Apply a "delete" mutation to a block's image anchors. Anchors whose
/// `byte_offset` falls inside `[byte_start..byte_end)` are removed;
/// anchors at or past `byte_end` shift back by the deleted length.
/// Returns the number of anchors removed.
pub fn shift_images_for_delete(
    images: &mut Vec<ImageAnchor>,
    byte_start: u32,
    byte_end: u32,
) -> usize {
    if byte_end <= byte_start {
        return 0;
    }
    let before = images.len();
    images.retain(|i| !(i.byte_offset >= byte_start && i.byte_offset < byte_end));
    let removed = before - images.len();
    let delta = (byte_end - byte_start) as i32;
    shift_images_after(images, byte_end, -delta);
    removed
}

/// Translate a logical character offset (counting text characters AND
/// image positions interleaved by their `byte_offset`) into a UTF-8
/// byte offset within `plain_text`. Used by writer use cases to map a
/// document-space char position to the byte position where text edits
/// should land in `block.plain_text`.
///
/// Images contribute 1 logical character but 0 bytes in `plain_text`.
/// Images at the same byte_offset are visited in their stored order.
pub fn logical_offset_to_byte(
    plain_text: &str,
    images: &[ImageAnchor],
    char_offset: i64,
) -> u32 {
    if char_offset <= 0 {
        return 0;
    }
    let mut logical: i64 = 0;
    let mut images_consumed = 0usize;
    for (b, _) in plain_text.char_indices() {
        while images_consumed < images.len()
            && images[images_consumed].byte_offset <= b as u32
        {
            if logical == char_offset {
                return b as u32;
            }
            logical += 1;
            images_consumed += 1;
        }
        if logical == char_offset {
            return b as u32;
        }
        logical += 1;
    }
    let plain_len = plain_text.len() as u32;
    while images_consumed < images.len() {
        if logical == char_offset {
            return plain_len;
        }
        logical += 1;
        images_consumed += 1;
    }
    plain_len
}

/// Split a block's format runs at `byte_offset`. The returned right-hand
/// vector has its run offsets re-based so they start at byte 0 of the
/// new (right) block. Straddling runs are split with their `format`
/// cloned to both halves.
pub fn split_runs_at(
    runs: &[FormatRun],
    byte_offset: u32,
) -> (Vec<FormatRun>, Vec<FormatRun>) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    for run in runs {
        if run.byte_end <= byte_offset {
            left.push(run.clone());
        } else if run.byte_start >= byte_offset {
            right.push(FormatRun {
                byte_start: run.byte_start - byte_offset,
                byte_end: run.byte_end - byte_offset,
                format: run.format.clone(),
            });
        } else {
            left.push(FormatRun {
                byte_start: run.byte_start,
                byte_end: byte_offset,
                format: run.format.clone(),
            });
            right.push(FormatRun {
                byte_start: 0,
                byte_end: run.byte_end - byte_offset,
                format: run.format.clone(),
            });
        }
    }
    (left, right)
}

/// Split block image anchors at `byte_offset`. Anchors at exactly
/// `byte_offset` go to the right half (rebased to offset 0).
pub fn split_images_at(
    images: &[ImageAnchor],
    byte_offset: u32,
) -> (Vec<ImageAnchor>, Vec<ImageAnchor>) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    for img in images {
        if img.byte_offset < byte_offset {
            left.push(img.clone());
        } else {
            let mut new = img.clone();
            new.byte_offset -= byte_offset;
            right.push(new);
        }
    }
    (left, right)
}

// ─────────────────────────────────────────────────────────────────────
// Bridge helpers between the legacy InlineElement model and the new
// ─────────────────────────────────────────────────────────────────────

/// Copy the `fmt_*` fields of an `InlineSegment` into a `CharacterFormat`.
pub fn character_format_from_segment(seg: &InlineSegment) -> CharacterFormat {
    CharacterFormat {
        font_family: seg.fmt_font_family.clone(),
        font_point_size: seg.fmt_font_point_size,
        font_weight: seg.fmt_font_weight,
        font_bold: seg.fmt_font_bold,
        font_italic: seg.fmt_font_italic,
        font_underline: seg.fmt_font_underline,
        font_overline: seg.fmt_font_overline,
        font_strikeout: seg.fmt_font_strikeout,
        letter_spacing: seg.fmt_letter_spacing,
        word_spacing: seg.fmt_word_spacing,
        anchor_href: seg.fmt_anchor_href.clone(),
        anchor_names: seg.fmt_anchor_names.clone(),
        is_anchor: seg.fmt_is_anchor,
        tooltip: seg.fmt_tooltip.clone(),
        underline_style: seg.fmt_underline_style.clone(),
        vertical_alignment: seg.fmt_vertical_alignment.clone(),
    }
}

/// Apply a `CharacterFormat` onto an `InlineSegment`'s fmt_* fields.
pub fn apply_character_format_to_segment(seg: &mut InlineSegment, fmt: &CharacterFormat) {
    seg.fmt_font_family = fmt.font_family.clone();
    seg.fmt_font_point_size = fmt.font_point_size;
    seg.fmt_font_weight = fmt.font_weight;
    seg.fmt_font_bold = fmt.font_bold;
    seg.fmt_font_italic = fmt.font_italic;
    seg.fmt_font_underline = fmt.font_underline;
    seg.fmt_font_overline = fmt.font_overline;
    seg.fmt_font_strikeout = fmt.font_strikeout;
    seg.fmt_letter_spacing = fmt.letter_spacing;
    seg.fmt_word_spacing = fmt.word_spacing;
    seg.fmt_anchor_href = fmt.anchor_href.clone();
    seg.fmt_anchor_names = fmt.anchor_names.clone();
    seg.fmt_is_anchor = fmt.is_anchor;
    seg.fmt_tooltip = fmt.tooltip.clone();
    seg.fmt_underline_style = fmt.underline_style.clone();
    seg.fmt_vertical_alignment = fmt.vertical_alignment.clone();
}

/// Synthesize a `Vec<InlineSegment>` view of a block from its
/// `plain_text`, `format_runs`, and `block_images`. Returns segments
/// in document order.
///
/// This is the Phase 1.14b-and-forward reader function for access to
/// per-element data without requiring the (now-deleted) inline_elements table.
pub fn inline_segments_view(
    plain_text: &str,
    runs: &[FormatRun],
    images: &[ImageAnchor],
) -> Vec<InlineSegment> {
    let mut out: Vec<InlineSegment> = Vec::new();
    let bytes = plain_text.as_bytes();

    let mut img_iter = images.iter().peekable();
    let mut cursor: u32 = 0;

    let emit_text = |out: &mut Vec<InlineSegment>,
                     bytes: &[u8],
                     start: u32,
                     end: u32,
                     fmt: CharacterFormat| {
        if start >= end {
            return;
        }
        let slice = &bytes[start as usize..end as usize];
        let s = std::str::from_utf8(slice)
            .expect("block plain_text must be valid UTF-8")
            .to_string();
        let mut seg = InlineSegment {
            content: InlineContent::Text(s),
            ..Default::default()
        };
        apply_character_format_to_segment(&mut seg, &fmt);
        out.push(seg);
    };

    let emit_image = |out: &mut Vec<InlineSegment>, anchor: &ImageAnchor| {
        let mut seg = InlineSegment {
            content: InlineContent::Image {
                name: anchor.name.clone(),
                width: anchor.width,
                height: anchor.height,
                quality: anchor.quality,
            },
            ..Default::default()
        };
        apply_character_format_to_segment(&mut seg, &anchor.format);
        out.push(seg);
    };

    for run in runs {
        while let Some(img) = img_iter.peek() {
            if img.byte_offset < run.byte_start {
                emit_text(
                    &mut out,
                    bytes,
                    cursor,
                    img.byte_offset,
                    CharacterFormat::default(),
                );
                emit_image(&mut out, img);
                cursor = img.byte_offset;
                img_iter.next();
            } else {
                break;
            }
        }

        if cursor < run.byte_start {
            emit_text(
                &mut out,
                bytes,
                cursor,
                run.byte_start,
                CharacterFormat::default(),
            );
        }

        emit_text(
            &mut out,
            bytes,
            run.byte_start,
            run.byte_end,
            run.format.clone(),
        );
        cursor = run.byte_end;
    }

    for img in img_iter {
        if img.byte_offset > cursor {
            emit_text(
                &mut out,
                bytes,
                cursor,
                img.byte_offset,
                CharacterFormat::default(),
            );
            cursor = img.byte_offset;
        }
        emit_image(&mut out, img);
    }

    if (cursor as usize) < bytes.len() {
        emit_text(
            &mut out,
            bytes,
            cursor,
            bytes.len() as u32,
            CharacterFormat::default(),
        );
    }

    out
}


#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: u32, e: u32, bold: bool) -> FormatRun {
        FormatRun {
            byte_start: s,
            byte_end: e,
            format: CharacterFormat {
                font_bold: Some(bold),
                ..Default::default()
            },
        }
    }

    #[test]
    fn empty_runs_are_well_formed() {
        debug_assert_well_formed(&[], 0);
        debug_assert_well_formed(&[], 100);
    }

    #[test]
    fn coalesce_merges_adjacent_equal_runs() {
        let mut rs = vec![run(0, 5, true), run(5, 10, true), run(10, 15, false)];
        coalesce_in_place(&mut rs);
        assert_eq!(rs.len(), 2);
        assert_eq!(rs[0].byte_end, 10);
    }

    #[test]
    fn coalesce_leaves_disjoint_runs_alone() {
        let mut rs = vec![run(0, 5, true), run(7, 10, true)];
        coalesce_in_place(&mut rs);
        assert_eq!(rs.len(), 2);
    }

    #[test]
    fn splice_range_clips_straddling_runs() {
        let mut rs = vec![run(0, 20, true)];
        splice_range(&mut rs, 5..15, vec![run(5, 15, false)]);
        assert_eq!(rs.len(), 3);
        assert_eq!(rs[0].byte_end, 5);
        assert_eq!(rs[1].format.font_bold, Some(false));
        assert_eq!(rs[2].byte_start, 15);
    }

    #[test]
    fn splice_range_empty_replacement_removes_inner_runs() {
        let mut rs = vec![run(0, 5, true), run(5, 10, false), run(10, 15, true)];
        splice_range(&mut rs, 5..10, vec![]);
        // 0..5 bold, then 10..15 bold — after coalesce these are NOT adjacent
        // (there's a gap from 5..10 in the run table, meaning "no format").
        assert_eq!(rs.len(), 2);
        assert_eq!(rs[0].byte_end, 5);
        assert_eq!(rs[1].byte_start, 10);
    }

    #[test]
    fn shift_after_moves_downstream() {
        let mut rs = vec![run(0, 5, true), run(10, 15, false)];
        shift_after(&mut rs, 5, 3);
        assert_eq!(rs[0].byte_start, 0); // unchanged
        assert_eq!(rs[1].byte_start, 13);
        assert_eq!(rs[1].byte_end, 18);
    }

}
