//! Shared document interior state.
//!
//! # Lock ordering (enforced by convention, never violated)
//!
//! 1. `TextDocumentInner` (the document lock)
//! 2. `CursorData` (individual cursor locks)
//!
//! Always acquire the document lock before any cursor lock.
//! Pure cursor-local reads (position, anchor, has_selection) may lock
//! only CursorData — this is safe because they never touch the document
//! lock in the same call. Editing methods must lock the document first,
//! then read/update cursor data while the document lock is held, and
//! call `adjust_cursors()` before releasing the document lock.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Weak};

use parking_lot::Mutex;

use anyhow::Result;
use frontend::AppContext;
use frontend::EventHubClient;
use frontend::common::types::EntityId;

use crate::DocumentEvent;

/// Cursor position data stored inside the document for automatic adjustment.
pub(crate) struct CursorData {
    pub position: usize,
    pub anchor: usize,
}

/// Callback entry for document event subscriptions.
pub(crate) struct CallbackEntry {
    pub alive: Weak<AtomicBool>,
    pub callback: Arc<dyn Fn(DocumentEvent) + Send + Sync>,
}

/// The shared document interior, behind `Arc<Mutex<>>`.
pub(crate) struct TextDocumentInner {
    pub ctx: AppContext,
    #[allow(dead_code)] // will be used for backend event wiring
    pub event_client: EventHubClient,
    pub stack_id: u64,
    #[allow(dead_code)] // will be used for entity tree access
    pub root_id: EntityId,
    pub document_id: EntityId,
    pub modified: bool,

    // Cursor tracking
    pub cursors: Vec<Weak<Mutex<CursorData>>>,

    // Event dispatch
    pub pending_events: Vec<DocumentEvent>,
    pub callbacks: Vec<CallbackEntry>,
    /// Index into `pending_events` of the next event to dispatch to callbacks.
    pub dispatch_cursor: usize,

    // Resource name → entity ID cache for O(1) lookups by name.
    pub resource_cache: HashMap<String, u64>,
}

impl TextDocumentInner {
    /// Remove dead `Weak` refs from the cursor list.
    pub fn prune_dead_cursors(&mut self) {
        self.cursors.retain(|w| w.strong_count() > 0);
    }

    /// After an edit, adjust all tracked cursor positions.
    ///
    /// Called while the document lock is held. Then locks individual
    /// CursorData mutexes (safe per lock ordering: doc before cursor).
    pub fn adjust_cursors(&mut self, edit_pos: usize, removed: usize, added: usize) {
        self.prune_dead_cursors();
        for weak in &self.cursors {
            if let Some(cursor) = weak.upgrade() {
                let mut data = cursor.lock();
                data.position = adjust_offset(data.position, edit_pos, removed, added);
                data.anchor = adjust_offset(data.anchor, edit_pos, removed, added);
            }
        }
    }

    /// Register a new cursor and return its shared data.
    pub fn register_cursor(&mut self, position: usize) -> Arc<Mutex<CursorData>> {
        self.prune_dead_cursors();
        let data = Arc::new(Mutex::new(CursorData {
            position,
            anchor: position,
        }));
        self.cursors.push(Arc::downgrade(&data));
        data
    }

    /// Queue a DocumentEvent for deferred dispatch.
    ///
    /// Events are collected while the lock is held, then dispatched
    /// after the lock is released via [`dispatch_queued_events`].
    pub fn queue_event(&mut self, event: DocumentEvent) {
        self.pending_events.push(event);
    }

