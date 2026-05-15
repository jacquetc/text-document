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
use crate::types::EntityId;
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

impl ImageAnchor {
    /// Resolve the image to its `Resource` entity by name lookup. Returns
    /// `None` if no matching resource exists (the caller must handle the
    /// missing-image case, same as today's broken-image semantics).
    pub fn resolve<'a, I>(&self, mut resources: I) -> Option<EntityId>
    where
        I: Iterator<Item = (EntityId, &'a str)>,
    {
        resources
            .find(|(_, name)| *name == self.name)
            .map(|(id, _)| id)
    }
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
