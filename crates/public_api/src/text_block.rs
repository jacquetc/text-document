//! Read-only block (paragraph) handle.

use std::sync::Arc;

use parking_lot::Mutex;

use frontend::commands::{block_commands, frame_commands, inline_element_commands, list_commands};
use frontend::common::types::EntityId;
use frontend::inline_element::dtos::InlineContent;

use crate::convert::to_usize;
use crate::flow::{BlockSnapshot, FragmentContent, ListInfo, TableCellContext, TableCellRef};
use crate::inner::TextDocumentInner;
use crate::text_frame::TextFrame;
use crate::text_list::TextList;
use crate::text_table::TextTable;
use crate::{BlockFormat, ListStyle, TextFormat};

/// A lightweight, read-only handle to a single block (paragraph).
///
/// Holds a stable entity ID — the handle remains valid across edits
/// that insert or remove other blocks. Each method acquires the
/// document lock independently. For consistent reads across multiple
/// fields, use [`snapshot()`](TextBlock::snapshot).
#[derive(Clone)]
pub struct TextBlock {
    pub(crate) doc: Arc<Mutex<TextDocumentInner>>,
    pub(crate) block_id: usize,
}

impl TextBlock {
    // ── Content ──────────────────────────────────────────────

    /// Block's plain text. O(1).
    pub fn text(&self) -> String {
        let inner = self.doc.lock();
        block_commands::get_block(&inner.ctx, &(self.block_id as u64))
            .ok()
            .flatten()
            .map(|b| b.plain_text)
            .unwrap_or_default()
    }

    /// Character count. O(1).
    pub fn length(&self) -> usize {
        let inner = self.doc.lock();
        block_commands::get_block(&inner.ctx, &(self.block_id as u64))
            .ok()
            .flatten()
            .map(|b| to_usize(b.text_length))
            .unwrap_or(0)
    }

    /// `length() == 0`. O(1).
    pub fn is_empty(&self) -> bool {
        let inner = self.doc.lock();
        block_commands::get_block(&inner.ctx, &(self.block_id as u64))
            .ok()
            .flatten()
            .map(|b| b.text_length == 0)
            .unwrap_or(true)
    }

    /// Block entity still exists in the database. O(1).
    pub fn is_valid(&self) -> bool {
        let inner = self.doc.lock();
        block_commands::get_block(&inner.ctx, &(self.block_id as u64))
            .ok()
            .flatten()
            .is_some()
    }

    // ── Identity and Position ────────────────────────────────

    /// Stable entity ID (stored in the handle). O(1).
    pub fn id(&self) -> usize {
        self.block_id
    }

    /// Character offset from `Block.document_position`. O(1).
    pub fn position(&self) -> usize {
        let inner = self.doc.lock();
        block_commands::get_block(&inner.ctx, &(self.block_id as u64))
            .ok()
            .flatten()
            .map(|b| to_usize(b.document_position))
            .unwrap_or(0)
    }

    /// Global 0-indexed block number. **O(n)**: requires scanning all blocks
    /// sorted by `document_position`. Prefer [`id()`](TextBlock::id) for
    /// identity and [`position()`](TextBlock::position) for ordering.
    pub fn block_number(&self) -> usize {
        let inner = self.doc.lock();
        compute_block_number(&inner, self.block_id as u64)
    }

    /// The next block in document order. **O(n)**.
    /// Returns `None` if this is the last block.
    pub fn next(&self) -> Option<TextBlock> {
        let inner = self.doc.lock();
        let all_blocks = block_commands::get_all_block(&inner.ctx).ok()?;
        let mut sorted: Vec<_> = all_blocks.into_iter().collect();
        sorted.sort_by_key(|b| b.document_position);
        let idx = sorted.iter().position(|b| b.id == self.block_id as u64)?;
        sorted.get(idx + 1).map(|b| TextBlock {
            doc: Arc::clone(&self.doc),
            block_id: b.id as usize,
        })
    }

    /// The previous block in document order. **O(n)**.
    /// Returns `None` if this is the first block.
    pub fn previous(&self) -> Option<TextBlock> {
        let inner = self.doc.lock();
        let all_blocks = block_commands::get_all_block(&inner.ctx).ok()?;
        let mut sorted: Vec<_> = all_blocks.into_iter().collect();
        sorted.sort_by_key(|b| b.document_position);
        let idx = sorted.iter().position(|b| b.id == self.block_id as u64)?;
        if idx == 0 {
            return None;
        }
        sorted.get(idx - 1).map(|b| TextBlock {
            doc: Arc::clone(&self.doc),
            block_id: b.id as usize,
        })
    }

