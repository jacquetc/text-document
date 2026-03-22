//! TextCursor implementation — Qt-style multi-cursor with automatic position adjustment.

use std::sync::Arc;

use parking_lot::Mutex;

use anyhow::Result;

use frontend::commands::{
    document_editing_commands, document_formatting_commands, document_inspection_commands,
    inline_element_commands, undo_redo_commands,
};
use crate::ListStyle;

use unicode_segmentation::UnicodeSegmentation;

use crate::convert::{to_i64, to_usize};
use crate::events::DocumentEvent;
use crate::fragment::DocumentFragment;
use crate::inner::{CursorData, TextDocumentInner};
use crate::{BlockFormat, FrameFormat, MoveMode, MoveOperation, SelectionType, TextFormat};

/// A cursor into a [`TextDocument`](crate::TextDocument).
///
/// Multiple cursors can coexist on the same document (like Qt's `QTextCursor`).
/// When any cursor edits text, all other cursors' positions are automatically
/// adjusted by the document.
///
/// Cloning a cursor creates an **independent** cursor at the same position.
pub struct TextCursor {
    pub(crate) doc: Arc<Mutex<TextDocumentInner>>,
    pub(crate) data: Arc<Mutex<CursorData>>,
}

impl Clone for TextCursor {
    fn clone(&self) -> Self {
        let (position, anchor) = {
            let d = self.data.lock();
            (d.position, d.anchor)
        };
        let data = {
            let mut inner = self.doc.lock();
            let data = Arc::new(Mutex::new(CursorData { position, anchor }));
            inner.cursors.push(Arc::downgrade(&data));
            data
        };
        TextCursor {
            doc: self.doc.clone(),
            data,
        }
    }
}

impl TextCursor {
    // ── Helpers (called while doc lock is NOT held) ──────────

    fn read_cursor(&self) -> (usize, usize) {
        let d = self.data.lock();
        (d.position, d.anchor)
    }

    /// Common post-edit bookkeeping: adjust all cursors, set this cursor to
    /// `new_pos`, mark modified, invalidate text cache, queue a
    /// `ContentsChanged` event, and return the queued events for dispatch.
    fn finish_edit(
        &self,
        inner: &mut TextDocumentInner,
        edit_pos: usize,
        removed: usize,
        new_pos: usize,
        blocks_affected: usize,
    ) -> Vec<(DocumentEvent, Vec<Arc<dyn Fn(DocumentEvent) + Send + Sync>>)> {
        let added = new_pos - edit_pos;
        inner.adjust_cursors(edit_pos, removed, added);
        {
            let mut d = self.data.lock();
            d.position = new_pos;
            d.anchor = new_pos;
        }
        inner.modified = true;
        inner.invalidate_text_cache();
        inner.queue_event(DocumentEvent::ContentsChanged {
            position: edit_pos,
            chars_removed: removed,
            chars_added: added,
            blocks_affected,
        });
        inner.check_block_count_changed();
        self.queue_undo_redo_event(inner)
    }

    // ── Position & selection ─────────────────────────────────

    /// Current cursor position (between characters).
    pub fn position(&self) -> usize {
        self.data.lock().position
    }

    /// Anchor position. Equal to `position()` when no selection.
    pub fn anchor(&self) -> usize {
        self.data.lock().anchor
    }

    /// Returns true if there is a selection.
    pub fn has_selection(&self) -> bool {
        let d = self.data.lock();
        d.position != d.anchor
    }

    /// Start of the selection (min of position and anchor).
    pub fn selection_start(&self) -> usize {
        let d = self.data.lock();
        d.position.min(d.anchor)
    }

    /// End of the selection (max of position and anchor).
    pub fn selection_end(&self) -> usize {
        let d = self.data.lock();
        d.position.max(d.anchor)
    }

    /// Get the selected text. Returns empty string if no selection.
    pub fn selected_text(&self) -> Result<String> {
        let (pos, anchor) = self.read_cursor();
        if pos == anchor {
            return Ok(String::new());
        }
        let start = pos.min(anchor);
        let len = pos.max(anchor) - start;
        let inner = self.doc.lock();
        let dto = frontend::document_inspection::GetTextAtPositionDto {
            position: to_i64(start),
            length: to_i64(len),
        };
        let result = document_inspection_commands::get_text_at_position(&inner.ctx, &dto)?;
        Ok(result.text)
    }

    /// Collapse the selection by moving anchor to position.
    pub fn clear_selection(&self) {
        let mut d = self.data.lock();
        d.anchor = d.position;
    }

