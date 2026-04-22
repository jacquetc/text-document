//! Grapheme-aware cursor navigation and deletion.
//!
//! Cursor *positions* stay scalar-indexed (so `cursor_at(1)` on
//! "e\u{0301}" still lands between the `e` and the combining mark —
//! callers that pass explicit positions get them) but the
//! *navigation* operations `NextCharacter` / `PreviousCharacter`
//! advance by full extended grapheme clusters, and Backspace /
//! Delete remove the whole cluster the user perceives as one
//! character.

use text_document::{MoveMode, MoveOperation, TextDocument};

fn new_doc(text: &str) -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text(text).unwrap();
    doc
}

// ── Combining marks ─────────────────────────────────────────────────

#[test]
fn arrow_right_skips_combining_acute() {
    // "e\u{0301}X" is 3 scalars forming 2 grapheme clusters ("é", "X").
    let doc = new_doc("e\u{0301}X");
    let c = doc.cursor_at(0);
    c.move_position(MoveOperation::NextCharacter, MoveMode::MoveAnchor, 1);
    assert_eq!(
        c.position(),
        2,
        "arrow-right from 0 must land past 'é' (pos 2), not inside it (pos 1)"
    );
    c.move_position(MoveOperation::NextCharacter, MoveMode::MoveAnchor, 1);
    assert_eq!(c.position(), 3, "second arrow-right must land past 'X'");
}

#[test]
fn arrow_left_skips_combining_acute() {
    let doc = new_doc("e\u{0301}X");
    let c = doc.cursor_at(3);
    c.move_position(MoveOperation::PreviousCharacter, MoveMode::MoveAnchor, 1);
    assert_eq!(c.position(), 2, "left from 3 lands before 'X'");
    c.move_position(MoveOperation::PreviousCharacter, MoveMode::MoveAnchor, 1);
    assert_eq!(
        c.position(),
        0,
        "left from 2 skips the whole 'é' cluster back to 0, not to 1"
    );
}

#[test]
fn backspace_removes_combining_acute_cluster() {
    let doc = new_doc("e\u{0301}X");
    let c = doc.cursor_at(2);
    c.delete_previous_char().unwrap();
    assert_eq!(
        doc.to_plain_text().unwrap(),
        "X",
        "backspace must remove both scalars of 'é'"
    );
}

#[test]
fn delete_forward_removes_combining_acute_cluster() {
    let doc = new_doc("e\u{0301}X");
    let c = doc.cursor_at(0);
    c.delete_char().unwrap();
    assert_eq!(
        doc.to_plain_text().unwrap(),
        "X",
        "delete must remove the whole 'é' cluster"
    );
}

// ── ZWJ emoji ───────────────────────────────────────────────────────

#[test]
fn arrow_right_skips_zwj_family() {
    // 👨‍👩‍👧‍👦 is 7 scalars, 1 grapheme cluster.
    let fam = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
    let doc = new_doc(fam);
    let c = doc.cursor_at(0);
    c.move_position(MoveOperation::NextCharacter, MoveMode::MoveAnchor, 1);
    assert_eq!(
        c.position(),
        7,
        "single arrow-right must clear the entire ZWJ family cluster"
    );
}

#[test]
fn backspace_removes_zwj_family_in_one_press() {
    let fam = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
    let doc = new_doc(fam);
    let c = doc.cursor_at(7);
    c.delete_previous_char().unwrap();
    assert_eq!(
        doc.to_plain_text().unwrap(),
        "",
        "backspace once on a ZWJ family must wipe it out, not leave a dismembered 👨‍👩‍👧"
    );
}

// ── Skin-tone modifiers ─────────────────────────────────────────────

#[test]
fn backspace_removes_waving_hand_with_skin_tone() {
    // 👋🏻 = U+1F44B U+1F3FB (waving hand + light skin tone).
    let doc = new_doc("\u{1F44B}\u{1F3FB}");
    let c = doc.cursor_at(2);
    c.delete_previous_char().unwrap();
    assert_eq!(
        doc.to_plain_text().unwrap(),
        "",
        "backspace must remove both base emoji and its skin-tone modifier"
    );
}

// ── Regional indicator flags ────────────────────────────────────────

#[test]
fn arrow_right_skips_flag_emoji() {
    // 🇫🇷 = U+1F1EB U+1F1F7 (two regional indicators).
    let doc = new_doc("\u{1F1EB}\u{1F1F7}");
    let c = doc.cursor_at(0);
    c.move_position(MoveOperation::NextCharacter, MoveMode::MoveAnchor, 1);
    assert_eq!(
        c.position(),
        2,
        "flag cluster is a single arrow-right, not two"
    );
}

#[test]
fn backspace_removes_flag_cluster() {
    let doc = new_doc("\u{1F1EB}\u{1F1F7}");
    let c = doc.cursor_at(2);
    c.delete_previous_char().unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "");
}

// ── ASCII regression ────────────────────────────────────────────────
// Grapheme snapping must not change behaviour for plain ASCII text
// where every scalar is its own cluster.

#[test]
fn ascii_navigation_unchanged() {
    let doc = new_doc("abcde");
    let c = doc.cursor_at(2);
    c.move_position(MoveOperation::NextCharacter, MoveMode::MoveAnchor, 1);
    assert_eq!(c.position(), 3);
    c.move_position(MoveOperation::PreviousCharacter, MoveMode::MoveAnchor, 2);
    assert_eq!(c.position(), 1);
}

#[test]
fn ascii_backspace_unchanged() {
    let doc = new_doc("abcde");
    let c = doc.cursor_at(3);
    c.delete_previous_char().unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "abde");
}

// ── Block-boundary crossing ─────────────────────────────────────────

#[test]
fn arrow_right_across_block_boundary_still_one_step() {
    // "AB\nCD": NextCharacter from end of block 0 (pos 2) to start of
    // block 1 (pos 3) — the separator is a single scalar, never part
    // of a grapheme cluster, so movement must be one step.
    let doc = new_doc("AB\nCD");
    let c = doc.cursor_at(2);
    c.move_position(MoveOperation::NextCharacter, MoveMode::MoveAnchor, 1);
    assert_eq!(c.position(), 3, "block separator is one scalar");
    c.move_position(MoveOperation::NextCharacter, MoveMode::MoveAnchor, 1);
    assert_eq!(c.position(), 4, "second step enters block 1's 'C'");
}

#[test]
fn backspace_across_block_boundary_merges_blocks() {
    // Backspace at start of block 1 should merge the two blocks, not
    // run grapheme analysis across the boundary.
    let doc = new_doc("AB\nCD");
    let c = doc.cursor_at(3);
    c.delete_previous_char().unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "ABCD");
    assert_eq!(doc.block_count(), 1);
}
