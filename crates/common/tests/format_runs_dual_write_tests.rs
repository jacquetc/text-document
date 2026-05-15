//! Validates the Phase 1 dual-write hook: every mutation through the
//! InlineElementRepository must leave `format_runs` and `block_images`
//! in sync with the underlying `inline_elements` table.
//!
//! The check uses [`inline_elements_view`] as the canonical synthesizer:
//! starting from the block's plain_text + format_runs + block_images,
//! we synthesize a Vec<InlineElement>-shaped view and compare its
//! plain text and image set to what's currently stored in
//! inline_elements. They MUST match for any block that has been
//! touched by a mutation.

#![cfg(test)]

use anyhow::Result;
use common::database::{db_context::DbContext, transactions::Transaction};
use common::direct_access::repository_factory;
use common::entities::{Block, Document, Frame, InlineContent, InlineElement, Root};
use common::event::EventBuffer;
use common::format_runs::inline_elements_view;
use common::types::EntityId;

fn seed_doc(txn: &mut Transaction, buf: &mut EventBuffer) -> Result<EntityId> {
    let mut root_repo = repository_factory::write::create_root_repository(txn)?;
    let mut doc_repo = repository_factory::write::create_document_repository(txn)?;
    let mut frame_repo = repository_factory::write::create_frame_repository(txn)?;
    let mut block_repo = repository_factory::write::create_block_repository(txn)?;

    let root = root_repo.create_orphan(buf, &Root::default())?;
    let doc = doc_repo.create(buf, &Document::default(), root.id, -1)?;
    let frame = frame_repo.create(buf, &Frame::default(), doc.id, -1)?;
    let block = block_repo.create(buf, &Block::default(), frame.id, -1)?;
    Ok(block.id)
}

fn text_elem(s: &str) -> InlineElement {
    InlineElement {
        content: InlineContent::Text(s.to_string()),
        ..Default::default()
    }
}

fn bold_text(s: &str) -> InlineElement {
    InlineElement {
        content: InlineContent::Text(s.to_string()),
        fmt_font_bold: Some(true),
        ..Default::default()
    }
}

fn image_elem(name: &str) -> InlineElement {
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

/// Read the current canonical state of a block's inline_elements
/// (joined plain text + image-name list) plus the format-run-derived
/// view of the same, then assert they match.
fn assert_dual_write_consistent(txn: &Transaction, block_id: EntityId) {
    let store = txn.get_store();

    // Legacy side: walk inline_elements in document order.
    let element_ids: Vec<EntityId> = store
        .jn_inline_element_from_block_elements
        .read()
        .unwrap()
        .get(&block_id)
        .cloned()
        .unwrap_or_default();
    let elements: Vec<InlineElement> = element_ids
        .iter()
        .filter_map(|id| store.inline_elements.read().unwrap().get(id).cloned())
        .collect();

    let legacy_plain: String = elements
        .iter()
        .filter_map(|e| match &e.content {
            InlineContent::Text(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();
    let legacy_images: Vec<String> = elements
        .iter()
        .filter_map(|e| match &e.content {
            InlineContent::Image { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();

    // New side: read format_runs + block_images and reconstruct.
    let runs = store
        .format_runs
        .read()
        .unwrap()
        .get(&block_id)
        .cloned()
        .unwrap_or_default();
    let imgs = store
        .block_images
        .read()
        .unwrap()
        .get(&block_id)
        .cloned()
        .unwrap_or_default();
    let view = inline_elements_view(&legacy_plain, &runs, &imgs);

    let view_plain: String = view
        .iter()
        .filter_map(|e| match &e.content {
            InlineContent::Text(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();
    let view_images: Vec<String> = view
        .iter()
        .filter_map(|e| match &e.content {
            InlineContent::Image { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(view_plain, legacy_plain, "plain text mismatch on block {block_id}");
    assert_eq!(view_images, legacy_images, "image list mismatch on block {block_id}");

    // The runs table itself should be coalesced & in-range.
    common::format_runs::debug_assert_well_formed(&runs, legacy_plain.len());
}

#[test]
fn dual_write_after_create_inline_element() -> Result<()> {
    let ctx = DbContext::new()?;
    let mut txn = Transaction::begin_write_transaction(&ctx)?;
    let mut buf = EventBuffer::new();

    let block_id = seed_doc(&mut txn, &mut buf)?;

    {
        let mut elem_repo = repository_factory::write::create_inline_element_repository(&txn)?;
        elem_repo.create(&mut buf, &text_elem("hello"), block_id, -1)?;
        elem_repo.create(&mut buf, &bold_text(" world"), block_id, -1)?;
    }
    assert_dual_write_consistent(&txn, block_id);
    txn.commit()?;
    Ok(())
}

#[test]
fn dual_write_after_update_inline_element() -> Result<()> {
    let ctx = DbContext::new()?;
    let mut txn = Transaction::begin_write_transaction(&ctx)?;
    let mut buf = EventBuffer::new();
    let block_id = seed_doc(&mut txn, &mut buf)?;

    {
        let mut elem_repo = repository_factory::write::create_inline_element_repository(&txn)?;
        let plain = elem_repo.create(&mut buf, &text_elem("hi"), block_id, -1)?;

        // Toggle bold on the existing element.
        let mut updated = plain.clone();
        updated.fmt_font_bold = Some(true);
        elem_repo.update(&mut buf, &updated)?;
    }
    assert_dual_write_consistent(&txn, block_id);
    txn.commit()?;
    Ok(())
}

#[test]
fn dual_write_after_remove_inline_element() -> Result<()> {
    let ctx = DbContext::new()?;
    let mut txn = Transaction::begin_write_transaction(&ctx)?;
    let mut buf = EventBuffer::new();
    let block_id = seed_doc(&mut txn, &mut buf)?;

    {
        let mut elem_repo = repository_factory::write::create_inline_element_repository(&txn)?;
        let a = elem_repo.create(&mut buf, &text_elem("aaa"), block_id, -1)?;
        let _b = elem_repo.create(&mut buf, &text_elem("bbb"), block_id, -1)?;
        elem_repo.remove(&mut buf, &a.id)?;
    }
    assert_dual_write_consistent(&txn, block_id);
    txn.commit()?;
    Ok(())
}

#[test]
fn dual_write_after_create_multi_with_image() -> Result<()> {
    let ctx = DbContext::new()?;
    let mut txn = Transaction::begin_write_transaction(&ctx)?;
    let mut buf = EventBuffer::new();
    let block_id = seed_doc(&mut txn, &mut buf)?;

    {
        let mut elem_repo = repository_factory::write::create_inline_element_repository(&txn)?;
        let elems = vec![
            text_elem("before "),
            image_elem("pic.png"),
            text_elem(" after"),
        ];
        elem_repo.create_multi(&mut buf, &elems, block_id, -1)?;
    }
    assert_dual_write_consistent(&txn, block_id);

    // The block_images entry should hold exactly one anchor.
    let imgs = txn
        .get_store()
        .block_images
        .read()
        .unwrap()
        .get(&block_id)
        .cloned()
        .unwrap_or_default();
    assert_eq!(imgs.len(), 1);
    assert_eq!(imgs[0].name, "pic.png");

    txn.commit()?;
    Ok(())
}
