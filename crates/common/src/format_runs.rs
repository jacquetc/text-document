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

use crate::entities::{CharVerticalAlignment, InlineContent, InlineElement, UnderlineStyle};
use serde::{Deserialize, Serialize};

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
/// Bit layout (u64): top bit = synth tag (always 1, so synthesized
/// ids never collide with real entity ids issued by the store's
/// counter, which start at 1 and grow upward). Next 31 bits = block id
/// (2 billion blocks per document). Bottom 32 bits = byte offset
/// (4 GB per block).
pub fn synth_element_id(block_id: u64, byte_start: u32) -> u64 {
    const SYNTH_TAG: u64 = 0x8000_0000_0000_0000;
    SYNTH_TAG | ((block_id & 0x7FFF_FFFF) << 32) | (byte_start as u64)
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
// Bridge helpers between the legacy InlineElement model and the new
// (FormatRun, ImageAnchor) model. Used during Phase 1 to mirror writes
// from the inline_elements table into format_runs/block_images, and to
// synthesize InlineElement-shaped reads for callers not yet migrated.
// ─────────────────────────────────────────────────────────────────────

/// Copy the `fmt_*` fields of an `InlineElement` into a `CharacterFormat`.
pub fn character_format_from_inline_element(e: &InlineElement) -> CharacterFormat {
    CharacterFormat {
        font_family: e.fmt_font_family.clone(),
        font_point_size: e.fmt_font_point_size,
        font_weight: e.fmt_font_weight,
        font_bold: e.fmt_font_bold,
        font_italic: e.fmt_font_italic,
        font_underline: e.fmt_font_underline,
        font_overline: e.fmt_font_overline,
        font_strikeout: e.fmt_font_strikeout,
        letter_spacing: e.fmt_letter_spacing,
        word_spacing: e.fmt_word_spacing,
        anchor_href: e.fmt_anchor_href.clone(),
        anchor_names: e.fmt_anchor_names.clone(),
        is_anchor: e.fmt_is_anchor,
        tooltip: e.fmt_tooltip.clone(),
        underline_style: e.fmt_underline_style.clone(),
        vertical_alignment: e.fmt_vertical_alignment.clone(),
    }
}

/// Build the new (FormatRun, ImageAnchor) representation from a block's
/// inline elements in document order. Byte offsets are computed by
/// concatenating each Text element's UTF-8 length; Image and Empty
/// elements are zero-byte in the linear text but Image anchors get a
/// position equal to the running byte offset.
///
/// Adjacent Text elements with identical formatting are coalesced
/// into a single FormatRun.
pub fn format_runs_from_inline_elements(
    elements: &[InlineElement],
) -> (Vec<FormatRun>, Vec<ImageAnchor>) {
    let mut runs: Vec<FormatRun> = Vec::with_capacity(elements.len());
    let mut images: Vec<ImageAnchor> = Vec::new();
    let mut byte_offset: u32 = 0;
    for elem in elements {
        let fmt = character_format_from_inline_element(elem);
        match &elem.content {
            InlineContent::Empty => {
                // Zero-length: nothing to emit, no offset advance.
            }
            InlineContent::Text(s) => {
                let len = s.len() as u32;
                if len > 0 {
                    runs.push(FormatRun {
                        byte_start: byte_offset,
                        byte_end: byte_offset + len,
                        format: fmt,
                    });
                    byte_offset += len;
                }
            }
            InlineContent::Image {
                name,
                width,
                height,
                quality,
            } => {
                images.push(ImageAnchor {
                    byte_offset,
                    name: name.clone(),
                    width: *width,
                    height: *height,
                    quality: *quality,
                    format: fmt,
                });
                // Image is a single logical character but occupies zero
                // bytes in block.plain_text until Phase 2 (where it will
                // be a U+FFFC sentinel, 3 UTF-8 bytes).
            }
        }
    }
    coalesce_in_place(&mut runs);
    (runs, images)
}

/// Apply a `CharacterFormat` onto an `InlineElement`'s fmt_* fields.
/// Used by the reverse-bridge that synthesizes an InlineElement-shaped
/// view for not-yet-migrated readers.
pub fn apply_character_format_to_inline_element(e: &mut InlineElement, fmt: &CharacterFormat) {
    e.fmt_font_family = fmt.font_family.clone();
    e.fmt_font_point_size = fmt.font_point_size;
    e.fmt_font_weight = fmt.font_weight;
    e.fmt_font_bold = fmt.font_bold;
    e.fmt_font_italic = fmt.font_italic;
    e.fmt_font_underline = fmt.font_underline;
    e.fmt_font_overline = fmt.font_overline;
    e.fmt_font_strikeout = fmt.font_strikeout;
    e.fmt_letter_spacing = fmt.letter_spacing;
    e.fmt_word_spacing = fmt.word_spacing;
    e.fmt_anchor_href = fmt.anchor_href.clone();
    e.fmt_anchor_names = fmt.anchor_names.clone();
    e.fmt_is_anchor = fmt.is_anchor;
    e.fmt_tooltip = fmt.tooltip.clone();
    e.fmt_underline_style = fmt.underline_style.clone();
    e.fmt_vertical_alignment = fmt.vertical_alignment.clone();
}

/// Synthesize a Vec<InlineElement>-shaped view of a block from its
/// `plain_text`, `format_runs`, and `block_images`. Caller supplies
/// the block id (the synthetic InlineElement ids will be 0; callers
/// that need real ids should fetch from the inline_elements table
/// directly until Phase 1.14).
///
/// Used by read paths that have not yet been migrated to consume
/// FormatRun directly. Returns elements in document order.
pub fn inline_elements_view(
    plain_text: &str,
    runs: &[FormatRun],
    images: &[ImageAnchor],
) -> Vec<InlineElement> {
    let mut out: Vec<InlineElement> = Vec::new();
    let bytes = plain_text.as_bytes();

    // Merge runs and image anchors in byte-offset order. Image anchors
    // sit between text runs. The text between runs (with no FormatRun
    // covering it) gets a default-formatted Text element.
    let mut img_iter = images.iter().peekable();
    let mut cursor: u32 = 0;

    let emit_text =
        |out: &mut Vec<InlineElement>, bytes: &[u8], start: u32, end: u32, fmt: CharacterFormat| {
            if start >= end {
                return;
            }
            let slice = &bytes[start as usize..end as usize];
            let s = std::str::from_utf8(slice)
                .expect("block plain_text must be valid UTF-8")
                .to_string();
            let mut elem = InlineElement {
                content: InlineContent::Text(s),
                ..Default::default()
            };
            apply_character_format_to_inline_element(&mut elem, &fmt);
            out.push(elem);
        };

    let emit_image = |out: &mut Vec<InlineElement>, anchor: &ImageAnchor| {
        let mut elem = InlineElement {
            content: InlineContent::Image {
                name: anchor.name.clone(),
                width: anchor.width,
                height: anchor.height,
                quality: anchor.quality,
            },
            ..Default::default()
        };
        apply_character_format_to_inline_element(&mut elem, &anchor.format);
        out.push(elem);
    };

    for run in runs {
        // Emit any image anchors that sit strictly before this run.
        while let Some(img) = img_iter.peek() {
            if img.byte_offset < run.byte_start {
                // Emit any default-formatted text gap before the image.
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

        // Default-formatted gap between cursor and run start.
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

    // Remaining images at or after the last run.
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

    // Trailing default-formatted text after the last run/image.
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

    // ── Bridge tests: InlineElement <-> (FormatRun, ImageAnchor) ──

    fn elem_text(s: &str, bold: bool) -> InlineElement {
        InlineElement {
            content: InlineContent::Text(s.to_string()),
            fmt_font_bold: Some(bold),
            ..Default::default()
        }
    }

    fn elem_image(name: &str) -> InlineElement {
        InlineElement {
            content: InlineContent::Image {
                name: name.to_string(),
                width: 100,
                height: 50,
                quality: 90,
            },
            ..Default::default()
        }
    }

    #[test]
    fn bridge_simple_text_runs() {
        let elements = vec![elem_text("hello", false), elem_text(" world", true)];
        let (runs, imgs) = format_runs_from_inline_elements(&elements);
        assert!(imgs.is_empty());
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].byte_start, 0);
        assert_eq!(runs[0].byte_end, 5);
        assert_eq!(runs[0].format.font_bold, Some(false));
        assert_eq!(runs[1].byte_start, 5);
        assert_eq!(runs[1].byte_end, 11);
        assert_eq!(runs[1].format.font_bold, Some(true));
    }

    #[test]
    fn bridge_adjacent_same_format_coalesces() {
        let elements = vec![elem_text("foo", true), elem_text("bar", true)];
        let (runs, _) = format_runs_from_inline_elements(&elements);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].byte_end, 6);
    }

    #[test]
    fn bridge_image_anchor() {
        let elements = vec![
            elem_text("hi ", false),
            elem_image("logo.png"),
            elem_text(" there", false),
        ];
        let (runs, imgs) = format_runs_from_inline_elements(&elements);
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0].byte_offset, 3); // after "hi "
        assert_eq!(imgs[0].name, "logo.png");
        // The two text runs have the same default format -> coalesced.
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].byte_end, 9); // "hi " (3) + " there" (6)
    }

    #[test]
    fn synthesize_view_round_trips_text() {
        let plain = "hello world";
        let runs = vec![run(0, 5, false), run(5, 11, true)];
        let view = inline_elements_view(plain, &runs, &[]);
        assert_eq!(view.len(), 2);
        match (&view[0].content, &view[1].content) {
            (InlineContent::Text(a), InlineContent::Text(b)) => {
                assert_eq!(a, "hello");
                assert_eq!(b, " world");
            }
            _ => panic!("expected two text elements"),
        }
        assert_eq!(view[0].fmt_font_bold, Some(false));
        assert_eq!(view[1].fmt_font_bold, Some(true));
    }

    #[test]
    fn synthesize_view_with_image_in_middle() {
        let plain = "ab"; // image sits between bytes 1 and 2
        let runs = vec![run(0, 1, false), run(1, 2, false)];
        let imgs = vec![ImageAnchor {
            byte_offset: 1,
            name: "x.png".into(),
            width: 1,
            height: 1,
            quality: 90,
            format: CharacterFormat::default(),
        }];
        let view = inline_elements_view(plain, &runs, &imgs);
        // Expect: text "a" (default), text "a" (bold=false from run), image, text "b" (bold=false)
        // Actually with adjacent identical runs the view emits one run per FormatRun.
        // The image is anchored at byte_offset=1 which is the boundary of the two runs.
        // Expect either: text(0..1) | image | text(1..2), or text + text + image + text depending.
        let images_in_view = view
            .iter()
            .filter(|e| matches!(e.content, InlineContent::Image { .. }))
            .count();
        assert_eq!(images_in_view, 1);
        let total_text: String = view
            .iter()
            .filter_map(|e| {
                if let InlineContent::Text(t) = &e.content {
                    Some(t.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(total_text, "ab");
    }

    #[test]
    fn bridge_round_trip_preserves_text() {
        // inline_elements -> (runs, images) -> inline_elements should
        // produce the same plain text and image-list shape.
        let elements = vec![
            elem_text("Hello ", false),
            elem_text("bold", true),
            elem_text(" world", false),
            elem_image("img"),
            elem_text("!", false),
        ];
        let plain: String = elements
            .iter()
            .filter_map(|e| {
                if let InlineContent::Text(s) = &e.content {
                    Some(s.as_str())
                } else {
                    None
                }
            })
            .collect();
        let (runs, imgs) = format_runs_from_inline_elements(&elements);
        let view = inline_elements_view(&plain, &runs, &imgs);
        let view_plain: String = view
            .iter()
            .filter_map(|e| {
                if let InlineContent::Text(s) = &e.content {
                    Some(s.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(view_plain, plain);
        let view_imgs = view
            .iter()
            .filter(|e| matches!(e.content, InlineContent::Image { .. }))
            .count();
        assert_eq!(view_imgs, 1);
    }
}