    // ── Structural Context ───────────────────────────────────

    /// Parent frame. O(1).
    pub fn frame(&self) -> TextFrame {
        let inner = self.doc.lock();
        let frame_id = find_parent_frame(&inner, self.block_id as u64);
        TextFrame {
            doc: Arc::clone(&self.doc),
            frame_id: frame_id.map(|id| id as usize).unwrap_or(0),
        }
    }

    /// If inside a table cell, returns table and cell coordinates.
    ///
    /// Finds the block's parent frame, then checks if any table cell
    /// references that frame as its `cell_frame`. If so, identifies the
    /// owning table.
    pub fn table_cell(&self) -> Option<TableCellRef> {
        let inner = self.doc.lock();
        let frame_id = find_parent_frame(&inner, self.block_id as u64)?;

        // Check if this frame is referenced as a cell_frame by any table cell.
        // First try the fast path: if the frame has a `table` field, use it.
        let frame_dto = frame_commands::get_frame(&inner.ctx, &frame_id)
            .ok()
            .flatten()?;

        if let Some(table_entity_id) = frame_dto.table {
            // This frame is a table anchor frame (not a cell frame).
            // Anchor frames don't contain blocks directly — cell frames do.
            // So this path shouldn't match, but check cells just in case.
            let table_dto =
                frontend::commands::table_commands::get_table(&inner.ctx, &{ table_entity_id })
                    .ok()
                    .flatten()?;
            for &cell_id in &table_dto.cells {
                if let Some(cell_dto) =
                    frontend::commands::table_cell_commands::get_table_cell(&inner.ctx, &{
                        cell_id
                    })
                    .ok()
                    .flatten()
                    && cell_dto.cell_frame == Some(frame_id)
                {
                    return Some(TableCellRef {
                        table: TextTable {
                            doc: Arc::clone(&self.doc),
                            table_id: table_entity_id as usize,
                        },
                        row: to_usize(cell_dto.row),
                        column: to_usize(cell_dto.column),
                    });
                }
            }
        }

        // Slow path: this frame has no `table` field (cell frames don't).
        // Scan all tables to find if any cell references this frame.
        let all_tables =
            frontend::commands::table_commands::get_all_table(&inner.ctx).unwrap_or_default();
        for table_dto in &all_tables {
            for &cell_id in &table_dto.cells {
                if let Some(cell_dto) =
                    frontend::commands::table_cell_commands::get_table_cell(&inner.ctx, &{
                        cell_id
                    })
                    .ok()
                    .flatten()
                    && cell_dto.cell_frame == Some(frame_id)
                {
                    return Some(TableCellRef {
                        table: TextTable {
                            doc: Arc::clone(&self.doc),
                            table_id: table_dto.id as usize,
                        },
                        row: to_usize(cell_dto.row),
                        column: to_usize(cell_dto.column),
                    });
                }
            }
        }

        None
    }

    // ── Formatting ──────────────────────────────────────────

    /// Block format (alignment, margins, indent, heading level, marker, tabs). O(1).
    pub fn block_format(&self) -> BlockFormat {
        let inner = self.doc.lock();
        block_commands::get_block(&inner.ctx, &(self.block_id as u64))
            .ok()
            .flatten()
            .map(|b| BlockFormat::from(&b))
            .unwrap_or_default()
    }

    /// Character format at a block-relative character offset. **O(k)**
    /// where k = number of InlineElements.
    ///
    /// Returns the [`TextFormat`] of the fragment containing the given
    /// offset. Returns `None` if the offset is out of range or the
    /// block has no fragments.
    pub fn char_format_at(&self, offset: usize) -> Option<TextFormat> {
        let inner = self.doc.lock();
        let fragments = build_fragments(&inner, self.block_id as u64);
        for frag in &fragments {
            match frag {
                FragmentContent::Text {
                    format,
                    offset: frag_offset,
                    length,
                    ..
                } => {
                    if offset >= *frag_offset && offset < frag_offset + length {
                        return Some(format.clone());
                    }
                }
                FragmentContent::Image {
                    format,
                    offset: frag_offset,
                    ..
                } => {
                    if offset == *frag_offset {
                        return Some(format.clone());
                    }
                }
            }
        }
        None
    }

