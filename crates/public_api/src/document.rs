//! TextDocument implementation.

use std::sync::{Arc, Mutex};

use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

use frontend::commands::{
    document_commands, document_inspection_commands, document_io_commands,
    document_search_commands, resource_commands, undo_redo_commands,
};
use frontend::common::entities::{ResourceType, TextDirection, WrapMode};

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
    pub fn new() -> Self {
        let ctx = frontend::AppContext::new();
        let doc_inner = TextDocumentInner::initialize(ctx).expect("failed to initialize document");
        Self {
            inner: Arc::new(Mutex::new(doc_inner)),
        }
    }

    // ── Whole-document content ────────────────────────────────

    /// Replace the entire document with plain text. Clears undo history.
    pub fn set_plain_text(&self, text: &str) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        let dto = frontend::document_io::ImportPlainTextDto {
            plain_text: text.into(),
        };
        document_io_commands::import_plain_text(&inner.ctx, &dto)?;
        undo_redo_commands::clear_stack(&inner.ctx, inner.stack_id);
        inner.emit_event(DocumentEvent::DocumentReset);
        Ok(())
    }

    /// Export the entire document as plain text.
    pub fn to_plain_text(&self) -> Result<String> {
        let inner = self.inner.lock().unwrap();
        let dto = document_io_commands::export_plain_text(&inner.ctx)?;
        Ok(dto.plain_text)
    }

    /// Replace the entire document with Markdown. Clears undo history.
    ///
    /// This is a **long operation**. Returns a typed [`Operation`] handle.
    pub fn set_markdown(&self, markdown: &str) -> Result<Operation<MarkdownImportResult>> {
        let inner = self.inner.lock().unwrap();
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
        let inner = self.inner.lock().unwrap();
        let dto = document_io_commands::export_markdown(&inner.ctx)?;
        Ok(dto.markdown_text)
    }

    /// Replace the entire document with HTML. Clears undo history.
    ///
    /// This is a **long operation**. Returns a typed [`Operation`] handle.
    pub fn set_html(&self, html: &str) -> Result<Operation<HtmlImportResult>> {
        let inner = self.inner.lock().unwrap();
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
        let inner = self.inner.lock().unwrap();
        let dto = document_io_commands::export_html(&inner.ctx)?;
        Ok(dto.html_text)
    }

    /// Export the entire document as LaTeX.
    pub fn to_latex(&self, document_class: &str, include_preamble: bool) -> Result<String> {
        let inner = self.inner.lock().unwrap();
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
        let inner = self.inner.lock().unwrap();
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
        let mut inner = self.inner.lock().unwrap();
        let dto = frontend::document_io::ImportPlainTextDto {
            plain_text: String::new(),
        };
        document_io_commands::import_plain_text(&inner.ctx, &dto)?;
        undo_redo_commands::clear_stack(&inner.ctx, inner.stack_id);
        inner.emit_event(DocumentEvent::DocumentReset);
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
            let mut inner = self.inner.lock().unwrap();
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
        let inner = self.inner.lock().unwrap();
        let dto = document_inspection_commands::get_document_stats(&inner.ctx)
            .expect("get_document_stats should not fail");
        DocumentStats::from(&dto)
    }

    /// Get the total character count. O(1) — reads cached value.
    pub fn character_count(&self) -> usize {
        self.stats().character_count
    }

    /// Get the number of blocks (paragraphs). O(1) — reads cached value.
    pub fn block_count(&self) -> usize {
        self.stats().block_count
    }

    /// Returns true if the document has no text content.
    pub fn is_empty(&self) -> bool {
        self.character_count() == 0
    }

    /// Get text at a position for a given length.
    pub fn text_at(&self, position: usize, length: usize) -> Result<String> {
        let inner = self.inner.lock().unwrap();
        let dto = frontend::document_inspection::GetTextAtPositionDto {
            position: to_i64(position),
            length: to_i64(length),
        };
        let result = document_inspection_commands::get_text_at_position(&inner.ctx, &dto)?;
        Ok(result.text)
    }

    /// Get info about the block at a position. O(log n).
    pub fn block_at(&self, position: usize) -> Result<BlockInfo> {
        let inner = self.inner.lock().unwrap();
        let dto = frontend::document_inspection::GetBlockAtPositionDto {
            position: to_i64(position),
        };
        let result = document_inspection_commands::get_block_at_position(&inner.ctx, &dto)?;
        Ok(BlockInfo::from(&result))
    }

    /// Get the block format at a position.
    pub fn block_format_at(&self, position: usize) -> Result<BlockFormat> {
        let inner = self.inner.lock().unwrap();
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
        let inner = self.inner.lock().unwrap();
        let dto = options.to_find_text_dto(query, from);
        let result = document_search_commands::find_text(&inner.ctx, &dto)?;
        Ok(convert::find_result_to_match(&result))
    }

    /// Find all occurrences.
    pub fn find_all(&self, query: &str, options: &FindOptions) -> Result<Vec<FindMatch>> {
        let inner = self.inner.lock().unwrap();
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
        let inner = self.inner.lock().unwrap();
        let dto = options.to_replace_dto(query, replacement, replace_all);
        let result =
            document_search_commands::replace_text(&inner.ctx, Some(inner.stack_id), &dto)?;
        Ok(to_usize(result.replacements_count))
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
        let inner = self.inner.lock().unwrap();
        let dto = frontend::resource::dtos::CreateResourceDto {
            created_at: Default::default(),
            updated_at: Default::default(),
            resource_type,
            name: name.into(),
            url: String::new(),
            mime_type: mime_type.into(),
            data_base64: BASE64.encode(data),
        };
        resource_commands::create_resource(
            &inner.ctx,
            Some(inner.stack_id),
            &dto,
            inner.document_id,
            -1,
        )?;
        Ok(())
    }

    /// Get a resource by name. Returns `None` if not found.
    pub fn resource(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let inner = self.inner.lock().unwrap();
        let all = resource_commands::get_all_resource(&inner.ctx)?;
        for r in &all {
            if r.name == name {
                let bytes = BASE64.decode(&r.data_base64)?;
                return Ok(Some(bytes));
            }
        }
        Ok(None)
    }

    // ── Undo / Redo ──────────────────────────────────────────

    /// Undo the last operation.
    pub fn undo(&self) -> Result<()> {
        let inner = self.inner.lock().unwrap();
        undo_redo_commands::undo(&inner.ctx, Some(inner.stack_id))
    }

    /// Redo the last undone operation.
    pub fn redo(&self) -> Result<()> {
        let inner = self.inner.lock().unwrap();
        undo_redo_commands::redo(&inner.ctx, Some(inner.stack_id))
    }

    /// Returns true if there are operations that can be undone.
    pub fn can_undo(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        undo_redo_commands::can_undo(&inner.ctx, Some(inner.stack_id))
    }

    /// Returns true if there are operations that can be redone.
    pub fn can_redo(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        undo_redo_commands::can_redo(&inner.ctx, Some(inner.stack_id))
    }

    /// Clear all undo/redo history.
    pub fn clear_undo_redo(&self) {
        let inner = self.inner.lock().unwrap();
        undo_redo_commands::clear_stack(&inner.ctx, inner.stack_id);
    }

    // ── Modified state ───────────────────────────────────────

    /// Returns true if the document has been modified since creation or last reset.
    pub fn is_modified(&self) -> bool {
        self.inner.lock().unwrap().modified
    }

    /// Set or clear the modified flag.
    pub fn set_modified(&self, modified: bool) {
        let mut inner = self.inner.lock().unwrap();
        if inner.modified != modified {
            inner.modified = modified;
            inner.emit_event(DocumentEvent::ModificationChanged(modified));
        }
    }

    // ── Document properties ──────────────────────────────────

    /// Get the document title.
    pub fn title(&self) -> String {
        let inner = self.inner.lock().unwrap();
        document_commands::get_document(&inner.ctx, &inner.document_id)
            .ok()
            .flatten()
            .map(|d| d.title)
            .unwrap_or_default()
    }

    /// Set the document title.
    pub fn set_title(&self, title: &str) -> Result<()> {
        let inner = self.inner.lock().unwrap();
        let doc = document_commands::get_document(&inner.ctx, &inner.document_id)?
            .ok_or_else(|| anyhow::anyhow!("document not found"))?;
        let mut update: frontend::document::dtos::UpdateDocumentDto = doc.into();
        update.title = title.into();
        document_commands::update_document(&inner.ctx, Some(inner.stack_id), &update)?;
        Ok(())
    }

    /// Get the text direction.
    pub fn text_direction(&self) -> TextDirection {
        let inner = self.inner.lock().unwrap();
        document_commands::get_document(&inner.ctx, &inner.document_id)
            .ok()
            .flatten()
            .map(|d| d.text_direction)
            .unwrap_or(TextDirection::LeftToRight)
    }

    /// Set the text direction.
    pub fn set_text_direction(&self, direction: TextDirection) -> Result<()> {
        let inner = self.inner.lock().unwrap();
        let doc = document_commands::get_document(&inner.ctx, &inner.document_id)?
            .ok_or_else(|| anyhow::anyhow!("document not found"))?;
        let mut update: frontend::document::dtos::UpdateDocumentDto = doc.into();
        update.text_direction = direction;
        document_commands::update_document(&inner.ctx, Some(inner.stack_id), &update)?;
        Ok(())
    }

    /// Get the default wrap mode.
    pub fn default_wrap_mode(&self) -> WrapMode {
        let inner = self.inner.lock().unwrap();
        document_commands::get_document(&inner.ctx, &inner.document_id)
            .ok()
            .flatten()
            .map(|d| d.default_wrap_mode)
            .unwrap_or(WrapMode::WordWrap)
    }

    /// Set the default wrap mode.
    pub fn set_default_wrap_mode(&self, mode: WrapMode) -> Result<()> {
        let inner = self.inner.lock().unwrap();
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
    /// **Warning:** The callback is invoked while the document lock is held.
    /// Do **not** call any `TextDocument` or `TextCursor` methods inside the
    /// callback — this will deadlock. Keep the callback lightweight and
    /// dispatch heavy work to another thread or queue.
    pub fn on_change<F>(&self, callback: F) -> Subscription
    where
        F: Fn(DocumentEvent) + Send + 'static,
    {
        let mut inner = self.inner.lock().unwrap();
        events::subscribe_inner(&mut inner, callback)
    }

    /// Drain all pending events since the last call.
    pub fn poll_events(&self) -> Vec<DocumentEvent> {
        let mut inner = self.inner.lock().unwrap();
        std::mem::take(&mut inner.pending_events)
    }
}

impl Default for TextDocument {
    fn default() -> Self {
        Self::new()
    }
}