    // ── Boundary queries ─────────────────────────────────────

    /// True if the cursor is at the start of a block.
    pub fn at_block_start(&self) -> bool {
        let pos = self.position();
        let inner = self.doc.lock();
        let dto = frontend::document_inspection::GetBlockAtPositionDto {
            position: to_i64(pos),
        };
        if let Ok(info) = document_inspection_commands::get_block_at_position(&inner.ctx, &dto) {
            pos == to_usize(info.block_start)
        } else {
            false
        }
    }

    /// True if the cursor is at the end of a block.
    pub fn at_block_end(&self) -> bool {
        let pos = self.position();
        let inner = self.doc.lock();
        let dto = frontend::document_inspection::GetBlockAtPositionDto {
            position: to_i64(pos),
        };
        if let Ok(info) = document_inspection_commands::get_block_at_position(&inner.ctx, &dto) {
            pos == to_usize(info.block_start) + to_usize(info.block_length)
        } else {
            false
        }
    }

    /// True if the cursor is at position 0.
    pub fn at_start(&self) -> bool {
        self.data.lock().position == 0
    }

    /// True if the cursor is at the very end of the document.
    pub fn at_end(&self) -> bool {
        let pos = self.position();
        let inner = self.doc.lock();
        let stats =
            document_inspection_commands::get_document_stats(&inner.ctx).unwrap_or({
                frontend::document_inspection::DocumentStatsDto {
                    character_count: 0,
                    word_count: 0,
                    block_count: 0,
                    frame_count: 0,
                    image_count: 0,
                    list_count: 0,
                }
            });
        pos >= to_usize(stats.character_count)
    }

    /// The block number (0-indexed) containing the cursor.
    pub fn block_number(&self) -> usize {
        let pos = self.position();
        let inner = self.doc.lock();
        let dto = frontend::document_inspection::GetBlockAtPositionDto {
            position: to_i64(pos),
        };
        document_inspection_commands::get_block_at_position(&inner.ctx, &dto)
            .map(|info| to_usize(info.block_number))
            .unwrap_or(0)
    }

    /// The cursor's column within the current block (0-indexed).
    pub fn position_in_block(&self) -> usize {
        let pos = self.position();
        let inner = self.doc.lock();
        let dto = frontend::document_inspection::GetBlockAtPositionDto {
            position: to_i64(pos),
        };
        document_inspection_commands::get_block_at_position(&inner.ctx, &dto)
            .map(|info| pos.saturating_sub(to_usize(info.block_start)))
            .unwrap_or(0)
    }

    // ── Movement ─────────────────────────────────────────────

    /// Set the cursor to an absolute position.
    pub fn set_position(&self, position: usize, mode: MoveMode) {
        // Clamp to document length
        let end = {
            let inner = self.doc.lock();
            document_inspection_commands::get_document_stats(&inner.ctx)
                .map(|s| to_usize(s.character_count))
                .unwrap_or(0)
        };
        let pos = position.min(end);
        let mut d = self.data.lock();
        d.position = pos;
        if mode == MoveMode::MoveAnchor {
            d.anchor = pos;
        }
    }

    /// Move the cursor by a semantic operation.
    ///
    /// `n` is used as a repeat count for character-level movements
    /// (`NextCharacter`, `PreviousCharacter`, `Left`, `Right`).
    /// For all other operations it is ignored. Returns `true` if the cursor moved.
    pub fn move_position(&self, operation: MoveOperation, mode: MoveMode, n: usize) -> bool {
        let old_pos = self.position();
        let target = self.resolve_move(operation, n);
        self.set_position(target, mode);
        self.position() != old_pos
    }

    /// Select a region relative to the cursor position.
    pub fn select(&self, selection: SelectionType) {
        match selection {
            SelectionType::Document => {
                let end = {
                    let inner = self.doc.lock();
                    document_inspection_commands::get_document_stats(&inner.ctx)
                        .map(|s| to_usize(s.character_count))
                        .unwrap_or(0)
                };
                let mut d = self.data.lock();
                d.anchor = 0;
                d.position = end;
            }
            SelectionType::BlockUnderCursor | SelectionType::LineUnderCursor => {
                let pos = self.position();
                let inner = self.doc.lock();
                let dto = frontend::document_inspection::GetBlockAtPositionDto {
                    position: to_i64(pos),
                };
                if let Ok(info) =
                    document_inspection_commands::get_block_at_position(&inner.ctx, &dto)
                {
                    let start = to_usize(info.block_start);
                    let end = start + to_usize(info.block_length);
                    drop(inner);
                    let mut d = self.data.lock();
                    d.anchor = start;
                    d.position = end;
                }
            }
            SelectionType::WordUnderCursor => {
                let pos = self.position();
                let (word_start, word_end) = self.find_word_boundaries(pos);
                let mut d = self.data.lock();
                d.anchor = word_start;
                d.position = word_end;
            }
        }
    }

