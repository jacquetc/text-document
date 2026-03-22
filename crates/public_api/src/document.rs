//! TextDocument implementation.

use std::sync::Arc;

use parking_lot::Mutex;

use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

use frontend::commands::{
    document_commands, document_inspection_commands, document_io_commands,
    document_search_commands, resource_commands, undo_redo_commands,
};
use crate::{ResourceType, TextDirection, WrapMode};

use crate::convert::{self, to_i64, to_usize};
use crate::cursor::TextCursor;
use crate::events::{self, DocumentEvent, Subscription};
use crate::inner::TextDocumentInner;
use crate::operation::{DocxExportResult, HtmlImportResult, MarkdownImportResult, Operation};
use crate::{BlockFormat, BlockInfo, DocumentStats, FindMatch, FindOptions};

/// A rich text document.
///
/// Owns the backend (database, event hub, undo/redo manager) and provides
/// document-level operations. All cursor-based editing goes through
/// [`TextCursor`], obtained via [`cursor()`](TextDocument::cursor) or
/// [`cursor_at()`](TextDocument::cursor_at).
///
/// Internally uses `Arc<Mutex<...>>` so that multiple [`TextCursor`]s can
/// coexist and edit concurrently. Cloning a `TextDocument` creates a new
/// handle to the **same** underlying document (like Qt's implicit sharing).
#[derive(Clone)]
pub struct TextDocument {
    pub(crate) inner: Arc<Mutex<TextDocumentInner>>,
}

impl TextDocument {
    // ── Construction ──────────────────────────────────────────

    /// Create a new, empty document.
    ///
    /// # Panics
    ///
    /// Panics if the database context cannot be created (e.g. filesystem error).
    /// Use [`TextDocument::try_new`] for a fallible alternative.
    pub fn new() -> Self {
        Self::try_new().expect("failed to initialize document")
    }

    /// Create a new, empty document, returning an error on failure.
    pub fn try_new() -> Result<Self> {
        let ctx = frontend::AppContext::new();
        let doc_inner = TextDocumentInner::initialize(ctx)?;
        let inner = Arc::new(Mutex::new(doc_inner));

        // Bridge backend long-operation events to public DocumentEvent.
        Self::subscribe_long_operation_events(&inner);

        Ok(Self { inner })
    }

    /// Subscribe to backend long-operation events and bridge them to DocumentEvent.
    fn subscribe_long_operation_events(inner: &Arc<Mutex<TextDocumentInner>>) {
        use frontend::common::event::{LongOperationEvent as LOE, Origin};

        let weak = Arc::downgrade(inner);
        {
            let locked = inner.lock();
            // Progress
            let w = weak.clone();
            locked.event_client.subscribe(
                Origin::LongOperation(LOE::Progress),
                move |event| {
                    if let Some(inner) = w.upgrade() {
                        let (op_id, percent, message) = parse_progress_data(&event.data);
                        let mut inner = inner.lock();
                        inner.queue_event(DocumentEvent::LongOperationProgress {
                            operation_id: op_id,
                            percent,
                            message,
                        });
                    }
                },
            );

            // Completed
            let w = weak.clone();
            locked.event_client.subscribe(
                Origin::LongOperation(LOE::Completed),
                move |event| {
                    if let Some(inner) = w.upgrade() {
                        let op_id = parse_id_data(&event.data);
                        let mut inner = inner.lock();
                        inner.queue_event(DocumentEvent::DocumentReset);
                        inner.check_block_count_changed();
                        inner.queue_event(DocumentEvent::LongOperationFinished {
                            operation_id: op_id,
                            success: true,
                            error: None,
                        });
                    }
                },
            );

            // Cancelled
            let w = weak.clone();
            locked.event_client.subscribe(
                Origin::LongOperation(LOE::Cancelled),
                move |event| {
                    if let Some(inner) = w.upgrade() {
                        let op_id = parse_id_data(&event.data);
                        let mut inner = inner.lock();
                        inner.queue_event(DocumentEvent::LongOperationFinished {
                            operation_id: op_id,
                            success: false,
                            error: Some("cancelled".into()),
                        });
                    }
                },
            );

            // Failed
            locked.event_client.subscribe(
                Origin::LongOperation(LOE::Failed),
                move |event| {
                    if let Some(inner) = weak.upgrade() {
                        let (op_id, error) = parse_failed_data(&event.data);
                        let mut inner = inner.lock();
                        inner.queue_event(DocumentEvent::LongOperationFinished {
                            operation_id: op_id,
                            success: false,
                            error: Some(error),
                        });
                    }
                },
            );
        }
    }

