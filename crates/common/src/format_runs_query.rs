//! Store-aware readers that synthesize `Vec<InlineElement>` from
//! per-block `format_runs` + `block_images`. Use cases that previously
//! walked InlineElement entities can swap their data source from
//! `uow.get_inline_element_multi(&ids)` to
//! `synthesize_block_inline_elements(&uow.store(), block_id, &block.plain_text)`.

use crate::database::hashmap_store::HashMapStore;
use crate::entities::InlineElement;
use crate::format_runs::{FormatRun, ImageAnchor, inline_elements_view_with_block_id};
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