    /// Collect un-dispatched events paired with live callbacks.
    ///
    /// Events remain in `pending_events` for `poll_events`. Only events
    /// added since the last call are returned. The caller should release
    /// the lock and then invoke callbacks via [`dispatch_queued_events`].
    pub fn take_queued_events(
        &mut self,
    ) -> Vec<(DocumentEvent, Vec<Arc<dyn Fn(DocumentEvent) + Send + Sync>>)> {
        if self.dispatch_cursor >= self.pending_events.len() {
            return Vec::new();
        }

        self.callbacks
            .retain(|entry| entry.alive.strong_count() > 0);

        let live_callbacks: Vec<Arc<dyn Fn(DocumentEvent) + Send + Sync>> = self
            .callbacks
            .iter()
            .filter_map(|entry| {
                let alive = entry.alive.upgrade()?;
                if alive.load(std::sync::atomic::Ordering::Relaxed) {
                    Some(Arc::clone(&entry.callback))
                } else {
                    None
                }
            })
            .collect();

        if live_callbacks.is_empty() {
            self.dispatch_cursor = self.pending_events.len();
            return Vec::new();
        }

        let new_events: Vec<DocumentEvent> = self.pending_events[self.dispatch_cursor..]
            .iter()
            .cloned()
            .collect();
        self.dispatch_cursor = self.pending_events.len();

        new_events
            .into_iter()
            .map(|e| (e, live_callbacks.clone()))
            .collect()
    }

    /// Initialize the document: create Root → Document → Frame → Block → InlineElement.
    pub fn initialize(ctx: AppContext) -> Result<Self> {
        use frontend::block::dtos::CreateBlockDto;
        use frontend::commands::{
            block_commands, document_commands, frame_commands, inline_element_commands,
            root_commands, undo_redo_commands,
        };
        use frontend::document::dtos::CreateDocumentDto;
        use frontend::frame::dtos::CreateFrameDto;
        use frontend::inline_element::dtos::CreateInlineElementDto;
        use frontend::root::dtos::CreateRootDto;

        let event_client = EventHubClient::new(&ctx.event_hub);
        event_client.start(ctx.quit_signal.clone());

        let stack_id = undo_redo_commands::create_new_stack(&ctx);

        // Create entity tree: Root → Document → Frame → Block → InlineElement
        let root = root_commands::create_orphan_root(&ctx, &CreateRootDto::default())?;
        let doc = document_commands::create_document(
            &ctx,
            Some(stack_id),
            &CreateDocumentDto::default(),
            root.id,
            -1,
        )?;
        let frame = frame_commands::create_frame(
            &ctx,
            Some(stack_id),
            &CreateFrameDto::default(),
            doc.id,
            -1,
        )?;
        let block = block_commands::create_block(
            &ctx,
            Some(stack_id),
            &CreateBlockDto::default(),
            frame.id,
            -1,
        )?;
        let _element = inline_element_commands::create_inline_element(
            &ctx,
            Some(stack_id),
            &CreateInlineElementDto::default(),
            block.id,
            -1,
        )?;

        // Clear undo stack — initialization shouldn't be undoable
        undo_redo_commands::clear_stack(&ctx, stack_id);

        Ok(Self {
            ctx,
            event_client,
            stack_id,
            root_id: root.id,
            document_id: doc.id,
            modified: false,
            cursors: Vec::new(),
            pending_events: Vec::new(),
            callbacks: Vec::new(),
            dispatch_cursor: 0,
            resource_cache: HashMap::new(),
        })
    }
}

impl Drop for TextDocumentInner {
    fn drop(&mut self) {
        self.ctx.shutdown();
    }
}

/// Dispatch queued events outside the lock.
///
/// Call this after releasing the document mutex to avoid deadlocks
/// when callbacks call back into the document.
pub(crate) fn dispatch_queued_events(
    queued: Vec<(DocumentEvent, Vec<Arc<dyn Fn(DocumentEvent) + Send + Sync>>)>,
) {
    for (event, callbacks) in queued {
        for cb in &callbacks {
            cb(event.clone());
        }
    }
}

/// Shift an offset after an edit: offsets before the edit are unchanged,
/// offsets inside the removed range clamp to the edit point, offsets after
/// shift by the delta.
pub(crate) fn adjust_offset(offset: usize, edit_pos: usize, removed: usize, added: usize) -> usize {
    if offset <= edit_pos {
        offset
    } else if offset <= edit_pos + removed {
        edit_pos + added
    } else {
        offset - removed + added
    }
}