    // ── Fragments ───────────────────────────────────────────

    /// All formatting runs in one call. O(k) where k = number of InlineElements.
    pub fn fragments(&self) -> Vec<FragmentContent> {
        let inner = self.doc.lock();
        build_fragments(&inner, self.block_id as u64)
    }

    // ── List Membership ─────────────────────────────────────

    /// List this block belongs to. O(1).
    pub fn list(&self) -> Option<TextList> {
        let inner = self.doc.lock();
        let block_dto = block_commands::get_block(&inner.ctx, &(self.block_id as u64))
            .ok()
            .flatten()?;
        let list_id = block_dto.list?;
        Some(TextList {
            doc: Arc::clone(&self.doc),
            list_id: list_id as usize,
        })
    }

    /// 0-based position within its list. **O(n)** where n = total blocks.
    pub fn list_item_index(&self) -> Option<usize> {
        let inner = self.doc.lock();
        let block_dto = block_commands::get_block(&inner.ctx, &(self.block_id as u64))
            .ok()
            .flatten()?;
        let list_id = block_dto.list?;
        Some(compute_list_item_index(
            &inner,
            list_id,
            self.block_id as u64,
        ))
    }

    // ── Snapshot ─────────────────────────────────────────────

    /// All layout-relevant data in one lock acquisition. O(k+n).
    pub fn snapshot(&self) -> BlockSnapshot {
        let inner = self.doc.lock();
        build_block_snapshot(&inner, self.block_id as u64).unwrap_or_else(|| BlockSnapshot {
            block_id: self.block_id,
            position: 0,
            length: 0,
            text: String::new(),
            fragments: Vec::new(),
            block_format: BlockFormat::default(),
            list_info: None,
            parent_frame_id: None,
            table_cell: None,
        })
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Internal helpers (called while lock is held)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Find the parent frame of a block by scanning all frames.
fn find_parent_frame(inner: &TextDocumentInner, block_id: u64) -> Option<EntityId> {
    let all_frames = frame_commands::get_all_frame(&inner.ctx).ok()?;
    let block_entity_id = block_id as EntityId;
    for frame in &all_frames {
        if frame.blocks.contains(&block_entity_id) {
            return Some(frame.id as EntityId);
        }
    }
    None
}

/// Find table cell context for a block (snapshot-friendly, no live handles).
/// Returns `None` if the block is not inside a table cell.
fn find_table_cell_context(inner: &TextDocumentInner, block_id: u64) -> Option<TableCellContext> {
    let frame_id = find_parent_frame(inner, block_id)?;

    let frame_dto = frame_commands::get_frame(&inner.ctx, &frame_id)
        .ok()
        .flatten()?;

    // Fast path: anchor frame with `table` field set
    if let Some(table_entity_id) = frame_dto.table {
        let table_dto =
            frontend::commands::table_commands::get_table(&inner.ctx, &{ table_entity_id })
                .ok()
                .flatten()?;
        for &cell_id in &table_dto.cells {
            if let Some(cell_dto) =
                frontend::commands::table_cell_commands::get_table_cell(&inner.ctx, &{ cell_id })
                    .ok()
                    .flatten()
                && cell_dto.cell_frame == Some(frame_id)
            {
                return Some(TableCellContext {
                    table_id: table_entity_id as usize,
                    row: to_usize(cell_dto.row),
                    column: to_usize(cell_dto.column),
                });
            }
        }
    }

    // Slow path: scan all tables for a cell referencing this frame
    let all_tables =
        frontend::commands::table_commands::get_all_table(&inner.ctx).unwrap_or_default();
    for table_dto in &all_tables {
        for &cell_id in &table_dto.cells {
            if let Some(cell_dto) =
                frontend::commands::table_cell_commands::get_table_cell(&inner.ctx, &{ cell_id })
                    .ok()
                    .flatten()
                && cell_dto.cell_frame == Some(frame_id)
            {
                return Some(TableCellContext {
                    table_id: table_dto.id as usize,
                    row: to_usize(cell_dto.row),
                    column: to_usize(cell_dto.column),
                });
            }
        }
    }

    None
}

/// Compute 0-indexed block number by scanning all blocks sorted by document_position.
fn compute_block_number(inner: &TextDocumentInner, block_id: u64) -> usize {
    let all_blocks = block_commands::get_all_block(&inner.ctx).unwrap_or_default();
    let mut sorted: Vec<_> = all_blocks.iter().collect();
    sorted.sort_by_key(|b| b.document_position);
    sorted.iter().position(|b| b.id == block_id).unwrap_or(0)
}

/// Build fragments for a block from its InlineElements, with highlight
/// spans merged in when a syntax highlighter is attached.
pub(crate) fn build_fragments(inner: &TextDocumentInner, block_id: u64) -> Vec<FragmentContent> {
    let fragments = build_raw_fragments(inner, block_id);

    if let Some(ref hl) = inner.highlight
        && let Some(block_hl) = hl.blocks.get(&(block_id as usize))
        && !block_hl.spans.is_empty()
    {
        return crate::highlight::merge_highlight_spans(fragments, &block_hl.spans);
    }

    fragments
}

/// Build raw fragments from InlineElements (no highlight merge).
fn build_raw_fragments(inner: &TextDocumentInner, block_id: u64) -> Vec<FragmentContent> {
    let block_dto = match block_commands::get_block(&inner.ctx, &block_id)
        .ok()
        .flatten()
    {
        Some(b) => b,
        None => return Vec::new(),
    };

    let element_ids = &block_dto.elements;
    let elements: Vec<_> = element_ids
        .iter()
        .filter_map(|&id| {
            inline_element_commands::get_inline_element(&inner.ctx, &{ id })
                .ok()
                .flatten()
        })
        .collect();

    let mut fragments = Vec::with_capacity(elements.len());
    let mut offset: usize = 0;

    for el in &elements {
        let format = TextFormat::from(el);
        match &el.content {
            InlineContent::Text(text) => {
                let length = text.chars().count();
                fragments.push(FragmentContent::Text {
                    text: text.clone(),
                    format,
                    offset,
                    length,
                });
                offset += length;
            }
            InlineContent::Image {
                name,
                width,
                height,
                quality,
            } => {
                fragments.push(FragmentContent::Image {
                    name: name.clone(),
                    width: *width as u32,
                    height: *height as u32,
                    quality: *quality as u32,
                    format,
                    offset,
                });
                offset += 1; // images take 1 character position
            }
            InlineContent::Empty => {
                // Empty elements don't produce fragments
            }
        }
    }

    fragments
}

/// Compute 0-based index of a block within its list.
fn compute_list_item_index(inner: &TextDocumentInner, list_id: EntityId, block_id: u64) -> usize {
    let all_blocks = block_commands::get_all_block(&inner.ctx).unwrap_or_default();
    let mut list_blocks: Vec<_> = all_blocks
        .iter()
        .filter(|b| b.list == Some(list_id))
        .collect();
    list_blocks.sort_by_key(|b| b.document_position);
    list_blocks
        .iter()
        .position(|b| b.id == block_id)
        .unwrap_or(0)
}

/// Format a list marker for the given item index.
pub(crate) fn format_list_marker(
    list_dto: &frontend::list::dtos::ListDto,
    item_index: usize,
) -> String {
    let number = item_index + 1; // 1-based for display
    let marker_body = match list_dto.style {
        ListStyle::Disc => "\u{2022}".to_string(),   // •
        ListStyle::Circle => "\u{25E6}".to_string(), // ◦
        ListStyle::Square => "\u{25AA}".to_string(), // ▪
        ListStyle::Decimal => format!("{number}"),
        ListStyle::LowerAlpha => {
            if number <= 26 {
                ((b'a' + (number as u8 - 1)) as char).to_string()
            } else {
                format!("{number}")
            }
        }
        ListStyle::UpperAlpha => {
            if number <= 26 {
                ((b'A' + (number as u8 - 1)) as char).to_string()
            } else {
                format!("{number}")
            }
        }
        ListStyle::LowerRoman => to_roman_lower(number),
        ListStyle::UpperRoman => to_roman_upper(number),
    };
    format!("{}{marker_body}{}", list_dto.prefix, list_dto.suffix)
}

fn to_roman_upper(mut n: usize) -> String {
    const VALUES: &[(usize, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut result = String::new();
    for &(val, sym) in VALUES {
        while n >= val {
            result.push_str(sym);
            n -= val;
        }
    }
    result
}

fn to_roman_lower(n: usize) -> String {
    to_roman_upper(n).to_lowercase()
}

/// Build a ListInfo for a block. Called while lock is held.
fn build_list_info(
    inner: &TextDocumentInner,
    block_dto: &frontend::block::dtos::BlockDto,
) -> Option<ListInfo> {
    let list_id = block_dto.list?;
    let list_dto = list_commands::get_list(&inner.ctx, &{ list_id })
        .ok()
        .flatten()?;

    let item_index = compute_list_item_index(inner, list_id, block_dto.id);
    let marker = format_list_marker(&list_dto, item_index);

    Some(ListInfo {
        list_id: list_id as usize,
        style: list_dto.style.clone(),
        indent: list_dto.indent as u8,
        marker,
        item_index,
    })
}

/// Build a BlockSnapshot for a block. Called while lock is held.
pub(crate) fn build_block_snapshot(
    inner: &TextDocumentInner,
    block_id: u64,
) -> Option<BlockSnapshot> {
    build_block_snapshot_with_position(inner, block_id, None)
}

/// Build a BlockSnapshot, optionally overriding the position with a computed value.
/// When `computed_position` is Some, it's used instead of `block_dto.document_position`
/// (which may be stale if position updates are deferred).
pub(crate) fn build_block_snapshot_with_position(
    inner: &TextDocumentInner,
    block_id: u64,
    computed_position: Option<usize>,
) -> Option<BlockSnapshot> {
    let block_dto = block_commands::get_block(&inner.ctx, &block_id)
        .ok()
        .flatten()?;

    let fragments = build_fragments(inner, block_id);
    let block_format = BlockFormat::from(&block_dto);
    let list_info = build_list_info(inner, &block_dto);

    let parent_frame_id = find_parent_frame(inner, block_id).map(|id| id as usize);
    let table_cell = find_table_cell_context(inner, block_id);

    let position = computed_position.unwrap_or_else(|| to_usize(block_dto.document_position));

    Some(BlockSnapshot {
        block_id: block_id as usize,
        position,
        length: to_usize(block_dto.text_length),
        text: block_dto.plain_text,
        fragments,
        block_format,
        list_info,
        parent_frame_id,
        table_cell,
    })
}

/// Build BlockSnapshots for all blocks in a frame, sorted by document_position.
pub(crate) fn build_blocks_snapshot_for_frame(
    inner: &TextDocumentInner,
    frame_id: u64,
) -> Vec<BlockSnapshot> {
    let frame_dto = match frame_commands::get_frame(&inner.ctx, &(frame_id as EntityId))
        .ok()
        .flatten()
    {
        Some(f) => f,
        None => return Vec::new(),
    };

    let mut block_dtos: Vec<_> = frame_dto
        .blocks
        .iter()
        .filter_map(|&id| {
            block_commands::get_block(&inner.ctx, &{ id })
                .ok()
                .flatten()
        })
        .collect();
    block_dtos.sort_by_key(|b| b.document_position);

    block_dtos
        .iter()
        .filter_map(|b| build_block_snapshot(inner, b.id))
        .collect()
}

/// Build BlockSnapshots with computed positions starting from `start_pos`.
///
/// Returns `(snapshots, running_pos_after_last_block)`.
/// Positions are computed sequentially from `start_pos` using each block's
/// `text_length`, matching the logic in `find_block_at_position_sequential`.
pub(crate) fn build_blocks_snapshot_for_frame_with_positions(
    inner: &TextDocumentInner,
    frame_id: u64,
    start_pos: usize,
) -> (Vec<BlockSnapshot>, usize) {
    let frame_dto = match frame_commands::get_frame(&inner.ctx, &(frame_id as EntityId))
        .ok()
        .flatten()
    {
        Some(f) => f,
        None => return (Vec::new(), start_pos),
    };

    let mut block_dtos: Vec<_> = frame_dto
        .blocks
        .iter()
        .filter_map(|&id| {
            block_commands::get_block(&inner.ctx, &{ id })
                .ok()
                .flatten()
        })
        .collect();
    block_dtos.sort_by_key(|b| b.document_position);

    let mut running_pos = start_pos;
    let mut snapshots = Vec::with_capacity(block_dtos.len());
    for b in &block_dtos {
        if let Some(snap) = build_block_snapshot_with_position(inner, b.id, Some(running_pos)) {
            running_pos += snap.length + 1; // +1 for block separator
            snapshots.push(snap);
        }
    }
    (snapshots, running_pos)
}
