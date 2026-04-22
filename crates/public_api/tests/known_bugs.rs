//! Known-bug regression tests.
//!
//! Each test here demonstrates a concrete bug with a minimal
//! failing input. All are marked `#[ignore = "FIXME: ..."]` so they
//! don't flicker CI but remain visible to anyone running
//! `cargo test -- --ignored` or scanning the file. When the bug is
//! fixed, remove the `#[ignore]` and the test becomes a regression
//! guard.
//!
//! This file exists because the fuzz / invariant properties in
//! `fuzz_robustness_tests.rs` and `invariant_tests.rs` had to be
//! loosened to pass while these bugs remain unfixed. Rather than
//! quietly weakening the assertions, each loosened invariant gets
//! a concrete "show the bug" counterpart here.

use text_document::{MoveMode, MoveOperation, TextDocument};

// ── Fix-forward regression guard (was Bug 1) ────────────────────────
// `delete_char` + out-of-range cursor on an empty document used to
// panic via the backend returning -1 (`to_usize(-1)`). The grapheme-
// cursor refactor in commit 6594416 added defensive bounds checks
// in `next_grapheme_boundary` / `prev_grapheme_boundary` that
// eliminated this path as a side effect.
//
// Keeping this test (unignored) as a positive regression guard so
// the fix can't silently regress. It also covers the input that
// fuzz_tests.rs:335 still skips defensively — a follow-up can drop
// that `if char_count == 0 { continue }` guard and let the fuzz
// cases exercise the empty-doc path too.

#[test]
fn empty_document_out_of_range_delete_ops_are_safe() {
    let doc = TextDocument::new();
    doc.set_plain_text("").unwrap();
    // Out-of-range cursor: max_cursor_position == 0 but we ask for 1.
    let cursor = doc.cursor_at(1);
    cursor.delete_char().unwrap();
    cursor.delete_previous_char().unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "");
    assert_eq!(doc.block_count(), 1);
}

// ── Bug 2 ───────────────────────────────────────────────────────────
// `NextCharacter` then `PreviousCharacter` is not identity when the
// cursor starts mid-grapheme-cluster.
//
// For "e\u{0301}X" (e + combining acute + X):
//   start at position 1 (between e and the combining mark)
//   → NextCharacter: advances past the whole "é" cluster to pos 2
//   → PreviousCharacter: retreats by the previous cluster to pos 0
//   Final: 0, not 1.
//
// Two acceptable fixes:
//   (a) `cursor_at(n)` snaps to the nearest grapheme-cluster
//       boundary, so position 1 is never reachable — the
//       round-trip is vacuously identity.
//   (b) Cursor tracks where it came from so PreviousCharacter
//       reverses the exact distance NextCharacter just advanced.
//
// Related: `invariant_tests.rs::next_then_prev_character_is_identity`
// restricts its alphabet to ASCII to avoid this case. The loosened
// invariant is honest only because this explicit test documents
// the unhandled one.

#[test]
fn mid_cluster_next_then_prev_returns_to_start() {
    // After the cursor_at snap fix: any requested position inside a
    // grapheme cluster snaps forward to the cluster end. The
    // round-trip identity is therefore trivially satisfied — every
    // cursor starts at a boundary where `next` then `prev` returns
    // to the same place.
    let doc = TextDocument::new();
    doc.set_plain_text("e\u{0301}X").unwrap();
    let c = doc.cursor_at(1);
    let start = c.position();
    assert!(
        start == 0 || start == 2,
        "expected snap to cluster boundary, got {}",
        start
    );
    c.move_position(MoveOperation::NextCharacter, MoveMode::MoveAnchor, 1);
    c.move_position(MoveOperation::PreviousCharacter, MoveMode::MoveAnchor, 1);
    assert_eq!(
        c.position(),
        start,
        "round-trip from a snapped boundary must return to start"
    );
}

// ── Bug 3 ───────────────────────────────────────────────────────────
// After a specific multi-block edit sequence, `delete_previous_char`
// decrements `character_count` and moves the cursor back but does
// NOT actually remove the character from the plain text. Result:
// the invariant `character_count + (block_count - 1) ==
// plain.chars().count()` is violated.
//
// Minimal reproduction, from proptest:
//   doc = TextDocument::new(); set_plain_text("");
//   cursor.insert_block();                  // "\n"
//   cursor.move(PreviousCharacter, 1);      // pos=0
//   cursor.insert_text("a");                // "a\n", pos=1
//   cursor.move(NextCharacter, 1);          // pos=2
//   cursor.insert_text("a");                // "a\na", pos=3
//   cursor.delete_previous_char();          // BUG: cc=1, pos=2, plain="a\na"
//
// Expected: delete_previous_char removes the last "a" from the
// plain text, leaving cc=1, bc=2, plain="a\n".
//
// Related: `fuzz_robustness_tests.rs::random_edit_sequence_
// preserves_invariants` had to loosen the post-op invariant check
// to just "no panic" to get past this case. When this is fixed,
// the invariant assertion can be re-enabled.

#[test]
fn delete_previous_char_after_crossblock_edit_keeps_invariant() {
    use text_document::{MoveMode, MoveOperation};
    let doc = TextDocument::new();
    doc.set_plain_text("").unwrap();
    let cursor = doc.cursor_at(0);
    cursor.insert_block().unwrap();
    cursor.move_position(MoveOperation::PreviousCharacter, MoveMode::MoveAnchor, 1);
    cursor.insert_text("a").unwrap();
    cursor.move_position(MoveOperation::NextCharacter, MoveMode::MoveAnchor, 1);
    cursor.insert_text("a").unwrap();
    cursor.delete_previous_char().unwrap();

    let plain = doc.to_plain_text().unwrap();
    let cc = doc.character_count();
    let bc = doc.block_count();
    assert_eq!(
        cc + bc - 1,
        plain.chars().count(),
        "character_count + (block_count - 1) must equal plain.chars().count() \
         — got cc={}, bc={}, plain={:?}",
        cc, bc, plain
    );
}

#[test]
fn cursor_at_should_snap_to_grapheme_boundary() {
    let doc = TextDocument::new();
    doc.set_plain_text("e\u{0301}X").unwrap();
    let c = doc.cursor_at(1);
    // Option (a) of the round-trip fix: `cursor_at(1)` returns a
    // cursor whose `position()` is 0 or 2, never 1 (inside the
    // cluster). Currently position is exactly 1.
    let pos = c.position();
    assert!(
        pos == 0 || pos == 2,
        "cursor_at(1) in decomposed 'e + U+0301 + X' should snap to 0 or 2, got {}",
        pos
    );
}