    // ── Text editing ─────────────────────────────────────────

    /// Insert plain text at the cursor. Replaces selection if any.
    pub fn insert_text(&self, text: &str) -> Result<()> {
        let (pos, anchor) = self.read_cursor();

        // Try direct insert first (handles same-block selection and no-selection cases)
        let dto = frontend::document_editing::InsertTextDto {
            position: to_i64(pos),
            anchor: to_i64(anchor),
            text: text.into(),
        };

        let queued = {
            let mut inner = self.doc.lock();
            let result = match document_editing_commands::insert_text(
                &inner.ctx,
                Some(inner.stack_id),
                &dto,
            ) {
                Ok(r) => r,
                Err(_) if pos != anchor => {
                    // Cross-block selection: compose delete + insert as a single undo unit
                    undo_redo_commands::begin_composite(&inner.ctx, Some(inner.stack_id));

                    let del_dto = frontend::document_editing::DeleteTextDto {
                        position: to_i64(pos),
                        anchor: to_i64(anchor),
                    };
                    let del_result = document_editing_commands::delete_text(
                        &inner.ctx,
                        Some(inner.stack_id),
                        &del_dto,
                    )?;
                    let del_pos = to_usize(del_result.new_position);

                    let ins_dto = frontend::document_editing::InsertTextDto {
                        position: to_i64(del_pos),
                        anchor: to_i64(del_pos),
                        text: text.into(),
                    };
                    let ins_result = document_editing_commands::insert_text(
                        &inner.ctx,
                        Some(inner.stack_id),
                        &ins_dto,
                    )?;

                    undo_redo_commands::end_composite(&inner.ctx);
                    ins_result
                }
                Err(e) => return Err(e),
            };

            let edit_pos = pos.min(anchor);
            let removed = pos.max(anchor) - edit_pos;
            self.finish_edit(
                &mut inner,
                edit_pos,
                removed,
                to_usize(result.new_position),
                to_usize(result.blocks_affected),
            )
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    /// Insert text with a specific character format. Replaces selection if any.
    pub fn insert_formatted_text(&self, text: &str, format: &TextFormat) -> Result<()> {
        let (pos, anchor) = self.read_cursor();
        let queued = {
            let mut inner = self.doc.lock();
            let dto = frontend::document_editing::InsertFormattedTextDto {
                position: to_i64(pos),
                anchor: to_i64(anchor),
                text: text.into(),
                font_family: format.font_family.clone().unwrap_or_default(),
                font_point_size: format.font_point_size.map(|v| v as i64).unwrap_or(0),
                font_bold: format.font_bold.unwrap_or(false),
                font_italic: format.font_italic.unwrap_or(false),
                font_underline: format.font_underline.unwrap_or(false),
                font_strikeout: format.font_strikeout.unwrap_or(false),
            };
            let result = document_editing_commands::insert_formatted_text(
                &inner.ctx,
                Some(inner.stack_id),
                &dto,
            )?;
            let edit_pos = pos.min(anchor);
            let removed = pos.max(anchor) - edit_pos;
            self.finish_edit(&mut inner, edit_pos, removed, to_usize(result.new_position), 1)
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    /// Insert a block break (new paragraph). Replaces selection if any.
    pub fn insert_block(&self) -> Result<()> {
        let (pos, anchor) = self.read_cursor();
        let queued = {
            let mut inner = self.doc.lock();
            let dto = frontend::document_editing::InsertBlockDto {
                position: to_i64(pos),
                anchor: to_i64(anchor),
            };
            let result =
                document_editing_commands::insert_block(&inner.ctx, Some(inner.stack_id), &dto)?;
            let edit_pos = pos.min(anchor);
            let removed = pos.max(anchor) - edit_pos;
            self.finish_edit(&mut inner, edit_pos, removed, to_usize(result.new_position), 2)
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    /// Insert an HTML fragment at the cursor position. Replaces selection if any.
    pub fn insert_html(&self, html: &str) -> Result<()> {
        let (pos, anchor) = self.read_cursor();
        let queued = {
            let mut inner = self.doc.lock();
            let dto = frontend::document_editing::InsertHtmlAtPositionDto {
                position: to_i64(pos),
                anchor: to_i64(anchor),
                html: html.into(),
            };
            let result = document_editing_commands::insert_html_at_position(
                &inner.ctx,
                Some(inner.stack_id),
                &dto,
            )?;
            let edit_pos = pos.min(anchor);
            let removed = pos.max(anchor) - edit_pos;
            self.finish_edit(
                &mut inner,
                edit_pos,
                removed,
                to_usize(result.new_position),
                to_usize(result.blocks_added),
            )
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    /// Insert a Markdown fragment at the cursor position. Replaces selection if any.
    pub fn insert_markdown(&self, markdown: &str) -> Result<()> {
        let (pos, anchor) = self.read_cursor();
        let queued = {
            let mut inner = self.doc.lock();
            let dto = frontend::document_editing::InsertMarkdownAtPositionDto {
                position: to_i64(pos),
                anchor: to_i64(anchor),
                markdown: markdown.into(),
            };
            let result = document_editing_commands::insert_markdown_at_position(
                &inner.ctx,
                Some(inner.stack_id),
                &dto,
            )?;
            let edit_pos = pos.min(anchor);
            let removed = pos.max(anchor) - edit_pos;
            self.finish_edit(
                &mut inner,
                edit_pos,
                removed,
                to_usize(result.new_position),
                to_usize(result.blocks_added),
            )
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    /// Insert a document fragment at the cursor. Replaces selection if any.
    pub fn insert_fragment(&self, fragment: &DocumentFragment) -> Result<()> {
        let (pos, anchor) = self.read_cursor();
        let queued = {
            let mut inner = self.doc.lock();
            let dto = frontend::document_editing::InsertFragmentDto {
                position: to_i64(pos),
                anchor: to_i64(anchor),
                fragment_data: fragment.raw_data().into(),
            };
            let result =
                document_editing_commands::insert_fragment(&inner.ctx, Some(inner.stack_id), &dto)?;
            let edit_pos = pos.min(anchor);
            let removed = pos.max(anchor) - edit_pos;
            self.finish_edit(
                &mut inner,
                edit_pos,
                removed,
                to_usize(result.new_position),
                to_usize(result.blocks_added),
            )
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    /// Extract the current selection as a [`DocumentFragment`].
    pub fn selection(&self) -> DocumentFragment {
        let (pos, anchor) = self.read_cursor();
        if pos == anchor {
            return DocumentFragment::new();
        }
        let inner = self.doc.lock();
        let dto = frontend::document_inspection::ExtractFragmentDto {
            position: to_i64(pos),
            anchor: to_i64(anchor),
        };
        match document_inspection_commands::extract_fragment(&inner.ctx, &dto) {
            Ok(result) => DocumentFragment::from_raw(result.fragment_data, result.plain_text),
            Err(_) => DocumentFragment::new(),
        }
    }

    /// Insert an image at the cursor.
    pub fn insert_image(&self, name: &str, width: u32, height: u32) -> Result<()> {
        let (pos, anchor) = self.read_cursor();
        let queued = {
            let mut inner = self.doc.lock();
            let dto = frontend::document_editing::InsertImageDto {
                position: to_i64(pos),
                anchor: to_i64(anchor),
                image_name: name.into(),
                width: width as i64,
                height: height as i64,
            };
            let result =
                document_editing_commands::insert_image(&inner.ctx, Some(inner.stack_id), &dto)?;
            let edit_pos = pos.min(anchor);
            let removed = pos.max(anchor) - edit_pos;
            self.finish_edit(&mut inner, edit_pos, removed, to_usize(result.new_position), 1)
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    /// Insert a new frame at the cursor.
    pub fn insert_frame(&self) -> Result<()> {
        let (pos, anchor) = self.read_cursor();
        let queued = {
            let mut inner = self.doc.lock();
            let dto = frontend::document_editing::InsertFrameDto {
                position: to_i64(pos),
                anchor: to_i64(anchor),
            };
            document_editing_commands::insert_frame(&inner.ctx, Some(inner.stack_id), &dto)?;
            // Frame insertion adds structural content; adjust cursors and emit event.
            // The backend doesn't return a new_position, so the cursor stays put.
            inner.modified = true;
            inner.invalidate_text_cache();
            inner.queue_event(DocumentEvent::ContentsChanged {
                position: pos.min(anchor),
                chars_removed: 0,
                chars_added: 0,
                blocks_affected: 1,
            });
            inner.check_block_count_changed();
            self.queue_undo_redo_event(&mut inner)
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    /// Delete the character after the cursor (Delete key).
    pub fn delete_char(&self) -> Result<()> {
        let (pos, anchor) = self.read_cursor();
        let (del_pos, del_anchor) = if pos != anchor {
            (pos, anchor)
        } else {
            (pos, pos + 1)
        };
        self.do_delete(del_pos, del_anchor)
    }

    /// Delete the character before the cursor (Backspace key).
    pub fn delete_previous_char(&self) -> Result<()> {
        let (pos, anchor) = self.read_cursor();
        let (del_pos, del_anchor) = if pos != anchor {
            (pos, anchor)
        } else if pos > 0 {
            (pos - 1, pos)
        } else {
            return Ok(());
        };
        self.do_delete(del_pos, del_anchor)
    }

    /// Delete the selected text. Returns the deleted text. No-op if no selection.
    pub fn remove_selected_text(&self) -> Result<String> {
        let (pos, anchor) = self.read_cursor();
        if pos == anchor {
            return Ok(String::new());
        }
        let queued = {
            let mut inner = self.doc.lock();
            let dto = frontend::document_editing::DeleteTextDto {
                position: to_i64(pos),
                anchor: to_i64(anchor),
            };
            let result =
                document_editing_commands::delete_text(&inner.ctx, Some(inner.stack_id), &dto)?;
            let edit_pos = pos.min(anchor);
            let removed = pos.max(anchor) - edit_pos;
            let new_pos = to_usize(result.new_position);
            inner.adjust_cursors(edit_pos, removed, 0);
            {
                let mut d = self.data.lock();
                d.position = new_pos;
                d.anchor = new_pos;
            }
            inner.modified = true;
            inner.invalidate_text_cache();
            inner.queue_event(DocumentEvent::ContentsChanged {
                position: edit_pos,
                chars_removed: removed,
                chars_added: 0,
                blocks_affected: 1,
            });
            inner.check_block_count_changed();
            // Return the deleted text alongside the queued events
            (result.deleted_text, self.queue_undo_redo_event(&mut inner))
        };
        crate::inner::dispatch_queued_events(queued.1);
        Ok(queued.0)
    }

    // ── List operations ──────────────────────────────────────

    /// Turn the block(s) in the selection into a list.
    pub fn create_list(&self, style: ListStyle) -> Result<()> {
        let (pos, anchor) = self.read_cursor();
        let queued = {
            let mut inner = self.doc.lock();
            let dto = frontend::document_editing::CreateListDto {
                position: to_i64(pos),
                anchor: to_i64(anchor),
                style: style.clone(),
            };
            document_editing_commands::create_list(&inner.ctx, Some(inner.stack_id), &dto)?;
            inner.modified = true;
            inner.queue_event(DocumentEvent::ContentsChanged {
                position: pos.min(anchor),
                chars_removed: 0,
                chars_added: 0,
                blocks_affected: 1,
            });
            self.queue_undo_redo_event(&mut inner)
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    /// Insert a new list item at the cursor position.
    pub fn insert_list(&self, style: ListStyle) -> Result<()> {
        let (pos, anchor) = self.read_cursor();
        let queued = {
            let mut inner = self.doc.lock();
            let dto = frontend::document_editing::InsertListDto {
                position: to_i64(pos),
                anchor: to_i64(anchor),
                style: style.clone(),
            };
            let result =
                document_editing_commands::insert_list(&inner.ctx, Some(inner.stack_id), &dto)?;
            let edit_pos = pos.min(anchor);
            let removed = pos.max(anchor) - edit_pos;
            self.finish_edit(&mut inner, edit_pos, removed, to_usize(result.new_position), 1)
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    // ── Format queries ───────────────────────────────────────

    /// Get the character format at the cursor position.
    pub fn char_format(&self) -> Result<TextFormat> {
        let pos = self.position();
        let inner = self.doc.lock();
        let dto = frontend::document_inspection::GetTextAtPositionDto {
            position: to_i64(pos),
            length: 1,
        };
        let text_info = document_inspection_commands::get_text_at_position(&inner.ctx, &dto)?;
        let element_id = text_info.element_id as u64;
        let element = inline_element_commands::get_inline_element(&inner.ctx, &element_id)?
            .ok_or_else(|| anyhow::anyhow!("element not found at position"))?;
        Ok(TextFormat::from(&element))
    }

    /// Get the block format of the block containing the cursor.
    pub fn block_format(&self) -> Result<BlockFormat> {
        let pos = self.position();
        let inner = self.doc.lock();
        let dto = frontend::document_inspection::GetBlockAtPositionDto {
            position: to_i64(pos),
        };
        let block_info = document_inspection_commands::get_block_at_position(&inner.ctx, &dto)?;
        let block_id = block_info.block_id as u64;
        let block = frontend::commands::block_commands::get_block(&inner.ctx, &block_id)?
            .ok_or_else(|| anyhow::anyhow!("block not found"))?;
        Ok(BlockFormat::from(&block))
    }

    // ── Format application ───────────────────────────────────

    /// Set the character format for the selection.
    pub fn set_char_format(&self, format: &TextFormat) -> Result<()> {
        let (pos, anchor) = self.read_cursor();
        let queued = {
            let mut inner = self.doc.lock();
            let dto = format.to_set_dto(pos, anchor);
            document_formatting_commands::set_text_format(&inner.ctx, Some(inner.stack_id), &dto)?;
            let start = pos.min(anchor);
            let length = pos.max(anchor) - start;
            inner.modified = true;
            inner.queue_event(DocumentEvent::FormatChanged {
                position: start,
                length,
            });
            self.queue_undo_redo_event(&mut inner)
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    /// Merge a character format into the selection.
    pub fn merge_char_format(&self, format: &TextFormat) -> Result<()> {
        let (pos, anchor) = self.read_cursor();
        let queued = {
            let mut inner = self.doc.lock();
            let dto = format.to_merge_dto(pos, anchor);
            document_formatting_commands::merge_text_format(
                &inner.ctx,
                Some(inner.stack_id),
                &dto,
            )?;
            let start = pos.min(anchor);
            let length = pos.max(anchor) - start;
            inner.modified = true;
            inner.queue_event(DocumentEvent::FormatChanged {
                position: start,
                length,
            });
            self.queue_undo_redo_event(&mut inner)
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    /// Set the block format for the current block (or all blocks in selection).
    pub fn set_block_format(&self, format: &BlockFormat) -> Result<()> {
        let (pos, anchor) = self.read_cursor();
        let queued = {
            let mut inner = self.doc.lock();
            let dto = format.to_set_dto(pos, anchor);
            document_formatting_commands::set_block_format(
                &inner.ctx,
                Some(inner.stack_id),
                &dto,
            )?;
            let start = pos.min(anchor);
            let length = pos.max(anchor) - start;
            inner.modified = true;
            inner.queue_event(DocumentEvent::FormatChanged {
                position: start,
                length,
            });
            self.queue_undo_redo_event(&mut inner)
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    /// Set the frame format.
    pub fn set_frame_format(&self, frame_id: usize, format: &FrameFormat) -> Result<()> {
        let (pos, anchor) = self.read_cursor();
        let queued = {
            let mut inner = self.doc.lock();
            let dto = format.to_set_dto(pos, anchor, frame_id);
            document_formatting_commands::set_frame_format(
                &inner.ctx,
                Some(inner.stack_id),
                &dto,
            )?;
            let start = pos.min(anchor);
            let length = pos.max(anchor) - start;
            inner.modified = true;
            inner.queue_event(DocumentEvent::FormatChanged {
                position: start,
                length,
            });
            self.queue_undo_redo_event(&mut inner)
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    // ── Edit blocks (composite undo) ─────────────────────────

    /// Begin a group of operations that will be undone as a single unit.
    pub fn begin_edit_block(&self) {
        let inner = self.doc.lock();
        undo_redo_commands::begin_composite(&inner.ctx, Some(inner.stack_id));
    }

    /// End the current edit block.
    pub fn end_edit_block(&self) {
        let inner = self.doc.lock();
        undo_redo_commands::end_composite(&inner.ctx);
    }

    /// Alias for [`begin_edit_block`](Self::begin_edit_block).
    ///
    /// Semantically indicates that the new composite should be merged with
    /// the previous one (e.g., consecutive keystrokes grouped into a single
    /// undo unit). The current backend treats this identically to
    /// `begin_edit_block`; future versions may implement automatic merging.
    pub fn join_previous_edit_block(&self) {
        self.begin_edit_block();
    }

    // ── Private helpers ─────────────────────────────────────

    /// Queue an `UndoRedoChanged` event and return all queued events for dispatch.
    fn queue_undo_redo_event(
        &self,
        inner: &mut TextDocumentInner,
    ) -> Vec<(DocumentEvent, Vec<Arc<dyn Fn(DocumentEvent) + Send + Sync>>)> {
        let can_undo =
            undo_redo_commands::can_undo(&inner.ctx, Some(inner.stack_id));
        let can_redo =
            undo_redo_commands::can_redo(&inner.ctx, Some(inner.stack_id));
        inner.queue_event(DocumentEvent::UndoRedoChanged { can_undo, can_redo });
        inner.take_queued_events()
    }

    fn do_delete(&self, pos: usize, anchor: usize) -> Result<()> {
        let queued = {
            let mut inner = self.doc.lock();
            let dto = frontend::document_editing::DeleteTextDto {
                position: to_i64(pos),
                anchor: to_i64(anchor),
            };
            let result =
                document_editing_commands::delete_text(&inner.ctx, Some(inner.stack_id), &dto)?;
            let edit_pos = pos.min(anchor);
            let removed = pos.max(anchor) - edit_pos;
            let new_pos = to_usize(result.new_position);
            inner.adjust_cursors(edit_pos, removed, 0);
            {
                let mut d = self.data.lock();
                d.position = new_pos;
                d.anchor = new_pos;
            }
            inner.modified = true;
            inner.invalidate_text_cache();
            inner.queue_event(DocumentEvent::ContentsChanged {
                position: edit_pos,
                chars_removed: removed,
                chars_added: 0,
                blocks_affected: 1,
            });
            inner.check_block_count_changed();
            self.queue_undo_redo_event(&mut inner)
        };
        crate::inner::dispatch_queued_events(queued);
        Ok(())
    }

    /// Resolve a MoveOperation to a concrete position.
    fn resolve_move(&self, op: MoveOperation, n: usize) -> usize {
        let pos = self.position();
        match op {
            MoveOperation::NoMove => pos,
            MoveOperation::Start => 0,
            MoveOperation::End => {
                let inner = self.doc.lock();
                document_inspection_commands::get_document_stats(&inner.ctx)
                    .map(|s| to_usize(s.character_count))
                    .unwrap_or(pos)
            }
            MoveOperation::NextCharacter | MoveOperation::Right => pos + n,
            MoveOperation::PreviousCharacter | MoveOperation::Left => pos.saturating_sub(n),
            MoveOperation::StartOfBlock | MoveOperation::StartOfLine => {
                let inner = self.doc.lock();
                let dto = frontend::document_inspection::GetBlockAtPositionDto {
                    position: to_i64(pos),
                };
                document_inspection_commands::get_block_at_position(&inner.ctx, &dto)
                    .map(|info| to_usize(info.block_start))
                    .unwrap_or(pos)
            }
            MoveOperation::EndOfBlock | MoveOperation::EndOfLine => {
                let inner = self.doc.lock();
                let dto = frontend::document_inspection::GetBlockAtPositionDto {
                    position: to_i64(pos),
                };
                document_inspection_commands::get_block_at_position(&inner.ctx, &dto)
                    .map(|info| to_usize(info.block_start) + to_usize(info.block_length))
                    .unwrap_or(pos)
            }
            MoveOperation::NextBlock => {
                let inner = self.doc.lock();
                let dto = frontend::document_inspection::GetBlockAtPositionDto {
                    position: to_i64(pos),
                };
                document_inspection_commands::get_block_at_position(&inner.ctx, &dto)
                    .map(|info| {
                        // Move past current block + 1 (block separator)
                        to_usize(info.block_start) + to_usize(info.block_length) + 1
                    })
                    .unwrap_or(pos)
            }
            MoveOperation::PreviousBlock => {
                let inner = self.doc.lock();
                let dto = frontend::document_inspection::GetBlockAtPositionDto {
                    position: to_i64(pos),
                };
                let block_start =
                    document_inspection_commands::get_block_at_position(&inner.ctx, &dto)
                        .map(|info| to_usize(info.block_start))
                        .unwrap_or(pos);
                if block_start >= 2 {
                    // Skip past the block separator (which maps to the current block)
                    let prev_dto = frontend::document_inspection::GetBlockAtPositionDto {
                        position: to_i64(block_start - 2),
                    };
                    document_inspection_commands::get_block_at_position(&inner.ctx, &prev_dto)
                        .map(|info| to_usize(info.block_start))
                        .unwrap_or(0)
                } else {
                    0
                }
            }
            MoveOperation::NextWord | MoveOperation::EndOfWord | MoveOperation::WordRight => {
                let (_, end) = self.find_word_boundaries(pos);
                // Move past the word end to the next word
                if end == pos {
                    // Already at a boundary, skip whitespace
                    let inner = self.doc.lock();
                    let stats = document_inspection_commands::get_document_stats(&inner.ctx)
                        .map(|s| to_usize(s.character_count))
                        .unwrap_or(0);
                    let scan_len = (stats - pos).min(64);
                    if scan_len == 0 {
                        return pos;
                    }
                    let dto = frontend::document_inspection::GetTextAtPositionDto {
                        position: to_i64(pos),
                        length: to_i64(scan_len),
                    };
                    if let Ok(r) =
                        document_inspection_commands::get_text_at_position(&inner.ctx, &dto)
                    {
                        for (i, ch) in r.text.chars().enumerate() {
                            if ch.is_alphanumeric() || ch == '_' {
                                // Found start of next word, find its end
                                let word_pos = pos + i;
                                drop(inner);
                                let (_, word_end) = self.find_word_boundaries(word_pos);
                                return word_end;
                            }
                        }
                    }
                    pos + scan_len
                } else {
                    end
                }
            }
            MoveOperation::PreviousWord | MoveOperation::StartOfWord | MoveOperation::WordLeft => {
                let (start, _) = self.find_word_boundaries(pos);
                if start < pos {
                    start
                } else if pos > 0 {
                    // Cursor is at a word start or on whitespace — scan backwards
                    // to find the start of the previous word.
                    let mut search = pos - 1;
                    loop {
                        let (ws, we) = self.find_word_boundaries(search);
                        if ws < we {
                            // Found a word; return its start
                            break ws;
                        }
                        // Still on whitespace/non-word; keep scanning
                        if search == 0 {
                            break 0;
                        }
                        search -= 1;
                    }
                } else {
                    0
                }
            }
            MoveOperation::Up | MoveOperation::Down => {
                // Up/Down are visual operations that depend on line wrapping.
                // Without layout info, treat as PreviousBlock/NextBlock.
                if matches!(op, MoveOperation::Up) {
                    self.resolve_move(MoveOperation::PreviousBlock, 1)
                } else {
                    self.resolve_move(MoveOperation::NextBlock, 1)
                }
            }
        }
    }

    /// Find the word boundaries around `pos`. Returns (start, end).
    /// Uses Unicode word segmentation for correct handling of non-ASCII text.
    ///
    /// Single-pass: tracks the last word seen to avoid a second iteration
    /// when the cursor is at the end of the last word (ISSUE-18).
    fn find_word_boundaries(&self, pos: usize) -> (usize, usize) {
        let inner = self.doc.lock();
        // Get block info so we can fetch the full block text
        let block_dto = frontend::document_inspection::GetBlockAtPositionDto {
            position: to_i64(pos),
        };
        let block_info =
            match document_inspection_commands::get_block_at_position(&inner.ctx, &block_dto) {
                Ok(info) => info,
                Err(_) => return (pos, pos),
            };

        let block_start = to_usize(block_info.block_start);
        let block_length = to_usize(block_info.block_length);
        if block_length == 0 {
            return (pos, pos);
        }

        let dto = frontend::document_inspection::GetTextAtPositionDto {
            position: to_i64(block_start),
            length: to_i64(block_length),
        };
        let text = match document_inspection_commands::get_text_at_position(&inner.ctx, &dto) {
            Ok(r) => r.text,
            Err(_) => return (pos, pos),
        };

        // cursor_offset is the char offset within the block text
        let cursor_offset = pos.saturating_sub(block_start);

        // Single pass: track the last word seen for end-of-last-word check
        let mut last_char_start = 0;
        let mut last_char_end = 0;

        for (word_byte_start, word) in text.unicode_word_indices() {
            // Convert byte offset to char offset
            let word_char_start = text[..word_byte_start].chars().count();
            let word_char_len = word.chars().count();
            let word_char_end = word_char_start + word_char_len;

            last_char_start = word_char_start;
            last_char_end = word_char_end;

            if cursor_offset >= word_char_start && cursor_offset < word_char_end {
                return (block_start + word_char_start, block_start + word_char_end);
            }
        }

        // Check if cursor is exactly at the end of the last word
        if cursor_offset == last_char_end && last_char_start < last_char_end {
            return (block_start + last_char_start, block_start + last_char_end);
        }

        (pos, pos)
    }
}
