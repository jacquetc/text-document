//! Phase 2 step 5.6: verify document_search use cases mirror their
//! mutations to the global rope under `rope_backend`. Skipped under
//! the default backend.

#![cfg(feature = "rope_backend")]

extern crate text_document_search as document_search;

use anyhow::Result;
use document_search::document_search_controller;
use document_search::ReplaceTextDto;
use test_harness::setup_with_imported_text;

/// `replace_text` mutates `block.plain_text`; under rope_backend it
/// must also mirror the splice into the rope so the two stay in sync.
#[test]
fn replace_text_mirrors_to_rope() -> Result<()> {
    let (db_context, event_hub, mut urm) = setup_with_imported_text("hello world hello")?;

    let store = db_context.get_store();
    assert_eq!(store.rope.read().unwrap().to_string(), "hello world hello");

    document_search_controller::replace_text(
        &db_context,
        &event_hub,
        &mut urm,
        None,
        &ReplaceTextDto {
            query: "hello".to_string(),
            replacement: "HEY".to_string(),
            case_sensitive: true,
            whole_word: false,
            use_regex: false,
            replace_all: true,
        },
    )?;

    // Rope reflects both replacements.
    assert_eq!(store.rope.read().unwrap().to_string(), "HEY world HEY");

    Ok(())
}

/// 5.6c: find/replace under rope_backend reads from the rope (via
/// `build_full_text_via_store`). To exercise this code path explicitly,
/// the test mutates the rope content directly (out-of-band with
/// `Block.plain_text`) and verifies the search reflects the rope —
/// not the now-stale plain_text.
#[test]
fn replace_text_reads_from_rope_under_rope_backend() -> Result<()> {
    let (db_context, event_hub, mut urm) = setup_with_imported_text("hello world")?;

    // Rewrite the rope directly: change "world" to "WORLD" without
    // touching Block.plain_text. (Direct rope mutation is a test-only
    // hack to prove the search reads from the rope.)
    let store = db_context.get_store();
    {
        let mut rope = store.rope.write().unwrap();
        let char_start = rope.byte_to_char(6);
        let char_end = rope.byte_to_char(11);
        rope.remove(char_start..char_end);
        rope.insert(char_start, "WORLD");
    }
    assert_eq!(store.rope.read().unwrap().to_string(), "hello WORLD");

    // Search for "WORLD" — should find it (proves read from rope).
    let result = document_search_controller::replace_text(
        &db_context,
        &event_hub,
        &mut urm,
        None,
        &ReplaceTextDto {
            query: "WORLD".to_string(),
            replacement: "earth".to_string(),
            case_sensitive: true,
            whole_word: false,
            use_regex: false,
            replace_all: true,
        },
    )?;

    assert_eq!(result.replacements_count, 1);
    // After replace, both block.plain_text and rope hold the new value.
    assert_eq!(store.rope.read().unwrap().to_string(), "hello earth");

    Ok(())
}

/// Replace inside a multi-block doc must update the rope across the
/// affected block's byte range without disturbing neighbors.
#[test]
fn replace_text_in_one_block_of_many_preserves_other_blocks() -> Result<()> {
    let (db_context, event_hub, mut urm) = setup_with_imported_text("first\nfind me\nthird")?;

    let store = db_context.get_store();
    assert_eq!(
        store.rope.read().unwrap().to_string(),
        "first\nfind me\nthird"
    );

    document_search_controller::replace_text(
        &db_context,
        &event_hub,
        &mut urm,
        None,
        &ReplaceTextDto {
            query: "find me".to_string(),
            replacement: "FOUND".to_string(),
            case_sensitive: true,
            whole_word: false,
            use_regex: false,
            replace_all: true,
        },
    )?;

    assert_eq!(
        store.rope.read().unwrap().to_string(),
        "first\nFOUND\nthird"
    );

    Ok(())
}
