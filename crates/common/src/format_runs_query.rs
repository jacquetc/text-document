//! Store-aware readers that synthesize `Vec<InlineElement>` from
//! per-block `format_runs` + `block_images`. Use cases that previously
//! walked InlineElement entities can swap their data source from
//! `uow.get_inline_element_multi(&ids)` to
//! `synthesize_block_inline_elements(&uow.store(), block_id, &block.plain_text)`.

use crate::database::hashmap_store::HashMapStore;
use crate::entities::InlineElement;
use crate::format_runs::{
    FormatRun, ImageAnchor, inline_elements_view, inline_elements_view_with_block_id,
};
use crate::types::EntityId;

/// Fetch the format runs for a block. Returns an empty Vec if the block
/// has no runs (treated the same as a missing entry).
pub fn get_format_runs(store: &HashMapStore, block_id: EntityId) -> Vec<FormatRun> {
    store
        .format_runs
        .read()
        .unwrap()
        .get(&block_id)
        .cloned()
        .unwrap_or_default()
}

/// Fetch the image anchors for a block.
pub fn get_block_images(store: &HashMapStore, block_id: EntityId) -> Vec<ImageAnchor> {
    store
        .block_images
        .read()
        .unwrap()
        .get(&block_id)
        .cloned()
        .unwrap_or_default()
}

/// Synthesize the `Vec<InlineElement>` view for a block from its
/// format_runs and block_images. Callers must pass the block's
/// `plain_text` (which they already have in scope from a prior
/// `get_block` call) — this avoids re-locking the blocks table.
pub fn synthesize_block_inline_elements(
    store: &HashMapStore,
    block_id: EntityId,
    block_plain_text: &str,
) -> Vec<InlineElement> {
    let runs = get_format_runs(store, block_id);
    let images = get_block_images(store, block_id);
    inline_elements_view_with_block_id(block_plain_text, &runs, &images, block_id)
}

/// Replace the per-block `inline_elements` (entity table + junction
/// `jn_inline_element_from_block_elements`) with a fresh list synthesized
/// from the block's current `plain_text` + `format_runs` + `block_images`.
///
/// Used by writers that have just mutated the new (format_runs / block_images)
/// representation to keep the legacy `inline_elements` view consistent for
/// callers that still read it (legacy writers, undiscoverable readers,
/// auto-sync invariants). After this call, both representations describe
/// the same logical content; the InlineElement entities receive freshly
/// allocated ids from the store's counter so they round-trip through any
/// downstream legacy logic that looks them up.
pub fn rebuild_block_inline_elements(
    store: &HashMapStore,
    block_id: EntityId,
    block_plain_text: &str,
) {
    let runs = get_format_runs(store, block_id);
    let images = get_block_images(store, block_id);
    // Use the unscoped view (block_id=0) so synthesized elements carry
    // default ids; we assign fresh real ids below to keep them queryable
    // via inline_element_controller::get.
    let mut synthesized: Vec<InlineElement> = inline_elements_view(block_plain_text, &runs, &images);
    // Legacy convention: an empty block carries exactly one Empty
    // inline_element. Unmigrated writers (insert_formatted_text_uc,
    // insert_fragment_uc, …) require at least one element to walk.
    if synthesized.is_empty() {
        synthesized.push(InlineElement {
            content: crate::entities::InlineContent::Empty,
            ..InlineElement::default()
        });
    }

    // Drop the old elements (both table entries and junction list) before
    // inserting the new ones so we don't leak stale rows.
    let old_ids: Vec<EntityId> = store
        .jn_inline_element_from_block_elements
        .read()
        .unwrap()
        .get(&block_id)
        .cloned()
        .unwrap_or_default();
    {
        let mut table = store.inline_elements.write().unwrap();
        for old_id in &old_ids {
            table.remove(old_id);
        }
    }

    // Assign fresh ids and write them in. Counter semantics match the
    // store's `next_id`: the counter holds the *next* id to hand out;
    // assign first, then increment, so subsequent allocations by either
    // path don't collide.
    let mut new_ids: Vec<EntityId> = Vec::with_capacity(synthesized.len());
    let now = chrono::Utc::now();
    {
        let mut counters = store.counters.write().unwrap();
        let counter = counters.entry("inline_element".to_string()).or_insert(1);
        let mut table = store.inline_elements.write().unwrap();
        for mut elem in synthesized {
            let id = *counter;
            *counter += 1;
            elem.id = id;
            elem.created_at = now;
            elem.updated_at = now;
            table.insert(elem.id, elem.clone());
            new_ids.push(elem.id);
        }
    }

    // Replace the junction.
    store
        .jn_inline_element_from_block_elements
        .write()
        .unwrap()
        .insert(block_id, new_ids);
}

/// Drop the inline_elements junction + table entries owned by a block
/// that is being removed entirely. Idempotent.
pub fn drop_block_inline_elements(store: &HashMapStore, block_id: EntityId) {
    let old_ids: Vec<EntityId> = store
        .jn_inline_element_from_block_elements
        .write()
        .unwrap()
        .remove(&block_id)
        .unwrap_or_default();
    let mut table = store.inline_elements.write().unwrap();
    for id in &old_ids {
        table.remove(id);
    }
}