    // ── Whole-document content ────────────────────────────────

    /// Replace the entire document with plain text. Clears undo history.
    pub fn set_plain_text(&self, text: &str) -> Result<()> {
        let queued = {
            let mut inner = self.inner.lock();
            let dto = frontend::document_io::ImportPlainTextDto {
                plain_text: text.into(),
            };
            document_io_commands::import_plain_text(&inner.ctx, &dto)?;
            undo_redo_commands::clear_stack(&inner.ctx, inner.stack_id);
            inner.invalidate_text_cache();
            inner.queue_event(DocumentEvent::DocumentReset);
            inner.check_block_count_changed();
            inner.queue_event(DocumentEvent::UndoRedoChanged {
                can_undo: false,
                can_redo: false,
            });
            inner.take_queued_events()
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    /// Export the entire document as plain text.
    pub fn to_plain_text(&self) -> Result<String> {
        let mut inner = self.inner.lock();
        Ok(inner.plain_text()?.to_string())
    }

    /// Replace the entire document with Markdown. Clears undo history.
    ///
    /// This is a **long operation**. Returns a typed [`Operation`] handle.
    pub fn set_markdown(&self, markdown: &str) -> Result<Operation<MarkdownImportResult>> {
        let mut inner = self.inner.lock();
        inner.invalidate_text_cache();
        let dto = frontend::document_io::ImportMarkdownDto {
            markdown_text: markdown.into(),
        };
        let op_id = document_io_commands::import_markdown(&inner.ctx, &dto)?;
        Ok(Operation::new(
            op_id,
            &inner.ctx,
            Box::new(|ctx, id| {
                document_io_commands::get_import_markdown_result(ctx, id)
                    .ok()
                    .flatten()
                    .map(|r| {
                        Ok(MarkdownImportResult {
                            block_count: to_usize(r.block_count),
                        })
                    })
            }),
        ))
    }

    /// Export the entire document as Markdown.
    pub fn to_markdown(&self) -> Result<String> {
        let inner = self.inner.lock();
        let dto = document_io_commands::export_markdown(&inner.ctx)?;
        Ok(dto.markdown_text)
    }

    /// Replace the entire document with HTML. Clears undo history.
    ///
    /// This is a **long operation**. Returns a typed [`Operation`] handle.
    pub fn set_html(&self, html: &str) -> Result<Operation<HtmlImportResult>> {
        let mut inner = self.inner.lock();
        inner.invalidate_text_cache();
        let dto = frontend::document_io::ImportHtmlDto {
            html_text: html.into(),
        };
        let op_id = document_io_commands::import_html(&inner.ctx, &dto)?;
        Ok(Operation::new(
            op_id,
            &inner.ctx,
            Box::new(|ctx, id| {
                document_io_commands::get_import_html_result(ctx, id)
                    .ok()
                    .flatten()
                    .map(|r| {
                        Ok(HtmlImportResult {
                            block_count: to_usize(r.block_count),
                        })
                    })
            }),
        ))
    }

    /// Export the entire document as HTML.
    pub fn to_html(&self) -> Result<String> {
        let inner = self.inner.lock();
        let dto = document_io_commands::export_html(&inner.ctx)?;
        Ok(dto.html_text)
    }

    /// Export the entire document as LaTeX.
    pub fn to_latex(&self, document_class: &str, include_preamble: bool) -> Result<String> {
        let inner = self.inner.lock();
        let dto = frontend::document_io::ExportLatexDto {
            document_class: document_class.into(),
            include_preamble,
        };
        let result = document_io_commands::export_latex(&inner.ctx, &dto)?;
        Ok(result.latex_text)
    }

    /// Export the entire document as DOCX to a file path.
    ///
    /// This is a **long operation**. Returns a typed [`Operation`] handle.
    pub fn to_docx(&self, output_path: &str) -> Result<Operation<DocxExportResult>> {
        let inner = self.inner.lock();
        let dto = frontend::document_io::ExportDocxDto {
            output_path: output_path.into(),
        };
        let op_id = document_io_commands::export_docx(&inner.ctx, &dto)?;
        Ok(Operation::new(
            op_id,
            &inner.ctx,
            Box::new(|ctx, id| {
                document_io_commands::get_export_docx_result(ctx, id)
                    .ok()
                    .flatten()
                    .map(|r| {
                        Ok(DocxExportResult {
                            file_path: r.file_path,
                            paragraph_count: to_usize(r.paragraph_count),
                        })
                    })
            }),
        ))
    }

    /// Clear all document content and reset to an empty state.
    pub fn clear(&self) -> Result<()> {
        let queued = {
            let mut inner = self.inner.lock();
            let dto = frontend::document_io::ImportPlainTextDto {
                plain_text: String::new(),
            };
            document_io_commands::import_plain_text(&inner.ctx, &dto)?;
            undo_redo_commands::clear_stack(&inner.ctx, inner.stack_id);
            inner.invalidate_text_cache();
            inner.queue_event(DocumentEvent::DocumentReset);
            inner.check_block_count_changed();
            inner.queue_event(DocumentEvent::UndoRedoChanged {
                can_undo: false,
                can_redo: false,
            });
            inner.take_queued_events()
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    // ── Cursor factory ───────────────────────────────────────

    /// Create a cursor at position 0.
    pub fn cursor(&self) -> TextCursor {
        self.cursor_at(0)
    }

    /// Create a cursor at the given position.
    pub fn cursor_at(&self, position: usize) -> TextCursor {
        let data = {
            let mut inner = self.inner.lock();
            inner.register_cursor(position)
        };
        TextCursor {
            doc: self.inner.clone(),
            data,
        }
    }

    // ── Document queries ─────────────────────────────────────

    /// Get document statistics. O(1) — reads cached values.
    pub fn stats(&self) -> DocumentStats {
        let inner = self.inner.lock();
        let dto = document_inspection_commands::get_document_stats(&inner.ctx)
            .expect("get_document_stats should not fail");
        DocumentStats::from(&dto)
    }

    /// Get the total character count. O(1) — reads cached value.
    pub fn character_count(&self) -> usize {
        let inner = self.inner.lock();
        let dto = document_inspection_commands::get_document_stats(&inner.ctx)
            .expect("get_document_stats should not fail");
        to_usize(dto.character_count)
    }

    /// Get the number of blocks (paragraphs). O(1) — reads cached value.
    pub fn block_count(&self) -> usize {
        let inner = self.inner.lock();
        let dto = document_inspection_commands::get_document_stats(&inner.ctx)
            .expect("get_document_stats should not fail");
        to_usize(dto.block_count)
    }

    /// Returns true if the document has no text content.
    pub fn is_empty(&self) -> bool {
        self.character_count() == 0
    }

    /// Get text at a position for a given length.
    pub fn text_at(&self, position: usize, length: usize) -> Result<String> {
        let inner = self.inner.lock();
        let dto = frontend::document_inspection::GetTextAtPositionDto {
            position: to_i64(position),
            length: to_i64(length),
        };
        let result = document_inspection_commands::get_text_at_position(&inner.ctx, &dto)?;
        Ok(result.text)
    }

    /// Get info about the block at a position. O(log n).
    pub fn block_at(&self, position: usize) -> Result<BlockInfo> {
        let inner = self.inner.lock();
        let dto = frontend::document_inspection::GetBlockAtPositionDto {
            position: to_i64(position),
        };
        let result = document_inspection_commands::get_block_at_position(&inner.ctx, &dto)?;
        Ok(BlockInfo::from(&result))
    }

    /// Get the block format at a position.
    pub fn block_format_at(&self, position: usize) -> Result<BlockFormat> {
        let inner = self.inner.lock();
        let dto = frontend::document_inspection::GetBlockAtPositionDto {
            position: to_i64(position),
        };
        let block_info = document_inspection_commands::get_block_at_position(&inner.ctx, &dto)?;
        let block_id = block_info.block_id;
        let block_id = block_id as u64;
        let block_dto = frontend::commands::block_commands::get_block(&inner.ctx, &block_id)?
            .ok_or_else(|| anyhow::anyhow!("block not found"))?;
        Ok(BlockFormat::from(&block_dto))
    }

    // ── Search ───────────────────────────────────────────────

    /// Find the next (or previous) occurrence. Returns `None` if not found.
    pub fn find(
        &self,
        query: &str,
        from: usize,
        options: &FindOptions,
    ) -> Result<Option<FindMatch>> {
        let inner = self.inner.lock();
        let dto = options.to_find_text_dto(query, from);
        let result = document_search_commands::find_text(&inner.ctx, &dto)?;
        Ok(convert::find_result_to_match(&result))
    }

    /// Find all occurrences.
    pub fn find_all(&self, query: &str, options: &FindOptions) -> Result<Vec<FindMatch>> {
        let inner = self.inner.lock();
        let dto = options.to_find_all_dto(query);
        let result = document_search_commands::find_all(&inner.ctx, &dto)?;
        Ok(convert::find_all_to_matches(&result))
    }

    /// Replace occurrences. Returns the number of replacements. Undoable.
    pub fn replace_text(
        &self,
        query: &str,
        replacement: &str,
        replace_all: bool,
        options: &FindOptions,
    ) -> Result<usize> {
        let (count, queued) = {
            let mut inner = self.inner.lock();
            let dto = options.to_replace_dto(query, replacement, replace_all);
            let result =
                document_search_commands::replace_text(&inner.ctx, Some(inner.stack_id), &dto)?;
            let count = to_usize(result.replacements_count);
            inner.invalidate_text_cache();
            if count > 0 {
                inner.modified = true;
                inner.queue_event(DocumentEvent::ContentsChanged {
                    position: 0,
                    chars_removed: 0,
                    chars_added: 0,
                    blocks_affected: 0,
                });
                inner.check_block_count_changed();
                let can_undo = undo_redo_commands::can_undo(&inner.ctx, Some(inner.stack_id));
                let can_redo = undo_redo_commands::can_redo(&inner.ctx, Some(inner.stack_id));
                inner.queue_event(DocumentEvent::UndoRedoChanged { can_undo, can_redo });
            }
            (count, inner.take_queued_events())
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(count)
    }

    // ── Resources ────────────────────────────────────────────

    /// Add a resource (image, stylesheet) to the document.
    pub fn add_resource(
        &self,
        resource_type: ResourceType,
        name: &str,
        mime_type: &str,
        data: &[u8],
    ) -> Result<()> {
        let mut inner = self.inner.lock();
        let dto = frontend::resource::dtos::CreateResourceDto {
            created_at: Default::default(),
            updated_at: Default::default(),
            resource_type,
            name: name.into(),
            url: String::new(),
            mime_type: mime_type.into(),
            data_base64: BASE64.encode(data),
        };
        let created = resource_commands::create_resource(
            &inner.ctx,
            Some(inner.stack_id),
            &dto,
            inner.document_id,
            -1,
        )?;
        inner.resource_cache.insert(name.to_string(), created.id);
        Ok(())
    }

    /// Get a resource by name. Returns `None` if not found.
    ///
    /// Uses an internal cache to avoid scanning all resources on repeated lookups.
    pub fn resource(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let mut inner = self.inner.lock();

        // Fast path: check the name → ID cache.
        if let Some(&id) = inner.resource_cache.get(name) {
            if let Some(r) = resource_commands::get_resource(&inner.ctx, &id)? {
                let bytes = BASE64.decode(&r.data_base64)?;
                return Ok(Some(bytes));
            }
            // ID was stale — fall through to full scan.
            inner.resource_cache.remove(name);
        }

        // Slow path: linear scan, then populate cache for the match.
        let all = resource_commands::get_all_resource(&inner.ctx)?;
        for r in &all {
            if r.name == name {
                inner.resource_cache.insert(name.to_string(), r.id);
                let bytes = BASE64.decode(&r.data_base64)?;
                return Ok(Some(bytes));
            }
        }
        Ok(None)
    }

    // ── Undo / Redo ──────────────────────────────────────────

    /// Undo the last operation.
    pub fn undo(&self) -> Result<()> {
        let queued = {
            let mut inner = self.inner.lock();
            let result = undo_redo_commands::undo(&inner.ctx, Some(inner.stack_id));
            inner.invalidate_text_cache();
            result?;
            inner.check_block_count_changed();
            let can_undo = undo_redo_commands::can_undo(&inner.ctx, Some(inner.stack_id));
            let can_redo = undo_redo_commands::can_redo(&inner.ctx, Some(inner.stack_id));
            inner.queue_event(DocumentEvent::UndoRedoChanged { can_undo, can_redo });
            inner.take_queued_events()
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    /// Redo the last undone operation.
    pub fn redo(&self) -> Result<()> {
        let queued = {
            let mut inner = self.inner.lock();
            let result = undo_redo_commands::redo(&inner.ctx, Some(inner.stack_id));
            inner.invalidate_text_cache();
            result?;
            inner.check_block_count_changed();
            let can_undo = undo_redo_commands::can_undo(&inner.ctx, Some(inner.stack_id));
            let can_redo = undo_redo_commands::can_redo(&inner.ctx, Some(inner.stack_id));
            inner.queue_event(DocumentEvent::UndoRedoChanged { can_undo, can_redo });
            inner.take_queued_events()
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    /// Returns true if there are operations that can be undone.
    pub fn can_undo(&self) -> bool {
        let inner = self.inner.lock();
        undo_redo_commands::can_undo(&inner.ctx, Some(inner.stack_id))
    }

    /// Returns true if there are operations that can be redone.
    pub fn can_redo(&self) -> bool {
        let inner = self.inner.lock();
        undo_redo_commands::can_redo(&inner.ctx, Some(inner.stack_id))
    }

    /// Clear all undo/redo history.
    pub fn clear_undo_redo(&self) {
        let inner = self.inner.lock();
        undo_redo_commands::clear_stack(&inner.ctx, inner.stack_id);
    }

    // ── Modified state ───────────────────────────────────────

    /// Returns true if the document has been modified since creation or last reset.
    pub fn is_modified(&self) -> bool {
        self.inner.lock().modified
    }

    /// Set or clear the modified flag.
    pub fn set_modified(&self, modified: bool) {
        let queued = {
            let mut inner = self.inner.lock();
            if inner.modified != modified {
                inner.modified = modified;
                inner.queue_event(DocumentEvent::ModificationChanged(modified));
            }
            inner.take_queued_events()
        };
        crate::inner::dispatch_queued_events(queued);
    }

    // ── Document properties ──────────────────────────────────

    /// Get the document title.
    pub fn title(&self) -> String {
        let inner = self.inner.lock();
        document_commands::get_document(&inner.ctx, &inner.document_id)
            .ok()
            .flatten()
            .map(|d| d.title)
            .unwrap_or_default()
    }

    /// Set the document title.
    pub fn set_title(&self, title: &str) -> Result<()> {
        let inner = self.inner.lock();
        let doc = document_commands::get_document(&inner.ctx, &inner.document_id)?
            .ok_or_else(|| anyhow::anyhow!("document not found"))?;
        let mut update: frontend::document::dtos::UpdateDocumentDto = doc.into();
        update.title = title.into();
        document_commands::update_document(&inner.ctx, Some(inner.stack_id), &update)?;
        Ok(())
    }

    /// Get the text direction.
    pub fn text_direction(&self) -> TextDirection {
        let inner = self.inner.lock();
        document_commands::get_document(&inner.ctx, &inner.document_id)
            .ok()
            .flatten()
            .map(|d| d.text_direction)
            .unwrap_or(TextDirection::LeftToRight)
    }

    /// Set the text direction.
    pub fn set_text_direction(&self, direction: TextDirection) -> Result<()> {
        let inner = self.inner.lock();
        let doc = document_commands::get_document(&inner.ctx, &inner.document_id)?
            .ok_or_else(|| anyhow::anyhow!("document not found"))?;
        let mut update: frontend::document::dtos::UpdateDocumentDto = doc.into();
        update.text_direction = direction;
        document_commands::update_document(&inner.ctx, Some(inner.stack_id), &update)?;
        Ok(())
    }

    /// Get the default wrap mode.
    pub fn default_wrap_mode(&self) -> WrapMode {
        let inner = self.inner.lock();
        document_commands::get_document(&inner.ctx, &inner.document_id)
            .ok()
            .flatten()
            .map(|d| d.default_wrap_mode)
            .unwrap_or(WrapMode::WordWrap)
    }

    /// Set the default wrap mode.
    pub fn set_default_wrap_mode(&self, mode: WrapMode) -> Result<()> {
        let inner = self.inner.lock();
        let doc = document_commands::get_document(&inner.ctx, &inner.document_id)?
            .ok_or_else(|| anyhow::anyhow!("document not found"))?;
        let mut update: frontend::document::dtos::UpdateDocumentDto = doc.into();
        update.default_wrap_mode = mode;
        document_commands::update_document(&inner.ctx, Some(inner.stack_id), &update)?;
        Ok(())
    }

    // ── Event subscription ───────────────────────────────────

    /// Subscribe to document events via callback.
    ///
    /// Callbacks are invoked **outside** the document lock (after the editing
    /// operation completes and the lock is released). It is safe to call
    /// `TextDocument` or `TextCursor` methods from within the callback without
    /// risk of deadlock. However, keep callbacks lightweight — they run
    /// synchronously on the calling thread and block the caller until they
    /// return.
    ///
    /// Drop the returned [`Subscription`] to unsubscribe.
    ///
    /// # Breaking change (v0.0.6)
    ///
    /// The callback bound changed from `Send` to `Send + Sync` in v0.0.6
    /// to support `Arc`-based dispatch. Callbacks that capture non-`Sync`
    /// types (e.g., `Rc<T>`, `Cell<T>`) must be wrapped in a `Mutex`.
    pub fn on_change<F>(&self, callback: F) -> Subscription
    where
        F: Fn(DocumentEvent) + Send + Sync + 'static,
    {
        let mut inner = self.inner.lock();
        events::subscribe_inner(&mut inner, callback)
    }

    /// Return events accumulated since the last `poll_events()` call.
    ///
    /// This delivery path is independent of callback dispatch via
    /// [`on_change`](Self::on_change) — using both simultaneously is safe
    /// and each path sees every event exactly once.
    pub fn poll_events(&self) -> Vec<DocumentEvent> {
        let mut inner = self.inner.lock();
        inner.drain_poll_events()
    }
}

impl Default for TextDocument {
    fn default() -> Self {
        Self::new()
    }
}

// ── Long-operation event data helpers ─────────────────────────

/// Parse progress JSON: `{"id":"...", "percentage": 50.0, "message": "..."}`
fn parse_progress_data(data: &Option<String>) -> (String, f64, String) {
    let Some(json) = data else {
        return (String::new(), 0.0, String::new());
    };
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    let id = v["id"].as_str().unwrap_or_default().to_string();
    let pct = v["percentage"].as_f64().unwrap_or(0.0);
    let msg = v["message"].as_str().unwrap_or_default().to_string();
    (id, pct, msg)
}

/// Parse completed/cancelled JSON: `{"id":"..."}`
fn parse_id_data(data: &Option<String>) -> String {
    let Some(json) = data else {
        return String::new();
    };
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    v["id"].as_str().unwrap_or_default().to_string()
}

/// Parse failed JSON: `{"id":"...", "error":"..."}`
fn parse_failed_data(data: &Option<String>) -> (String, String) {
    let Some(json) = data else {
        return (String::new(), "unknown error".into());
    };
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    let id = v["id"].as_str().unwrap_or_default().to_string();
    let error = v["error"].as_str().unwrap_or("unknown error").to_string();
    (id, error)
}
