//! Regression guards for the per-keystroke cost of editing large
//! rope-clean documents.
//!
//! Three independent O(N) costs used to ride on every keystroke in a
//! 1000-block document:
//!   1. `BlockOffsetIndex::shift_after` scanned ALL entries even when
//!      the threshold was past the end (fixed: `partition_point`).
//!   2. The snapshot taken for undo memcpy'd the whole entries Vec
//!      (fixed: `Arc<Vec>` + copy-on-write — the clone is skipped when
//!      no entry actually shifts).
//!   3. `insert_text_uc` / `delete_text_uc` rewrote
//!      `Block.document_position` on every block after the cursor
//!      (fixed: gated behind `rope_positions_match_flow`, so rope-clean
//!      docs derive positions from the index instead).
//!
//! For inserts at the END of a document, none of those three need to
//! touch any trailing entry. The per-keystroke cost drops from
//! linear-in-block-count to a much smaller residual (rope-size log
//! factors, the im::HashMap marker-index lookups, and the per-edit
//! UoW commit/snapshot constants). That large reduction is the signal
//! this test guards: a 10x larger document must cost only modestly
//! more per end-insert (~4x in practice), NOT the ~8-10x it cost when
//! a per-block position-refresh walk rode on every keystroke.
//!
//! (Insert-at-START is intrinsically O(N) — every entry's byte offset
//! genuinely shifts — so it is deliberately not used as the guard;
//! only a Fenwick/segment-tree rewrite of `shift_after` could make it
//! sub-linear. The `cursor` is created once outside the timed loop
//! because `cursor_at` triggers an O(N) `get_document_stats`
//! word-count via grapheme snapping.)

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

/// Measure `n_inserts` single-char insertions at the END of the
/// document. A single cursor is created once (outside the timed loop)
/// and reused — `cursor_at` itself triggers an O(N) `get_document_stats`
/// word-count via grapheme snapping, which would otherwise dominate
/// the measurement and mask the insert cost we care about. The cursor
/// auto-advances to the new end after each insert.
fn time_inserts_at_end(paragraphs: usize, n_inserts: usize) -> Duration {
    let doc = make_doc(paragraphs);
    let end = doc.to_plain_text().unwrap().chars().count();
    let cursor = doc.cursor_at(end);
    // Warm-up insert.
    cursor.insert_text("X").unwrap();

    let start = Instant::now();
    for _ in 0..n_inserts {
        cursor.insert_text(black_box("X")).unwrap();
    }
    let elapsed = start.elapsed();
    black_box(&doc);
    elapsed
}

/// Inserting one char at the end of a 10x larger rope-clean document
/// must cost only modestly more per keystroke (~4x in practice from
/// rope-size log factors and commit overhead), NOT the ~8-10x it cost
/// when a per-block position-refresh walk rode on every keystroke. A
/// ratio past 6x means an O(N) walk has crept back into the end-insert
/// path — check that shift_after still short-circuits when no entries
/// shift, that the snapshot Arc<Vec> clone is skipped on no-op shifts,
/// and that the insert_text_uc / delete_text_uc position-refresh loops
/// are still gated behind rope_positions_match_flow.
#[test]
fn insert_at_end_scaling_is_sub_linear() {
    const N_INSERTS: usize = 200;
    // Warm both sizes once to amortize first-touch allocation.
    let _ = time_inserts_at_end(100, 20);
    let _ = time_inserts_at_end(1000, 20);

    let t_small = time_inserts_at_end(100, N_INSERTS);
    let t_large = time_inserts_at_end(1000, N_INSERTS);

    let ratio = t_large.as_nanos() as f64 / t_small.as_nanos().max(1) as f64;
    assert!(
        ratio < 6.0,
        "End-insert into a 10x larger document took {:.1}x longer \
         ({:?} for 1000 paragraphs vs {:?} for 100). Expected ~4x \
         (rope-size log factors + commit overhead). A ratio past 6x \
         means an O(N) walk has returned to the end-insert path — \
         most likely a re-introduced position-refresh loop, an \
         un-gated entries scan, or a snapshot that deep-clones the \
         entries Vec.",
        ratio,
        t_large,
        t_small,
    );
}
