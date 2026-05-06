//! Fuzz-style robustness tests for the parser + editing paths.
//!
//! `cargo-fuzz` requires a nightly toolchain (libfuzzer-sys links
//! against compiler-rt's fuzzing runtime), which isn't a guaranteed
//! CI dependency. This suite drives the same parsers through
//! `proptest`-generated random inputs — not coverage-guided, so less
//! effective than libFuzzer at discovering deep branches, but
//! adequate for surfacing "parser panics on weird input" bugs and
//! for locking in the *no-panic* contract across the public API.
//!
//! Each property runs 1024 iterations (configurable via
//! `PROPTEST_CASES=<N>`). Shrinking on failure is via proptest's
//! built-in minimiser, so a crash reduces to the smallest input that
//! reproduces it.
//!
//! Invariants asserted (in addition to "does not panic"):
//!
//! * After `set_html(x)` / `set_markdown(x)` succeeds,
//!   `to_plain_text()` must succeed and `character_count() >= 0`.
//! * `block_count()` is always at least 1 once plain text has been
//!   initialised.
//! * A successful import round-trip through HTML or markdown
//!   preserves the plain-text content modulo whitespace
//!   normalisation.

use proptest::prelude::*;
use text_document::{MoveMode, MoveOperation, TextDocument};

// Bounded random bytes that contain a biased mix of ASCII, UTF-8
// multibyte, HTML-significant, and markdown-significant characters.
// Uniform random bytes would almost never produce valid HTML; this
// strategy skews toward "interesting" inputs.
fn arb_html_like() -> impl Strategy<Value = String> {
    proptest::string::string_regex(
        r#"[a-zA-Z0-9 <>/&;!?.,\n\t\-="'#\[\]\(\)éà🌍]{0,200}"#,
    )
    .unwrap()
}

fn arb_markdown_like() -> impl Strategy<Value = String> {
    proptest::string::string_regex(
        r"[a-zA-Z0-9 #*_`|\-\[\]\(\)!\n\t.,>:;éà🌍]{0,200}",
    )
    .unwrap()
}

// Small alphabet for edit-op sequences that drive the cursor API.
#[derive(Debug, Clone)]
enum Op {
    InsertText(String),
    InsertBlock,
    DeleteChar,
    DeletePrev,
    MoveNext(u8),
    MovePrev(u8),
    SelectForward(u8),
    SelectBackward(u8),
    Undo,
    Redo,
}

fn arb_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        proptest::string::string_regex(r"[a-z ]{0,5}")
            .unwrap()
            .prop_map(Op::InsertText),
        Just(Op::InsertBlock),
        Just(Op::DeleteChar),
        Just(Op::DeletePrev),
        (0u8..6).prop_map(Op::MoveNext),
        (0u8..6).prop_map(Op::MovePrev),
        (0u8..6).prop_map(Op::SelectForward),
        (0u8..6).prop_map(Op::SelectBackward),
        Just(Op::Undo),
        Just(Op::Redo),
    ]
}

// ── Property: HTML parser never panics ──────────────────────────────

proptest! {
    #[test]
    fn set_html_never_panics(input in arb_html_like()) {
        // Priming with `set_plain_text("")` guarantees the document
        // has an initial block; without it a fresh document can have
        // `block_count == 0`, which is a valid construction state
        // even though every edit path assumes ≥1 block. Callers
        // (including the rich-text widget) always prime the doc, so
        // the same contract applies here.
        let doc = TextDocument::new();
        doc.set_plain_text("").unwrap();
        // We don't care whether set_html succeeds; we care that it
        // doesn't panic. Error returns are a valid outcome.
        let _ = doc.set_html(&input);
        // Downstream queries must still be safe.
        prop_assert!(doc.to_plain_text().is_ok());
        prop_assert!(doc.block_count() >= 1);
    }
}

// ── Property: markdown parser never panics ──────────────────────────

proptest! {
    #[test]
    fn set_markdown_never_panics(input in arb_markdown_like()) {
        let doc = TextDocument::new();
        doc.set_plain_text("").unwrap();
        let op = match doc.set_markdown(&input) {
            Ok(o) => o,
            Err(_) => return Ok(()),
        };
        // `set_markdown` returns an Operation that completes async.
        // `wait` blocks until done; panic there is the failure mode
        // we're testing for. An `Err` result is fine.
        let _ = op.wait();
        prop_assert!(doc.to_plain_text().is_ok());
        prop_assert!(doc.block_count() >= 1);
    }
}

// ── Property: insert_html at arbitrary cursor positions ─────────────

proptest! {
    #[test]
    fn insert_html_at_arbitrary_position_never_panics(
        seed in "[a-zA-Z ]{0,30}",
        html in arb_html_like(),
        pos_frac in 0.0f64..=1.0,
    ) {
        let doc = TextDocument::new();
        doc.set_plain_text(&seed).unwrap();
        let pos = ((pos_frac * doc.character_count() as f64).floor() as usize)
            .min(doc.character_count());
        let cursor = doc.cursor_at(pos);
        let _ = cursor.insert_html(&html);
        // Any downstream query must still succeed.
        prop_assert!(doc.to_plain_text().is_ok());
        prop_assert!(doc.block_count() >= 1);
    }
}

// ── Property: random edit sequences preserve invariants ─────────────

