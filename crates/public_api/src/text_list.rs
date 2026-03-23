//! Read-only list handle.

use std::sync::Arc;

use parking_lot::Mutex;

use frontend::commands::{block_commands, list_commands};
use frontend::common::types::EntityId;

use crate::ListStyle;
use crate::inner::TextDocumentInner;
use crate::text_block::{TextBlock, format_list_marker};

/// A read-only handle to a list in the document.
///
/// Created via [`TextBlock::list()`].
#[derive(Clone)]
pub struct TextList {
    pub(crate) doc: Arc<Mutex<TextDocumentInner>>,
    pub(crate) list_id: usize,
}

impl TextList {
    /// Stable entity ID. O(1).
    pub fn id(&self) -> usize {
        self.list_id
    }

    /// List style (Disc, Circle, Square, Decimal, LowerAlpha, etc.). O(1).
    pub fn style(&self) -> ListStyle {
        let inner = self.doc.lock();
        list_commands::get_list(&inner.ctx, &(self.list_id as u64))
            .ok()
            .flatten()
            .map(|l| l.style)
            .unwrap_or(ListStyle::Disc)
    }

    /// Indentation level. O(1).
    pub fn indent(&self) -> u8 {
        let inner = self.doc.lock();
        list_commands::get_list(&inner.ctx, &(self.list_id as u64))
            .ok()
            .flatten()
            .map(|l| l.indent as u8)
            .unwrap_or(0)
    }

    /// Text before the marker (e.g., "("). O(1).
    pub fn prefix(&self) -> String {
        let inner = self.doc.lock();
        list_commands::get_list(&inner.ctx, &(self.list_id as u64))
            .ok()
            .flatten()
            .map(|l| l.prefix)
            .unwrap_or_default()
    }

    /// Text after the marker (e.g., ")"). O(1).
    pub fn suffix(&self) -> String {
        let inner = self.doc.lock();
        list_commands::get_list(&inner.ctx, &(self.list_id as u64))
            .ok()
            .flatten()
            .map(|l| l.suffix)
            .unwrap_or_default()
    }

    /// Number of blocks in this list. **O(n)** — scans all blocks.
    pub fn count(&self) -> usize {
        let inner = self.doc.lock();
        let list_entity_id = self.list_id as EntityId;
        let all_blocks = block_commands::get_all_block(&inner.ctx).unwrap_or_default();
        all_blocks
            .iter()
            .filter(|b| b.list == Some(list_entity_id))
            .count()
    }

    /// Block at the given 0-based index within this list. **O(n)**.
    pub fn item(&self, index: usize) -> Option<TextBlock> {
        let inner = self.doc.lock();
        let list_entity_id = self.list_id as EntityId;
        let all_blocks = block_commands::get_all_block(&inner.ctx).unwrap_or_default();
        let mut list_blocks: Vec<_> = all_blocks
            .iter()
            .filter(|b| b.list == Some(list_entity_id))
            .collect();
        list_blocks.sort_by_key(|b| b.document_position);

        list_blocks.get(index).map(|b| TextBlock {
            doc: Arc::clone(&self.doc),
            block_id: b.id as usize,
        })
    }

    /// Formatted marker for item at index. O(1) (after looking up list properties).
    pub fn item_marker(&self, index: usize) -> String {
        let inner = self.doc.lock();
        match list_commands::get_list(&inner.ctx, &(self.list_id as u64))
            .ok()
            .flatten()
        {
            Some(list_dto) => format_list_marker(&list_dto, index),
            None => String::new(),
        }
    }
}
