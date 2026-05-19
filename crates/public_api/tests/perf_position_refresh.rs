//! Reproduction test for the position-refresh O(N) loop in
//! `execute_insert_simple` (and twins in `delete_text_uc` /
//! `insert_fragment_uc`). On every single-character insert that
//! lands inside a block, the use case currently walks every block
//! sitting after the cursor and rewrites its `Block.document_position`
//! through `update_block_multi`. Inserting at position 0 of a
//! 1000-block document therefore touches 999 blocks per keystroke.
//!
//! Strategy: time a single-character insert at the *start* of two
//! documents that differ only in block count (100 vs 1000 paragraphs).
//! If the per-insert cost is genuinely O(1) in block count, the
//! ratio stays near 1; the current implementation produces a ratio
//! that scales with the size ratio.
//!
//! This test fails on current `main` and is expected to pass once
//! the position-refresh loops are removed in Phase 3.

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

/// Inserting a single character at position 0 should be O(1) in the
/// number of blocks. The current position-refresh loop in
/// `execute_insert_simple` makes it O(N), so a 10× larger document
/// produces ~10× the per-insert cost.
#[test]
#[ignore = "reproduction; fails until Phase 3 removes the position-refresh loop"]
fn insert_at_start_does_not_scale_with_block_count() {
    const N_INSERTS: usize = 30;
    let t_small = time_inserts_at_start(100, N_INSERTS);
    let t_large = time_inserts_at_start(1000, N_INSERTS);

    let ratio = t_large.as_nanos() as f64 / t_small.as_nanos().max(1) as f64;
    assert!(
        ratio < 5.0,
        "Inserting one char at position 0 of a 10× larger document \
         took {:.1}× longer ({:?} for 1000 paragraphs vs {:?} for \
         100). This is the position-refresh O(N) loop \
         (insert_text_uc.rs:397-420) shifting every subsequent \
         block's document_position. After Phase 3 migrates readers \
         to BlockOffsetIndex and the loop is deleted, the ratio \
         should approach 1.",
        ratio,
        t_large,
        t_small,
    );
}