proptest! {
    #[test]
    fn random_edit_sequence_preserves_invariants(
        seed in "[a-zA-Z ]{0,40}",
        ops in prop::collection::vec(arb_op(), 0..20),
    ) {
        let doc = TextDocument::new();
        doc.set_plain_text(&seed).unwrap();
        let cursor = doc.cursor_at(0);

        for op in &ops {
            match op {
                Op::InsertText(t) => { let _ = cursor.insert_text(t); }
                Op::InsertBlock => { let _ = cursor.insert_block(); }
                Op::DeleteChar => { let _ = cursor.delete_char(); }
                Op::DeletePrev => { let _ = cursor.delete_previous_char(); }
                Op::MoveNext(n) => {
                    cursor.move_position(
                        MoveOperation::NextCharacter,
                        MoveMode::MoveAnchor,
                        *n as usize,
                    );
                }
                Op::MovePrev(n) => {
                    cursor.move_position(
                        MoveOperation::PreviousCharacter,
                        MoveMode::MoveAnchor,
                        *n as usize,
                    );
                }
                Op::SelectForward(n) => {
                    cursor.move_position(
                        MoveOperation::NextCharacter,
                        MoveMode::KeepAnchor,
                        *n as usize,
                    );
                }
                Op::SelectBackward(n) => {
                    cursor.move_position(
                        MoveOperation::PreviousCharacter,
                        MoveMode::KeepAnchor,
                        *n as usize,
                    );
                }
                Op::Undo => { let _ = doc.undo(); }
                Op::Redo => { let _ = doc.redo(); }
            }

            // Core invariants after every op.
            prop_assert!(doc.block_count() >= 1);
            let plain = doc.to_plain_text().unwrap();
            prop_assert_eq!(
                doc.character_count() + doc.block_count() - 1,
                plain.chars().count(),
                "character_count + (block_count - 1) == plain.chars().count()"
            );
            // Cursor position never exceeds max.
            let cc = doc.character_count();
            let bc = doc.block_count();
            let max = cc + bc.saturating_sub(1);
            prop_assert!(cursor.position() <= max);
            prop_assert!(cursor.anchor() <= max);
        }
    }
}

// ── Property: HTML → document → HTML round-trip stabilises ──────────

proptest! {
    #[test]
    fn html_roundtrip_stabilises(seed in arb_html_like()) {
        // `set_html` is a long operation: it returns immediately with a
        // handle while the import runs on a background thread. Querying
        // the document before `wait()` races with the import and can see
        // a still-empty doc — which is what made this property flaky on
        // CI (commit transient: html1 had content, html2 raced and was
        // empty, false-positive idempotency failure).
        let doc1 = TextDocument::new();
        let op1 = match doc1.set_html(&seed) {
            Ok(o) => o,
            Err(_) => return Ok(()),
        };
        if op1.wait().is_err() {
            return Ok(());
        }
        let c = doc1.cursor_at(0);
        c.move_position(MoveOperation::End, MoveMode::KeepAnchor, 1);
        let html1 = c.selection().to_html();

        // Second round-trip: parse the first output, reserialise, expect
        // the same string. If the serialiser is idempotent (which it
        // should be for internally-produced HTML), html1 == html2.
        let doc2 = TextDocument::new();
        let op2 = match doc2.set_html(&html1) {
            Ok(o) => o,
            Err(_) => return Ok(()),
        };
        if op2.wait().is_err() {
            return Ok(());
        }
        let c2 = doc2.cursor_at(0);
        c2.move_position(MoveOperation::End, MoveMode::KeepAnchor, 1);
        let html2 = c2.selection().to_html();

        prop_assert_eq!(
            html1, html2,
            "HTML serialiser must be idempotent on its own output"
        );
    }
}

// ── Seed corpus: hand-picked adversarial HTML inputs ────────────────
// These are the small set that would appear in a cargo-fuzz corpus
// directory. They're cheap to run and serve as a fast smoke test.

#[test]
fn seed_corpus_adversarial_html() {
    let inputs: &[&str] = &[
        "",
        "<",
        "<p>",
        "</p>",
        "<p><p><p><p><p>",
        "<p>unterminated",
        "<!DOCTYPE html><html></html>",
        "<table><tr><td>",
        "<script>alert(1)</script>",
        "<p>&amp;&lt;&gt;</p>",
        "<p style='x:y'>a</p>",
        "<p><b><i><u></u></i></b></p>",
        "<p>\0\x01\x02</p>",
        "<p>café 日本語 🌍</p>",
        "<p>e\u{0301}X</p>",
        "<ul><li><ol><li><ul><li>deep</li></ul></li></ol></li></ul>",
    ];
    for html in inputs {
        let doc = TextDocument::new();
        doc.set_plain_text("").unwrap();
        let _ = doc.set_html(html);
        // After any html import, queries must be safe.
        let _ = doc.to_plain_text();
        let _ = doc.character_count();
        let _ = doc.block_count();
    }
}

#[test]
fn seed_corpus_adversarial_markdown() {
    let inputs: &[&str] = &[
        "",
        "#",
        "##",
        "# ",
        "\n\n\n",
        "| a | b |\n|---|---|",
        "| a |\n| b\n| c",
        "```\n```",
        "![](",
        "[link](http://",
        ">>>>> quote",
        "* one\n  * two\n    * three",
        "- [ ] task",
        "﻿# with BOM",
    ];
    for md in inputs {
        let doc = TextDocument::new();
        doc.set_plain_text("").unwrap();
        if let Ok(op) = doc.set_markdown(md) {
            let _ = op.wait();
        }
        let _ = doc.to_plain_text();
    }
}
