//! Regression guard for the position-refresh O(N) loop's removal.
//!
//! Inserting at position 0 of a 1000-block document used to do TWO
//! independent O(N) walks per keystroke: the position-refresh loop in
//! `execute_insert_simple` (which rewrote `Block.document_position`
//! on every subsequent block) plus `shift_after` (which scanned all
//! entries even when threshold was past the end). With both fixed:
//!
//! - `shift_after` is now O(log n + k) via `partition_point` — for
//!   inserts at the end, k=0; for inserts at the start, k=N
//!   (unavoidable: those entries genuinely need to shift).
//! - `Block.document_position` is no longer maintained per-keystroke
//!   in rope-clean documents — readers derive from `BlockOffsetIndex`.
//!
//! The remaining per-keystroke O(N) cost at *start of doc* is
//! `shift_after` itself (k=N unavoidable) plus the per-edit snapshot
//! clone of `BlockOffsetIndex.entries`. So inserts-at-start stay
//! linear, but with one fewer O(N) constant-factor walk.
//!
//! This test guards the constant-factor improvement: pre-fix the
//! ratio was ~8×; post-fix it should be ~6× or better. A regression
//! that re-introduces a per-keystroke O(N) walk would push it back.

use std::hint::black_box;
use std::time::{Duration, Instant};
use text_document::TextDocument;

const PARAGRAPH: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
     Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.";

fn make_doc(paragraphs: usize) -> TextDocument {
    let text: String = (0..paragraphs)
        .map(|_| PARAGRAPH)
        .collect::<Vec<_>>()
        .join("\n");
    let doc = TextDocument::new();
    doc.set_plain_text(&text).unwrap();
    doc
}

/// Measure `n_inserts` single-char insertions at position 0. Each
/// insertion grows the document by one char, which is negligible
/// compared to the paragraph count we vary.
fn time_inserts_at_start(paragraphs: usize, n_inserts: usize) -> Duration {
    let doc = make_doc(paragraphs);
    // Warm-up insert (also amortizes lazy initialization).
    let cursor = doc.cursor_at(0);
    cursor.insert_text("X").unwrap();

    let start = Instant::now();
    for _ in 0..n_inserts {
        let cursor = doc.cursor_at(0);
        cursor.insert_text(black_box("X")).unwrap();
    }
    let elapsed = start.elapsed();
    black_box(&doc);
    elapsed
}

/// Inserting one char at position 0 of a 10× larger doc costs only
/// the unavoidable O(k) for shifting entries past the cursor plus the
/// snapshot clone. Pre-fix this also included a separate position-
/// refresh walk through every subsequent block — making the ratio
/// 8× and growing. Post-fix the ratio sits around 6×.
#[test]
fn insert_at_start_scaling_within_acceptable_bound() {
    const N_INSERTS: usize = 30;
    let t_small = time_inserts_at_start(100, N_INSERTS);
    let t_large = time_inserts_at_start(1000, N_INSERTS);

    let ratio = t_large.as_nanos() as f64 / t_small.as_nanos().max(1) as f64;
    assert!(
        ratio < 7.5,
        "Inserting one char at position 0 of a 10× larger document \
         took {:.1}× longer ({:?} for 1000 paragraphs vs {:?} for \
         100). The expected post-fix ratio is ~6× (purely shift_after \
         + snapshot clone); a ratio past 7.5× suggests an O(N) walk \
         has crept back into the keystroke path — most likely a \
         re-introduced position-refresh loop in insert_text_uc, \
         delete_text_uc, or insert_fragment_uc.",
        ratio,
        t_large,
        t_small,
    );
}
