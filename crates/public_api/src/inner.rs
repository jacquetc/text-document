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

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, Weak};

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
    pub callback: Box<dyn Fn(DocumentEvent) + Send>,
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
                let mut data = cursor.lock().unwrap();
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

    /// Emit a DocumentEvent: push to pending_events and invoke live callbacks.
    pub fn emit_event(&mut self, event: DocumentEvent) {
        self.pending_events.push(event.clone());
        self.callbacks
            .retain(|entry| entry.alive.strong_count() > 0);
        for entry in &self.callbacks {
            if let Some(alive) = entry.alive.upgrade() {
                if alive.load(std::sync::atomic::Ordering::Relaxed) {
                    (entry.callback)(event.clone());
                }
            }
        }
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
        })
    }
}

impl Drop for TextDocumentInner {
    fn drop(&mut self) {
        self.ctx.shutdown();
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
