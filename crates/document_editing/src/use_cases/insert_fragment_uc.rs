use super::editing_helpers::{
    CellFrameCreator, create_cell_frame, find_block_at_position, find_element_at_offset,
    impl_cell_frame_creator,
};
use crate::InsertFragmentDto;
use crate::InsertFragmentResultDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::direct_access::block::block_repository::BlockRelationshipField;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::direct_access::table::TableRelationshipField;
use common::entities::{
    Block, Document, Frame, InlineContent, InlineElement, List, Root, Table, TableCell,
};
use std::collections::HashMap;
use common::parser_tools::fragment_schema::{FragmentBlock, FragmentData, FragmentTable};
use common::parser_tools::list_grouper::ListGrouper;
use common::snapshot::EntityTreeSnapshot;
use common::types::{EntityId, ROOT_ENTITY_ID};
use common::undo_redo::UndoRedoCommand;
use std::any::Any;

pub trait InsertFragmentUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn InsertFragmentUnitOfWorkTrait>;
}

#[macros::uow_action(entity = "Root", action = "Get")]
#[macros::uow_action(entity = "Root", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Get")]
#[macros::uow_action(entity = "Document", action = "Update")]
#[macros::uow_action(entity = "Document", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Snapshot")]
#[macros::uow_action(entity = "Document", action = "Restore")]
#[macros::uow_action(entity = "Frame", action = "Get")]
#[macros::uow_action(entity = "Frame", action = "Update")]
#[macros::uow_action(entity = "Frame", action = "GetRelationship")]
#[macros::uow_action(entity = "Block", action = "Get")]
#[macros::uow_action(entity = "Block", action = "GetMulti")]
#[macros::uow_action(entity = "Block", action = "Update")]
#[macros::uow_action(entity = "Block", action = "UpdateMulti")]
#[macros::uow_action(entity = "Block", action = "Create")]
#[macros::uow_action(entity = "Block", action = "GetRelationship")]
#[macros::uow_action(entity = "InlineElement", action = "Get")]
#[macros::uow_action(entity = "InlineElement", action = "GetMulti")]
#[macros::uow_action(entity = "InlineElement", action = "Update")]
#[macros::uow_action(entity = "InlineElement", action = "Create")]
#[macros::uow_action(entity = "Block", action = "Remove")]
#[macros::uow_action(entity = "InlineElement", action = "Remove")]
#[macros::uow_action(entity = "InlineElement", action = "RemoveMulti")]
#[macros::uow_action(entity = "List", action = "Get")]
#[macros::uow_action(entity = "List", action = "Create")]
#[macros::uow_action(entity = "Frame", action = "Create")]
#[macros::uow_action(entity = "Table", action = "Get")]
#[macros::uow_action(entity = "Table", action = "Create")]
#[macros::uow_action(entity = "Table", action = "GetRelationship")]
#[macros::uow_action(entity = "TableCell", action = "GetMulti")]
#[macros::uow_action(entity = "TableCell", action = "Create")]
pub trait InsertFragmentUnitOfWorkTrait: CommandUnitOfWork {}

impl_cell_frame_creator!(dyn InsertFragmentUnitOfWorkTrait);

/// Build a mapping from block_id → (cell_frame_id, table_id) for all tables in the document.
fn build_block_to_cell_map(
    uow: &Box<dyn InsertFragmentUnitOfWorkTrait>,
    doc_id: EntityId,
) -> Result<HashMap<EntityId, (EntityId, EntityId)>> {
    let table_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Tables)?;
    let mut map: HashMap<EntityId, (EntityId, EntityId)> = HashMap::new();
    for &tid in &table_ids {
        let cell_ids = uow.get_table_relationship(&tid, &TableRelationshipField::Cells)?;
        let cells_opt = uow.get_table_cell_multi(&cell_ids)?;
        for cell in cells_opt.into_iter().flatten() {
            if let Some(cf_id) = cell.cell_frame {
                let blk_ids =
                    uow.get_frame_relationship(&cf_id, &FrameRelationshipField::Blocks)?;
                for bid in blk_ids {
                    map.insert(bid, (cf_id, tid));
                }
            }
        }
    }
    Ok(map)
}

/// Replace cell contents in an existing table with fragment data.
/// Returns Ok(Some(result)) if replacement was performed, Ok(None) if not applicable.
fn try_replace_table_cells(
    uow: &mut Box<dyn InsertFragmentUnitOfWorkTrait>,
    dto: &InsertFragmentDto,
    fragment_data: &FragmentData,
    doc_id: EntityId,
) -> Result<Option<(InsertFragmentResultDto, EntityTreeSnapshot)>> {
    // Only handle single-table fragments
    if fragment_data.tables.len() != 1 {
        return Ok(None);
    }
    let frag_table = &fragment_data.tables[0];

    // Build block→cell mapping to detect if cursor is inside a table
    let block_to_cell = build_block_to_cell_map(uow, doc_id)?;

    // Find the block at cursor position
    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
    let frame_id = *frame_ids
        .first()
        .ok_or_else(|| anyhow!("Document has no frames"))?;

    let block_ids = uow.get_frame_relationship(&frame_id, &FrameRelationshipField::Blocks)?;
    let blocks_opt = uow.get_block_multi(&block_ids)?;
    let mut all_blocks: Vec<Block> = blocks_opt.into_iter().flatten().collect();

    // Also collect cell blocks
    for &tid in &uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Tables)? {
        let cell_ids = uow.get_table_relationship(&tid, &TableRelationshipField::Cells)?;
        let cells = uow.get_table_cell_multi(&cell_ids)?;
        for cell in cells.into_iter().flatten() {
            if let Some(cf_id) = cell.cell_frame {
                let cf_blk_ids =
                    uow.get_frame_relationship(&cf_id, &FrameRelationshipField::Blocks)?;
                let cf_blks = uow.get_block_multi(&cf_blk_ids)?;
                all_blocks.extend(cf_blks.into_iter().flatten());
            }
        }
    }
    all_blocks.sort_by_key(|b| b.document_position);

    let (cursor_block, _, _) = find_block_at_position(&all_blocks, dto.position)?;

    // Check if cursor block is inside a table
    let target_table_id = match block_to_cell.get(&cursor_block.id) {
        Some((_, tid)) => *tid,
        None => return Ok(None), // cursor not in a table
    };

    // Get the target table's cells
    let target_table = uow
        .get_table(&target_table_id)?
        .ok_or_else(|| anyhow!("Target table not found"))?;
    let target_cell_ids =
        uow.get_table_relationship(&target_table_id, &TableRelationshipField::Cells)?;
    let target_cells_opt = uow.get_table_cell_multi(&target_cell_ids)?;
    let target_cells: Vec<TableCell> = target_cells_opt.into_iter().flatten().collect();

    // Check dimensions match (fragment fits within table)
    if frag_table.rows > target_table.rows as usize
        || frag_table.columns > target_table.columns as usize
    {
        return Ok(None); // fragment too large, fall back to new table
    }

    let now = chrono::Utc::now();
    let snapshot = uow.snapshot_document(&[doc_id])?;

    // Find the cursor's cell position to use as offset
    let cursor_cell = target_cells
        .iter()
        .find(|c| {
            c.cell_frame
                .is_some_and(|cf| block_to_cell.get(&cursor_block.id).is_some_and(|(cf2, _)| cf == *cf2))
        });
    let (base_row, base_col) = cursor_cell
        .map(|c| (c.row as usize, c.column as usize))
        .unwrap_or((0, 0));

    // Replace cell contents
    for frag_cell in &frag_table.cells {
        let target_row = base_row + frag_cell.row;
        let target_col = base_col + frag_cell.column;

        // Find the matching target cell
        let target = target_cells.iter().find(|c| {
            c.row as usize == target_row && c.column as usize == target_col
        });
        let target = match target {
            Some(t) => t,
            None => continue, // no matching cell, skip
        };

        let cf_id = match target.cell_frame {
            Some(id) => id,
            None => continue,
        };

        // Clear existing cell content
        let existing_blk_ids =
            uow.get_frame_relationship(&cf_id, &FrameRelationshipField::Blocks)?;
        let existing_blks_opt = uow.get_block_multi(&existing_blk_ids)?;
        let existing_blks: Vec<Block> = existing_blks_opt.into_iter().flatten().collect();

        // Remove all existing blocks except the first (which we'll update)
        for blk in existing_blks.iter().skip(1) {
            let elem_ids =
                uow.get_block_relationship(&blk.id, &BlockRelationshipField::Elements)?;
            uow.remove_inline_element_multi(&elem_ids)?;
            uow.remove_block(&blk.id)?;
        }

        if let Some(first_blk) = existing_blks.first() {
            // Clear existing elements from first block
            let elem_ids =
                uow.get_block_relationship(&first_blk.id, &BlockRelationshipField::Elements)?;
            uow.remove_inline_element_multi(&elem_ids)?;

            if let Some(first_frag_blk) = frag_cell.blocks.first() {
                // Update first block with fragment content
                let mut updated = first_blk.clone();
                updated.plain_text = first_frag_blk.plain_text.clone();
                updated.text_length = first_frag_blk.plain_text.chars().count() as i64;
                updated.updated_at = now;
                uow.update_block(&updated)?;

                // Create elements for first block
                for frag_elem in &first_frag_blk.elements {
                    let elem = frag_elem.to_entity();
                    uow.create_inline_element(&elem, first_blk.id, -1)?;
                }

                // Create additional blocks for multi-block cells
                for extra_frag in &frag_cell.blocks[1..] {
                    let extra_block = Block {
                        id: 0,
                        created_at: now,
                        updated_at: now,
                        elements: vec![],
                        list: None,
                        text_length: extra_frag.plain_text.chars().count() as i64,
                        document_position: 0, // will be reassigned
                        plain_text: extra_frag.plain_text.clone(),
                        ..Default::default()
                    };
                    let created = uow.create_block(&extra_block, cf_id, -1)?;
                    for frag_elem in &extra_frag.elements {
                        let elem = frag_elem.to_entity();
                        uow.create_inline_element(&elem, created.id, -1)?;
                    }
                }
            } else {
                // Empty fragment cell: clear the block
                let mut updated = first_blk.clone();
                updated.plain_text = String::new();
                updated.text_length = 0;
                updated.updated_at = now;
                uow.update_block(&updated)?;

                let empty_elem = InlineElement {
                    id: 0,
                    created_at: now,
                    updated_at: now,
                    content: InlineContent::Empty,
                    ..Default::default()
                };
                uow.create_inline_element(&empty_elem, first_blk.id, -1)?;
            }
        }
    }

    Ok(Some((
        InsertFragmentResultDto {
            new_position: dto.position,
            blocks_added: 0,
        },
        snapshot,
    )))
}

/// Insert a table-only fragment at the cursor position.
/// Creates one table per `FragmentTable` entry, each with its cells and content.
fn insert_table_fragment(
    uow: &mut Box<dyn InsertFragmentUnitOfWorkTrait>,
    dto: &InsertFragmentDto,
    fragment_data: &FragmentData,
) -> Result<(InsertFragmentResultDto, EntityTreeSnapshot)> {
    let now = chrono::Utc::now();

    let root = uow
        .get_root(&ROOT_ENTITY_ID)?
        .ok_or_else(|| anyhow!("Root entity not found"))?;
    let doc_ids = uow.get_root_relationship(&root.id, &RootRelationshipField::Document)?;
    let doc_id = *doc_ids
        .first()
        .ok_or_else(|| anyhow!("Root has no document"))?;

    // Try to replace cell contents in an existing table first (Word behavior)
    if let Some(result) = try_replace_table_cells(uow, dto, fragment_data, doc_id)? {
        return Ok(result);
    }

    let document = uow
        .get_document(&doc_id)?
        .ok_or_else(|| anyhow!("Document not found"))?;

    let snapshot = uow.snapshot_document(&[doc_id])?;

    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
    let frame_id = *frame_ids
        .first()
        .ok_or_else(|| anyhow!("Document has no frames"))?;

    // Collect all blocks to find insertion point
    let block_ids = uow.get_frame_relationship(&frame_id, &FrameRelationshipField::Blocks)?;
    let blocks_opt = uow.get_block_multi(&block_ids)?;
    let mut blocks: Vec<Block> = blocks_opt.into_iter().flatten().collect();
    blocks.sort_by_key(|b| b.document_position);

    let insert_pos = dto.position;

    // Find which block the cursor is in, to determine child_order insertion index
    let child_order_insert_idx = if blocks.is_empty() {
        0usize
    } else {
        let (target_block, _, _) = find_block_at_position(&blocks, insert_pos)?;
        let blk_ids = uow.get_frame_relationship(&frame_id, &FrameRelationshipField::Blocks)?;
        blk_ids
            .iter()
            .position(|&bid| bid == target_block.id)
            .map(|i| i + 1)
            .unwrap_or(0)
    };

    let mut total_blocks_added: i64 = 0;
    let mut current_child_idx = child_order_insert_idx;
    let mut current_pos = insert_pos;

    for frag_table in &fragment_data.tables {
        if frag_table.rows == 0 || frag_table.columns == 0 || frag_table.cells.is_empty() {
            continue; // skip degenerate table fragments
        }

        // Create the Table entity
        let table = Table {
            id: 0,
            created_at: now,
            updated_at: now,
            cells: vec![],
            rows: frag_table.rows as i64,
            columns: frag_table.columns as i64,
            column_widths: if frag_table.column_widths.is_empty() {
                vec![0; frag_table.columns]
            } else {
                frag_table.column_widths.clone()
            },
            fmt_border: frag_table.fmt_border,
            fmt_cell_spacing: frag_table.fmt_cell_spacing,
            fmt_cell_padding: frag_table.fmt_cell_padding,
            fmt_width: frag_table.fmt_width,
            fmt_alignment: frag_table.fmt_alignment.clone(),
        };
        let created_table = uow.create_table(&table, doc_id, -1)?;

        // Create cells with content
        let mut cell_blocks_to_update: Vec<Block> = Vec::new();

        for frag_cell in &frag_table.cells {
            // Create the cell frame
            let (cell_frame_id, created_block) = create_cell_frame(uow, doc_id, now)?;

            // If the fragment cell has content, populate it
            if !frag_cell.blocks.is_empty() {
                let first_frag = &frag_cell.blocks[0];
                // Update the created block with the first fragment block's content
                let mut updated_block = created_block.clone();
                updated_block.plain_text = first_frag.plain_text.clone();
                updated_block.text_length = first_frag.plain_text.chars().count() as i64;
                updated_block.document_position = current_pos;
                updated_block.updated_at = now;
                cell_blocks_to_update.push(updated_block);

                // Create elements for the first block
                for frag_elem in &first_frag.elements {
                    let elem = frag_elem.to_entity();
                    uow.create_inline_element(&elem, created_block.id, -1)?;
                }

                current_pos += first_frag.plain_text.chars().count() as i64 + 1;
                total_blocks_added += 1;

                // Create additional blocks for multi-block cells
                // (rare in copy/paste, but supported by the schema)
                for extra_frag in &frag_cell.blocks[1..] {
                    let extra_block = Block {
                        id: 0,
                        created_at: now,
                        updated_at: now,
                        elements: vec![],
                        list: None,
                        text_length: extra_frag.plain_text.chars().count() as i64,
                        document_position: current_pos,
                        plain_text: extra_frag.plain_text.clone(),
                        ..Default::default()
                    };
                    let created_extra = uow.create_block(&extra_block, cell_frame_id, -1)?;
                    for frag_elem in &extra_frag.elements {
                        let elem = frag_elem.to_entity();
                        uow.create_inline_element(&elem, created_extra.id, -1)?;
                    }
                    current_pos += extra_frag.plain_text.chars().count() as i64 + 1;
                    total_blocks_added += 1;
                }
            } else {
                // Empty cell — just position the empty block
                let mut updated_block = created_block.clone();
                updated_block.document_position = current_pos;
                updated_block.updated_at = now;
                cell_blocks_to_update.push(updated_block);
                current_pos += 1;
                total_blocks_added += 1;
            }

            // Create the TableCell entity
            let cell = TableCell {
                id: 0,
                created_at: now,
                updated_at: now,
                row: frag_cell.row as i64,
                column: frag_cell.column as i64,
                row_span: frag_cell.row_span as i64,
                column_span: frag_cell.column_span as i64,
                cell_frame: Some(cell_frame_id),
                fmt_padding: frag_cell.fmt_padding,
                fmt_border: frag_cell.fmt_border,
                fmt_vertical_alignment: frag_cell.fmt_vertical_alignment.clone(),
                fmt_background_color: frag_cell.fmt_background_color.clone(),
            };
            uow.create_table_cell(&cell, created_table.id, -1)?;
        }

        // Update cell block positions
        if !cell_blocks_to_update.is_empty() {
            uow.update_block_multi(&cell_blocks_to_update)?;
        }

        // Create the anchor frame for the table
        let anchor_frame = Frame {
            id: 0,
            created_at: now,
            updated_at: now,
            parent_frame: Some(frame_id),
            blocks: vec![],
            child_order: vec![],
            fmt_height: None,
            fmt_width: None,
            fmt_top_margin: None,
            fmt_bottom_margin: None,
            fmt_left_margin: None,
            fmt_right_margin: None,
            fmt_padding: None,
            fmt_border: None,
            fmt_position: None,
            fmt_is_blockquote: None,
            table: Some(created_table.id),
        };
        let created_anchor = uow.create_frame(&anchor_frame, doc_id, -1)?;

        // Insert anchor into parent frame's child_order
        let parent_frame = uow
            .get_frame(&frame_id)?
            .ok_or_else(|| anyhow!("Parent frame not found"))?;
        let mut updated_parent = parent_frame;
        let idx = current_child_idx.min(updated_parent.child_order.len());
        updated_parent
            .child_order
            .insert(idx, -(created_anchor.id as i64));
        updated_parent.updated_at = now;
        uow.update_frame(&updated_parent)?;

        current_child_idx += 1;
    }

    // Shift positions for existing blocks after the insertion point
    let pos_shift = current_pos - insert_pos;
    if pos_shift > 0 {
        let mut shifted: Vec<Block> = Vec::new();
        for block in &blocks {
            if block.document_position >= insert_pos {
                let mut ub = block.clone();
                ub.document_position += pos_shift;
                ub.updated_at = now;
                shifted.push(ub);
            }
        }
        if !shifted.is_empty() {
            uow.update_block_multi(&shifted)?;
        }
    }

    // Update document stats
    let mut updated_doc = document.clone();
    updated_doc.block_count += total_blocks_added;
    updated_doc.updated_at = now;
    uow.update_document(&updated_doc)?;

    Ok((
        InsertFragmentResultDto {
            new_position: insert_pos,
            blocks_added: total_blocks_added,
        },
        snapshot,
    ))
}

/// Insert a mixed fragment (both blocks and tables) at the cursor position.
/// Blocks and tables are interleaved according to each table's `block_insert_index`.
fn insert_mixed_fragment(
    uow: &mut Box<dyn InsertFragmentUnitOfWorkTrait>,
    dto: &InsertFragmentDto,
    fragment_data: &FragmentData,
) -> Result<(InsertFragmentResultDto, EntityTreeSnapshot)> {
    let now = chrono::Utc::now();

    let root = uow
        .get_root(&ROOT_ENTITY_ID)?
        .ok_or_else(|| anyhow!("Root entity not found"))?;
    let doc_ids = uow.get_root_relationship(&root.id, &RootRelationshipField::Document)?;
    let doc_id = *doc_ids
        .first()
        .ok_or_else(|| anyhow!("Root has no document"))?;
    let document = uow
        .get_document(&doc_id)?
        .ok_or_else(|| anyhow!("Document not found"))?;
    let snapshot = uow.snapshot_document(&[doc_id])?;

    if dto.position != dto.anchor {
        return Err(anyhow!(
            "Selection replacement is not supported. Use delete_text first."
        ));
    }

    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
    let frame_id = *frame_ids
        .first()
        .ok_or_else(|| anyhow!("Document has no frames"))?;
    let frame = uow
        .get_frame(&frame_id)?
        .ok_or_else(|| anyhow!("Frame not found"))?;

    let block_ids = uow.get_frame_relationship(&frame_id, &FrameRelationshipField::Blocks)?;
    let blocks_opt = uow.get_block_multi(&block_ids)?;
    let mut blocks: Vec<Block> = blocks_opt.into_iter().flatten().collect();
    blocks.sort_by_key(|b| b.document_position);

    let (current_block, block_idx, offset) = find_block_at_position(&blocks, dto.position)?;

    // ── Split current block elements ─────────────────────────────
    let element_ids =
        uow.get_block_relationship(&current_block.id, &BlockRelationshipField::Elements)?;
    let elements_opt = uow.get_inline_element_multi(&element_ids)?;
    let elements: Vec<InlineElement> = elements_opt.into_iter().flatten().collect();

    let plain_chars: Vec<char> = current_block.plain_text.chars().collect();
    let split_pos = (offset as usize).min(plain_chars.len());
    let text_before: String = plain_chars[..split_pos].iter().collect();
    let text_after: String = plain_chars[split_pos..].iter().collect();

    let mut after_elements: Vec<InlineElement> = Vec::new();
    let mut char_cursor: usize = 0;
    let mut split_found = false;

    for elem in &elements {
        let elem_char_len = match &elem.content {
            InlineContent::Text(s) => s.chars().count(),
            InlineContent::Image { .. } => 1,
            InlineContent::Empty => 0,
        };

        if !split_found {
            if char_cursor + elem_char_len <= split_pos {
                char_cursor += elem_char_len;
                continue;
            }
            split_found = true;
            let local_split = split_pos - char_cursor;

            match &elem.content {
                InlineContent::Text(s) => {
                    let chars: Vec<char> = s.chars().collect();
                    let before_str: String = chars[..local_split].iter().collect();
                    let after_str: String = chars[local_split..].iter().collect();
                    let mut updated = elem.clone();
                    updated.content = InlineContent::Text(before_str);
                    updated.updated_at = now;
                    uow.update_inline_element(&updated)?;
                    if !after_str.is_empty() {
                        let mut new_elem = elem.clone();
                        new_elem.id = 0;
                        new_elem.content = InlineContent::Text(after_str);
                        new_elem.created_at = now;
                        new_elem.updated_at = now;
                        after_elements.push(new_elem);
                    }
                }
                InlineContent::Image { .. } => {
                    if local_split == 0 {
                        let mut new_elem = elem.clone();
                        new_elem.id = 0;
                        new_elem.created_at = now;
                        new_elem.updated_at = now;
                        after_elements.push(new_elem);
                        let mut cleared = elem.clone();
                        cleared.content = InlineContent::Empty;
                        cleared.updated_at = now;
                        uow.update_inline_element(&cleared)?;
                    }
                }
                InlineContent::Empty => {}
            }
            char_cursor += elem_char_len;
        } else {
            let mut new_elem = elem.clone();
            new_elem.id = 0;
            new_elem.created_at = now;
            new_elem.updated_at = now;
            after_elements.push(new_elem);
            let mut cleared = elem.clone();
            cleared.content = InlineContent::Text(String::new());
            cleared.updated_at = now;
            uow.update_inline_element(&cleared)?;
            char_cursor += elem_char_len;
        }
    }

    if after_elements.is_empty() {
        after_elements.push(InlineElement {
            id: 0,
            created_at: now,
            updated_at: now,
            content: InlineContent::Text(text_after.clone()),
            ..Default::default()
        });
    }

    // ── Build interleaved item order ─────────────────────────────
    enum FragItem<'a> {
        Block(&'a FragmentBlock),
        Table(&'a FragmentTable),
    }

    let mut sorted_tables: Vec<&FragmentTable> = fragment_data.tables.iter().collect();
    sorted_tables.sort_by_key(|t| t.block_insert_index);

    let mut items: Vec<FragItem> = Vec::new();
    let mut blk_cursor = 0;
    for frag_table in &sorted_tables {
        let idx = frag_table
            .block_insert_index
            .min(fragment_data.blocks.len());
        while blk_cursor < idx {
            items.push(FragItem::Block(&fragment_data.blocks[blk_cursor]));
            blk_cursor += 1;
        }
        items.push(FragItem::Table(frag_table));
    }
    while blk_cursor < fragment_data.blocks.len() {
        items.push(FragItem::Block(&fragment_data.blocks[blk_cursor]));
        blk_cursor += 1;
    }

    // ── Merge first/last block optimisations ─────────────────────
    let merge_first = matches!(items.first(), Some(FragItem::Block(b)) if b.is_inline_only());
    let merge_last = fragment_data.blocks.len() >= 2
        && matches!(items.last(), Some(FragItem::Block(b)) if b.is_inline_only());

    let first_len: i64 = if merge_first {
        fragment_data.blocks[0].plain_text.chars().count() as i64
    } else {
        0
    };

    // Update current block (text_before + optional first-block merge)
    let mut updated_current = current_block.clone();
    if merge_first {
        let fb = &fragment_data.blocks[0];
        updated_current.plain_text = text_before.clone() + &fb.plain_text;
        updated_current.text_length = text_before.chars().count() as i64 + first_len;
    } else {
        updated_current.plain_text = text_before.clone();
        updated_current.text_length = text_before.chars().count() as i64;
    }
    updated_current.updated_at = now;
    uow.update_block(&updated_current)?;

    if merge_first {
        let fb = &fragment_data.blocks[0];
        for frag_elem in &fb.elements {
            let elem = frag_elem.to_entity();
            uow.create_inline_element(&elem, current_block.id, -1)?;
        }
    }

    let mut running_position = current_block.document_position + updated_current.text_length + 1;
    let mut new_child_order_entries: Vec<i64> = Vec::new();
    let mut total_new_chars: i64 = if merge_first { first_len } else { 0 };
    let mut total_blocks_added: i64 = 0;

    let skip_first = merge_first;
    let skip_last = merge_last;
    let mut block_index = 0usize; // index into fragment_data.blocks

    fn create_frag_elements_mixed(
        uow: &mut Box<dyn InsertFragmentUnitOfWorkTrait>,
        elements: &[common::parser_tools::fragment_schema::FragmentElement],
        block_id: EntityId,
    ) -> Result<()> {
        for frag_elem in elements {
            let elem = frag_elem.to_entity();
            uow.create_inline_element(&elem, block_id, -1)?;
        }
        Ok(())
    }

    let mut list_grouper = ListGrouper::new();
    // Pre-seed with the adjacent block's list for continuation (Word behavior)
    if let Some(list_id) = current_block.list {
        if let Ok(Some(list_entity)) = uow.get_list(&list_id) {
            list_grouper.register(list_id, list_entity.style.clone(), list_entity.indent as u32);
        }
    }

    // ── Process items in order ───────────────────────────────────
    for item in &items {
        match item {
            FragItem::Block(frag_block) => {
                let is_first = block_index == 0;
                let is_last = block_index == fragment_data.blocks.len() - 1;
                block_index += 1;

                if is_first && skip_first {
                    continue;
                }
                if is_last && skip_last {
                    continue;
                }

                let block_text_len = frag_block.plain_text.chars().count() as i64;

                let list_id = if let Some(ref frag_list) = frag_block.list {
                    if let Some(existing_id) =
                        list_grouper.try_reuse(&frag_list.style, frag_list.indent as u32)
                    {
                        Some(existing_id)
                    } else {
                        let list = frag_list.to_entity();
                        let created_list = uow.create_list(&list, doc_id, -1)?;
                        list_grouper.register(
                            created_list.id,
                            frag_list.style.clone(),
                            frag_list.indent as u32,
                        );
                        Some(created_list.id)
                    }
                } else {
                    list_grouper.reset();
                    None
                };

                let new_block = Block {
                    id: 0,
                    created_at: now,
                    updated_at: now,
                    elements: vec![],
                    list: list_id,
                    text_length: block_text_len,
                    document_position: running_position,
                    plain_text: frag_block.plain_text.clone(),
                    fmt_alignment: frag_block.alignment.clone(),
                    fmt_top_margin: frag_block.top_margin,
                    fmt_bottom_margin: frag_block.bottom_margin,
                    fmt_left_margin: frag_block.left_margin,
                    fmt_right_margin: frag_block.right_margin,
                    fmt_heading_level: frag_block.heading_level,
                    fmt_indent: frag_block.indent,
                    fmt_text_indent: frag_block.text_indent,
                    fmt_marker: frag_block.marker.clone(),
                    fmt_tab_positions: frag_block.tab_positions.clone(),
                    fmt_line_height: frag_block.line_height,
                    fmt_non_breakable_lines: frag_block.non_breakable_lines,
                    fmt_direction: frag_block.direction.clone(),
                    fmt_background_color: frag_block.background_color.clone(),
                    fmt_is_code_block: frag_block.is_code_block,
                    fmt_code_language: frag_block.code_language.clone(),
                };

                let created_block = uow.create_block(&new_block, frame_id, -1)?;
                create_frag_elements_mixed(uow, &frag_block.elements, created_block.id)?;

                if frag_block.elements.is_empty() {
                    let elem = InlineElement {
                        id: 0,
                        created_at: now,
                        updated_at: now,
                        content: InlineContent::Text(String::new()),
                        ..Default::default()
                    };
                    uow.create_inline_element(&elem, created_block.id, -1)?;
                }

                new_child_order_entries.push(created_block.id as i64);
                total_new_chars += block_text_len;
                total_blocks_added += 1;
                running_position += block_text_len + 1;
            }
            FragItem::Table(frag_table) => {
                if frag_table.rows == 0 || frag_table.columns == 0 || frag_table.cells.is_empty() {
                    continue;
                }

                let table = Table {
                    id: 0,
                    created_at: now,
                    updated_at: now,
                    cells: vec![],
                    rows: frag_table.rows as i64,
                    columns: frag_table.columns as i64,
                    column_widths: if frag_table.column_widths.is_empty() {
                        vec![0; frag_table.columns]
                    } else {
                        frag_table.column_widths.clone()
                    },
                    fmt_border: frag_table.fmt_border,
                    fmt_cell_spacing: frag_table.fmt_cell_spacing,
                    fmt_cell_padding: frag_table.fmt_cell_padding,
                    fmt_width: frag_table.fmt_width,
                    fmt_alignment: frag_table.fmt_alignment.clone(),
                };
                let created_table = uow.create_table(&table, doc_id, -1)?;

                let mut cell_blocks_to_update: Vec<Block> = Vec::new();

                for frag_cell in &frag_table.cells {
                    let (cell_frame_id, created_block) = create_cell_frame(uow, doc_id, now)?;

                    if !frag_cell.blocks.is_empty() {
                        let first_cb = &frag_cell.blocks[0];
                        let mut updated_block = created_block.clone();
                        updated_block.plain_text = first_cb.plain_text.clone();
                        updated_block.text_length = first_cb.plain_text.chars().count() as i64;
                        updated_block.document_position = running_position;
                        updated_block.updated_at = now;
                        cell_blocks_to_update.push(updated_block);

                        for frag_elem in &first_cb.elements {
                            let elem = frag_elem.to_entity();
                            uow.create_inline_element(&elem, created_block.id, -1)?;
                        }

                        running_position += first_cb.plain_text.chars().count() as i64 + 1;
                        total_blocks_added += 1;

                        for extra_frag in &frag_cell.blocks[1..] {
                            let extra_block = Block {
                                id: 0,
                                created_at: now,
                                updated_at: now,
                                elements: vec![],
                                list: None,
                                text_length: extra_frag.plain_text.chars().count() as i64,
                                document_position: running_position,
                                plain_text: extra_frag.plain_text.clone(),
                                ..Default::default()
                            };
                            let created_extra =
                                uow.create_block(&extra_block, cell_frame_id, -1)?;
                            for frag_elem in &extra_frag.elements {
                                let elem = frag_elem.to_entity();
                                uow.create_inline_element(&elem, created_extra.id, -1)?;
                            }
                            running_position += extra_frag.plain_text.chars().count() as i64 + 1;
                            total_blocks_added += 1;
                        }
                    } else {
                        let mut updated_block = created_block.clone();
                        updated_block.document_position = running_position;
                        updated_block.updated_at = now;
                        cell_blocks_to_update.push(updated_block);
                        running_position += 1;
                        total_blocks_added += 1;
                    }

                    let cell = TableCell {
                        id: 0,
                        created_at: now,
                        updated_at: now,
                        row: frag_cell.row as i64,
                        column: frag_cell.column as i64,
                        row_span: frag_cell.row_span as i64,
                        column_span: frag_cell.column_span as i64,
                        cell_frame: Some(cell_frame_id),
                        fmt_padding: frag_cell.fmt_padding,
                        fmt_border: frag_cell.fmt_border,
                        fmt_vertical_alignment: frag_cell.fmt_vertical_alignment.clone(),
                        fmt_background_color: frag_cell.fmt_background_color.clone(),
                    };
                    uow.create_table_cell(&cell, created_table.id, -1)?;
                }

                if !cell_blocks_to_update.is_empty() {
                    uow.update_block_multi(&cell_blocks_to_update)?;
                }

                let anchor_frame = Frame {
                    id: 0,
                    created_at: now,
                    updated_at: now,
                    parent_frame: Some(frame_id),
                    blocks: vec![],
                    child_order: vec![],
                    fmt_height: None,
                    fmt_width: None,
                    fmt_top_margin: None,
                    fmt_bottom_margin: None,
                    fmt_left_margin: None,
                    fmt_right_margin: None,
                    fmt_padding: None,
                    fmt_border: None,
                    fmt_position: None,
                    fmt_is_blockquote: None,
                    table: Some(created_table.id),
                };
                let created_anchor = uow.create_frame(&anchor_frame, doc_id, -1)?;
                new_child_order_entries.push(-(created_anchor.id as i64));
            }
        }
    }

    // ── Create tail block ────────────────────────────────────────
    let last_frag = if skip_last {
        fragment_data.blocks.last()
    } else {
        None
    };
    let last_len = last_frag
        .map(|b| b.plain_text.chars().count() as i64)
        .unwrap_or(0);

    let tail_plain = if let Some(lfb) = last_frag {
        total_new_chars += last_len;
        lfb.plain_text.clone() + &text_after
    } else {
        text_after.clone()
    };

    let tail_block = Block {
        id: 0,
        created_at: now,
        updated_at: now,
        elements: vec![],
        list: current_block.list,
        text_length: tail_plain.chars().count() as i64,
        document_position: running_position,
        plain_text: tail_plain,
        fmt_alignment: current_block.fmt_alignment.clone(),
        fmt_top_margin: current_block.fmt_top_margin,
        fmt_bottom_margin: current_block.fmt_bottom_margin,
        fmt_left_margin: current_block.fmt_left_margin,
        fmt_right_margin: current_block.fmt_right_margin,
        fmt_heading_level: current_block.fmt_heading_level,
        fmt_indent: current_block.fmt_indent,
        fmt_text_indent: current_block.fmt_text_indent,
        fmt_marker: current_block.fmt_marker.clone(),
        fmt_tab_positions: current_block.fmt_tab_positions.clone(),
        fmt_line_height: current_block.fmt_line_height,
        fmt_non_breakable_lines: current_block.fmt_non_breakable_lines,
        fmt_direction: current_block.fmt_direction.clone(),
        fmt_background_color: current_block.fmt_background_color.clone(),
        fmt_is_code_block: current_block.fmt_is_code_block,
        fmt_code_language: current_block.fmt_code_language.clone(),
    };

    let created_tail = uow.create_block(&tail_block, frame_id, -1)?;

    if let Some(lfb) = last_frag {
        create_frag_elements_mixed(uow, &lfb.elements, created_tail.id)?;
    }
    for after_elem in &after_elements {
        uow.create_inline_element(after_elem, created_tail.id, -1)?;
    }

    new_child_order_entries.push(created_tail.id as i64);
    total_blocks_added += 1;

    // ── Update frame child_order ─────────────────────────────────
    let mut updated_frame = frame.clone();
    let child_order_insert_pos = (block_idx + 1).min(updated_frame.child_order.len());
    for (i, entry) in new_child_order_entries.iter().enumerate() {
        updated_frame
            .child_order
            .insert(child_order_insert_pos + i, *entry);
    }
    updated_frame.updated_at = now;
    updated_frame.blocks =
        uow.get_frame_relationship(&frame_id, &FrameRelationshipField::Blocks)?;
    uow.update_frame(&updated_frame)?;

    // ── Shift existing blocks after insertion point ───────────────
    let original_next_pos = current_block.document_position + current_block.text_length + 1;
    let new_next_pos = running_position + created_tail.text_length + 1;
    let pos_shift = new_next_pos - original_next_pos;

    let mut blocks_to_update: Vec<Block> = Vec::new();
    for b in &blocks[(block_idx + 1)..] {
        let mut ub = b.clone();
        ub.document_position += pos_shift;
        ub.updated_at = now;
        blocks_to_update.push(ub);
    }
    if !blocks_to_update.is_empty() {
        uow.update_block_multi(&blocks_to_update)?;
    }

    // ── Update document stats ────────────────────────────────────
    let mut updated_doc = document.clone();
    updated_doc.block_count += total_blocks_added;
    updated_doc.character_count += total_new_chars;
    updated_doc.updated_at = now;
    uow.update_document(&updated_doc)?;

    let new_position = if skip_last {
        created_tail.document_position + last_len
    } else {
        created_tail.document_position
    };

    Ok((
        InsertFragmentResultDto {
            new_position,
            blocks_added: total_blocks_added,
        },
        snapshot,
    ))
}

fn execute_insert_fragment(
    uow: &mut Box<dyn InsertFragmentUnitOfWorkTrait>,
    dto: &InsertFragmentDto,
) -> Result<(InsertFragmentResultDto, EntityTreeSnapshot)> {
    const MAX_FRAGMENT_SIZE: usize = 64 * 1024 * 1024; // 64 MB
    if dto.fragment_data.len() > MAX_FRAGMENT_SIZE {
        return Err(anyhow!(
            "Fragment data exceeds maximum size ({} bytes, limit {})",
            dto.fragment_data.len(),
            MAX_FRAGMENT_SIZE
        ));
    }

    let fragment_data: FragmentData = serde_json::from_str(&dto.fragment_data)
        .map_err(|e| anyhow!("Invalid fragment_data JSON: {}", e))?;

    if fragment_data.blocks.is_empty() && fragment_data.tables.is_empty() {
        return Err(anyhow!("Fragment contains no blocks or tables"));
    }

    // ── Table-only fragment path ──────────────────────────────────
    if !fragment_data.tables.is_empty() && fragment_data.blocks.is_empty() {
        return insert_table_fragment(uow, dto, &fragment_data);
    }

    // ── Mixed blocks + tables fragment path ──────────────────────
    if !fragment_data.tables.is_empty() && !fragment_data.blocks.is_empty() {
        return insert_mixed_fragment(uow, dto, &fragment_data);
    }

    let root = uow
        .get_root(&ROOT_ENTITY_ID)?
        .ok_or_else(|| anyhow!("Root entity not found"))?;
    let doc_ids = uow.get_root_relationship(&root.id, &RootRelationshipField::Document)?;
    let doc_id = *doc_ids
        .first()
        .ok_or_else(|| anyhow!("Root has no document"))?;

    let document = uow
        .get_document(&doc_id)?
        .ok_or_else(|| anyhow!("Document not found"))?;

    let snapshot = uow.snapshot_document(&[doc_id])?;

    if dto.position != dto.anchor {
        return Err(anyhow!(
            "Selection replacement is not supported. Use delete_text first."
        ));
    }

    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
    let frame_id = *frame_ids
        .first()
        .ok_or_else(|| anyhow!("Document has no frames"))?;

    let frame = uow
        .get_frame(&frame_id)?
        .ok_or_else(|| anyhow!("Frame not found"))?;

    let block_ids = uow.get_frame_relationship(&frame_id, &FrameRelationshipField::Blocks)?;
    let blocks_opt = uow.get_block_multi(&block_ids)?;
    let mut blocks: Vec<Block> = blocks_opt.into_iter().flatten().collect();
    blocks.sort_by_key(|b| b.document_position);

    let (current_block, block_idx, offset) = find_block_at_position(&blocks, dto.position)?;

    // Get current block's elements for splitting
    let element_ids =
        uow.get_block_relationship(&current_block.id, &BlockRelationshipField::Elements)?;
    let elements_opt = uow.get_inline_element_multi(&element_ids)?;
    let elements: Vec<InlineElement> = elements_opt.into_iter().flatten().collect();

    let plain_chars: Vec<char> = current_block.plain_text.chars().collect();
    let split_pos = (offset as usize).min(plain_chars.len());

    // ── Inline merge: single block with no block-level formatting ──
    if fragment_data.blocks.len() == 1 && fragment_data.blocks[0].is_inline_only() {
        let frag_block = &fragment_data.blocks[0];
        let inserted_plain = &frag_block.plain_text;
        let inserted_len = inserted_plain.chars().count() as i64;

        if inserted_len == 0 {
            return Ok((
                InsertFragmentResultDto {
                    new_position: dto.position,
                    blocks_added: 0,
                },
                snapshot,
            ));
        }

        let now = chrono::Utc::now();

        // Find element at the cursor offset
        let (target_elem, elem_idx, local_offset) = find_element_at_offset(&elements, offset)?;

        // Split the target element: keep "before" text in place
        let after_text = match &target_elem.content {
            InlineContent::Text(s) => {
                let chars: Vec<char> = s.chars().collect();
                let lo = local_offset as usize;
                let before: String = chars[..lo].iter().collect();
                let after: String = chars[lo..].iter().collect();

                let mut updated = target_elem.clone();
                updated.content = InlineContent::Text(before);
                updated.updated_at = now;
                uow.update_inline_element(&updated)?;

                after
            }
            _ => String::new(),
        };

        // Create new inline elements from fragment
        let mut insert_idx = (elem_idx + 1) as i32;
        for frag_elem in &frag_block.elements {
            let elem = frag_elem.to_entity();
            uow.create_inline_element(&elem, current_block.id, insert_idx)?;
            insert_idx += 1;
        }

        // Create element for remaining text (preserving original formatting)
        if !after_text.is_empty() {
            let mut after_elem = target_elem.clone();
            after_elem.id = 0;
            after_elem.content = InlineContent::Text(after_text);
            after_elem.created_at = now;
            after_elem.updated_at = now;
            uow.create_inline_element(&after_elem, current_block.id, insert_idx)?;
        }

        // Update block metadata
        let new_plain: String = plain_chars[..split_pos].iter().collect::<String>()
            + inserted_plain
            + &plain_chars[split_pos..].iter().collect::<String>();
        let mut updated_block = current_block.clone();
        updated_block.plain_text = new_plain;
        updated_block.text_length += inserted_len;
        updated_block.updated_at = now;
        uow.update_block(&updated_block)?;

        // Shift subsequent blocks
        let mut blocks_to_update: Vec<Block> = Vec::new();
        for b in &blocks[(block_idx + 1)..] {
            let mut ub = b.clone();
            ub.document_position += inserted_len;
            ub.updated_at = now;
            blocks_to_update.push(ub);
        }
        if !blocks_to_update.is_empty() {
            uow.update_block_multi(&blocks_to_update)?;
        }

        // Update document character count
        let mut updated_doc = document.clone();
        updated_doc.character_count += inserted_len;
        updated_doc.updated_at = now;
        uow.update_document(&updated_doc)?;

        return Ok((
            InsertFragmentResultDto {
                new_position: dto.position + inserted_len,
                blocks_added: 0,
            },
            snapshot,
        ));
    }

    // ── Block-splitting path (multi-block or block-level content) ──
    let text_before: String = plain_chars[..split_pos].iter().collect();
    let text_after: String = plain_chars[split_pos..].iter().collect();

    let now = chrono::Utc::now();

    // Split elements: find which go before and after the split point
    let mut after_elements: Vec<InlineElement> = Vec::new();
    let mut char_cursor: usize = 0;
    let mut split_found = false;

    for elem in &elements {
        let elem_char_len = match &elem.content {
            InlineContent::Text(s) => s.chars().count(),
            InlineContent::Image { .. } => 1,
            InlineContent::Empty => 0,
        };

        if !split_found {
            if char_cursor + elem_char_len <= split_pos {
                char_cursor += elem_char_len;
                continue;
            }
            split_found = true;
            let local_split = split_pos - char_cursor;

            match &elem.content {
                InlineContent::Text(s) => {
                    let chars: Vec<char> = s.chars().collect();
                    let before_text: String = chars[..local_split].iter().collect();
                    let after_text: String = chars[local_split..].iter().collect();

                    let mut updated = elem.clone();
                    updated.content = InlineContent::Text(before_text);
                    updated.updated_at = now;
                    uow.update_inline_element(&updated)?;

                    if !after_text.is_empty() {
                        let mut new_elem = elem.clone();
                        new_elem.id = 0;
                        new_elem.content = InlineContent::Text(after_text);
                        new_elem.created_at = now;
                        new_elem.updated_at = now;
                        after_elements.push(new_elem);
                    }
                }
                InlineContent::Image { .. } => {
                    if local_split == 0 {
                        let mut new_elem = elem.clone();
                        new_elem.id = 0;
                        new_elem.created_at = now;
                        new_elem.updated_at = now;
                        after_elements.push(new_elem);

                        let mut cleared = elem.clone();
                        cleared.content = InlineContent::Empty;
                        cleared.updated_at = now;
                        uow.update_inline_element(&cleared)?;
                    }
                }
                InlineContent::Empty => {}
            }
            char_cursor += elem_char_len;
        } else {
            let mut new_elem = elem.clone();
            new_elem.id = 0;
            new_elem.created_at = now;
            new_elem.updated_at = now;
            after_elements.push(new_elem);

            let mut cleared = elem.clone();
            cleared.content = InlineContent::Text(String::new());
            cleared.updated_at = now;
            uow.update_inline_element(&cleared)?;

            char_cursor += elem_char_len;
        }
    }

    if after_elements.is_empty() {
        after_elements.push(InlineElement {
            id: 0,
            created_at: now,
            updated_at: now,
            content: InlineContent::Text(text_after.clone()),
            ..Default::default()
        });
    }

    // Helper: create fragment elements on a block
    fn create_frag_elements(
        uow: &mut Box<dyn InsertFragmentUnitOfWorkTrait>,
        elements: &[common::parser_tools::fragment_schema::FragmentElement],
        block_id: EntityId,
    ) -> Result<()> {
        for frag_elem in elements {
            let elem = frag_elem.to_entity();
            uow.create_inline_element(&elem, block_id, -1)?;
        }
        Ok(())
    }

    if fragment_data.blocks.len() >= 2 {
        // ── Multi-block: merge inline-only first/last, standalone otherwise ──
        let first_frag = &fragment_data.blocks[0];
        let last_frag = &fragment_data.blocks[fragment_data.blocks.len() - 1];
        let merge_first = first_frag.is_inline_only();
        let merge_last = last_frag.is_inline_only();

        let first_len = first_frag.plain_text.chars().count() as i64;

        let mut updated_current = current_block.clone();
        if merge_first {
            updated_current.plain_text = text_before.clone() + &first_frag.plain_text;
            updated_current.text_length = text_before.chars().count() as i64 + first_len;
        } else {
            updated_current.plain_text = text_before.clone();
            updated_current.text_length = text_before.chars().count() as i64;
        }
        updated_current.updated_at = now;
        uow.update_block(&updated_current)?;

        if merge_first {
            create_frag_elements(uow, &first_frag.elements, current_block.id)?;
        }

        let mut new_block_ids: Vec<EntityId> = Vec::new();
        let mut total_new_chars: i64 = if merge_first { first_len } else { 0 };
        let mut running_position =
            current_block.document_position + updated_current.text_length + 1;

        let middle_start = if merge_first { 1 } else { 0 };
        let middle_end = if merge_last {
            fragment_data.blocks.len() - 1
        } else {
            fragment_data.blocks.len()
        };

        let mut list_grouper = ListGrouper::new();
        // Pre-seed with the adjacent block's list for continuation (Word behavior)
        if let Some(list_id) = current_block.list {
            if let Ok(Some(list_entity)) = uow.get_list(&list_id) {
                list_grouper.register(list_id, list_entity.style.clone(), list_entity.indent as u32);
            }
        }
        for frag_block in &fragment_data.blocks[middle_start..middle_end] {
            let block_text_len = frag_block.plain_text.chars().count() as i64;

            let list_id = if let Some(ref frag_list) = frag_block.list {
                if let Some(existing_id) =
                    list_grouper.try_reuse(&frag_list.style, frag_list.indent as u32)
                {
                    Some(existing_id)
                } else {
                    let list = frag_list.to_entity();
                    let created_list = uow.create_list(&list, doc_id, -1)?;
                    list_grouper.register(
                        created_list.id,
                        frag_list.style.clone(),
                        frag_list.indent as u32,
                    );
                    Some(created_list.id)
                }
            } else {
                list_grouper.reset();
                None
            };

            let new_block = Block {
                id: 0,
                created_at: now,
                updated_at: now,
                elements: vec![],
                list: list_id,
                text_length: block_text_len,
                document_position: running_position,
                plain_text: frag_block.plain_text.clone(),
                fmt_alignment: frag_block.alignment.clone(),
                fmt_top_margin: frag_block.top_margin,
                fmt_bottom_margin: frag_block.bottom_margin,
                fmt_left_margin: frag_block.left_margin,
                fmt_right_margin: frag_block.right_margin,
                fmt_heading_level: frag_block.heading_level,
                fmt_indent: frag_block.indent,
                fmt_text_indent: frag_block.text_indent,
                fmt_marker: frag_block.marker.clone(),
                fmt_tab_positions: frag_block.tab_positions.clone(),
                fmt_line_height: frag_block.line_height,
                fmt_non_breakable_lines: frag_block.non_breakable_lines,
                fmt_direction: frag_block.direction.clone(),
                fmt_background_color: frag_block.background_color.clone(),
                fmt_is_code_block: frag_block.is_code_block,
                fmt_code_language: frag_block.code_language.clone(),
            };

            let insert_index = (block_idx + 1 + new_block_ids.len()) as i32;
            let created_block = uow.create_block(&new_block, frame_id, insert_index)?;

            create_frag_elements(uow, &frag_block.elements, created_block.id)?;

            if frag_block.elements.is_empty() {
                let elem = InlineElement {
                    id: 0,
                    created_at: now,
                    updated_at: now,
                    content: InlineContent::Text(String::new()),
                    ..Default::default()
                };
                uow.create_inline_element(&elem, created_block.id, -1)?;
            }

            new_block_ids.push(created_block.id);
            total_new_chars += block_text_len;
            running_position += block_text_len + 1;
        }

        let last_len = last_frag.plain_text.chars().count() as i64;

        let tail_plain = if merge_last {
            total_new_chars += last_len;
            last_frag.plain_text.clone() + &text_after
        } else {
            text_after.clone()
        };
        let tail_block = Block {
            id: 0,
            created_at: now,
            updated_at: now,
            elements: vec![],
            list: current_block.list,
            text_length: tail_plain.chars().count() as i64,
            document_position: running_position,
            plain_text: tail_plain,
            fmt_alignment: current_block.fmt_alignment.clone(),
            fmt_top_margin: current_block.fmt_top_margin,
            fmt_bottom_margin: current_block.fmt_bottom_margin,
            fmt_left_margin: current_block.fmt_left_margin,
            fmt_right_margin: current_block.fmt_right_margin,
            fmt_heading_level: current_block.fmt_heading_level,
            fmt_indent: current_block.fmt_indent,
            fmt_text_indent: current_block.fmt_text_indent,
            fmt_marker: current_block.fmt_marker.clone(),
            fmt_tab_positions: current_block.fmt_tab_positions.clone(),
            fmt_line_height: current_block.fmt_line_height,
            fmt_non_breakable_lines: current_block.fmt_non_breakable_lines,
            fmt_direction: current_block.fmt_direction.clone(),
            fmt_background_color: current_block.fmt_background_color.clone(),
            fmt_is_code_block: current_block.fmt_is_code_block,
            fmt_code_language: current_block.fmt_code_language.clone(),
        };

        let tail_insert_index = (block_idx + 1 + new_block_ids.len()) as i32;
        let created_tail = uow.create_block(&tail_block, frame_id, tail_insert_index)?;

        if merge_last {
            create_frag_elements(uow, &last_frag.elements, created_tail.id)?;
        }
        for after_elem in &after_elements {
            uow.create_inline_element(after_elem, created_tail.id, -1)?;
        }

        // Update frame child_order
        let mut updated_frame = frame.clone();
        let child_order_insert_pos = (block_idx + 1).min(updated_frame.child_order.len());
        let mut new_child_ids: Vec<i64> = new_block_ids.iter().map(|id| *id as i64).collect();
        new_child_ids.push(created_tail.id as i64);

        for (i, id) in new_child_ids.iter().enumerate() {
            updated_frame
                .child_order
                .insert(child_order_insert_pos + i, *id);
        }
        updated_frame.updated_at = now;
        updated_frame.blocks =
            uow.get_frame_relationship(&frame_id, &FrameRelationshipField::Blocks)?;
        uow.update_frame(&updated_frame)?;

        let standalone_count = (middle_end - middle_start) as i64;
        let blocks_added = standalone_count + 1;
        let original_next_pos = current_block.document_position + current_block.text_length + 1;
        let new_next_pos = running_position + created_tail.text_length + 1;
        let pos_shift = new_next_pos - original_next_pos;

        let mut blocks_to_update: Vec<Block> = Vec::new();
        for b in &blocks[(block_idx + 1)..] {
            let mut ub = b.clone();
            ub.document_position += pos_shift;
            ub.updated_at = now;
            blocks_to_update.push(ub);
        }
        if !blocks_to_update.is_empty() {
            uow.update_block_multi(&blocks_to_update)?;
        }

        let mut updated_doc = document.clone();
        updated_doc.block_count += blocks_added;
        updated_doc.character_count += total_new_chars;
        updated_doc.updated_at = now;
        uow.update_document(&updated_doc)?;

        let new_position = if merge_last {
            created_tail.document_position + last_len
        } else {
            created_tail.document_position
        };

        Ok((
            InsertFragmentResultDto {
                new_position,
                blocks_added,
            },
            snapshot,
        ))
    } else {
        // ── Single block with block-level formatting ──
        let frag_block = &fragment_data.blocks[0];
        let block_text_len = frag_block.plain_text.chars().count() as i64;

        let mut updated_current = current_block.clone();
        updated_current.plain_text = text_before.clone();
        updated_current.text_length = text_before.chars().count() as i64;
        updated_current.updated_at = now;
        uow.update_block(&updated_current)?;

        let mut running_position =
            current_block.document_position + updated_current.text_length + 1;

        let list_id = if let Some(ref frag_list) = frag_block.list {
            let list = frag_list.to_entity();
            let created_list = uow.create_list(&list, doc_id, -1)?;
            Some(created_list.id)
        } else {
            None
        };

        let new_block = Block {
            id: 0,
            created_at: now,
            updated_at: now,
            elements: vec![],
            list: list_id,
            text_length: block_text_len,
            document_position: running_position,
            plain_text: frag_block.plain_text.clone(),
            fmt_alignment: frag_block.alignment.clone(),
            fmt_top_margin: frag_block.top_margin,
            fmt_bottom_margin: frag_block.bottom_margin,
            fmt_left_margin: frag_block.left_margin,
            fmt_right_margin: frag_block.right_margin,
            fmt_heading_level: frag_block.heading_level,
            fmt_indent: frag_block.indent,
            fmt_text_indent: frag_block.text_indent,
            fmt_marker: frag_block.marker.clone(),
            fmt_tab_positions: frag_block.tab_positions.clone(),
            fmt_line_height: frag_block.line_height,
            fmt_non_breakable_lines: frag_block.non_breakable_lines,
            fmt_direction: frag_block.direction.clone(),
            fmt_background_color: frag_block.background_color.clone(),
            fmt_is_code_block: frag_block.is_code_block,
            fmt_code_language: frag_block.code_language.clone(),
        };

        let created_block = uow.create_block(&new_block, frame_id, (block_idx + 1) as i32)?;
        create_frag_elements(uow, &frag_block.elements, created_block.id)?;

        if frag_block.elements.is_empty() {
            let elem = InlineElement {
                id: 0,
                created_at: now,
                updated_at: now,
                content: InlineContent::Text(String::new()),
                ..Default::default()
            };
            uow.create_inline_element(&elem, created_block.id, -1)?;
        }

        running_position += block_text_len + 1;

        let tail_block = Block {
            id: 0,
            created_at: now,
            updated_at: now,
            elements: vec![],
            list: current_block.list,
            text_length: text_after.chars().count() as i64,
            document_position: running_position,
            plain_text: text_after,
            fmt_alignment: current_block.fmt_alignment.clone(),
            fmt_top_margin: current_block.fmt_top_margin,
            fmt_bottom_margin: current_block.fmt_bottom_margin,
            fmt_left_margin: current_block.fmt_left_margin,
            fmt_right_margin: current_block.fmt_right_margin,
            fmt_heading_level: current_block.fmt_heading_level,
            fmt_indent: current_block.fmt_indent,
            fmt_text_indent: current_block.fmt_text_indent,
            fmt_marker: current_block.fmt_marker.clone(),
            fmt_tab_positions: current_block.fmt_tab_positions.clone(),
            fmt_line_height: current_block.fmt_line_height,
            fmt_non_breakable_lines: current_block.fmt_non_breakable_lines,
            fmt_direction: current_block.fmt_direction.clone(),
            fmt_background_color: current_block.fmt_background_color.clone(),
            fmt_is_code_block: current_block.fmt_is_code_block,
            fmt_code_language: current_block.fmt_code_language.clone(),
        };

        let created_tail = uow.create_block(&tail_block, frame_id, (block_idx + 2) as i32)?;
        for after_elem in &after_elements {
            uow.create_inline_element(after_elem, created_tail.id, -1)?;
        }

        let mut updated_frame = frame.clone();
        let child_order_insert_pos = (block_idx + 1).min(updated_frame.child_order.len());
        let new_child_ids = [created_block.id as i64, created_tail.id as i64];
        for (i, id) in new_child_ids.iter().enumerate() {
            updated_frame
                .child_order
                .insert(child_order_insert_pos + i, *id);
        }
        updated_frame.updated_at = now;
        updated_frame.blocks =
            uow.get_frame_relationship(&frame_id, &FrameRelationshipField::Blocks)?;
        uow.update_frame(&updated_frame)?;

        let blocks_added: i64 = 2;
        let original_next_pos = current_block.document_position + current_block.text_length + 1;
        let new_next_pos = running_position + created_tail.text_length + 1;
        let pos_shift = new_next_pos - original_next_pos;

        let mut blocks_to_update: Vec<Block> = Vec::new();
        for b in &blocks[(block_idx + 1)..] {
            let mut ub = b.clone();
            ub.document_position += pos_shift;
            ub.updated_at = now;
            blocks_to_update.push(ub);
        }
        if !blocks_to_update.is_empty() {
            uow.update_block_multi(&blocks_to_update)?;
        }

        let mut updated_doc = document.clone();
        updated_doc.block_count += blocks_added;
        updated_doc.character_count += block_text_len;
        updated_doc.updated_at = now;
        uow.update_document(&updated_doc)?;

        Ok((
            InsertFragmentResultDto {
                new_position: running_position,
                blocks_added: 1,
            },
            snapshot,
        ))
    }
}

pub struct InsertFragmentUseCase {
    uow_factory: Box<dyn InsertFragmentUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<InsertFragmentDto>,
}

impl InsertFragmentUseCase {
    pub fn new(uow_factory: Box<dyn InsertFragmentUnitOfWorkFactoryTrait>) -> Self {
        InsertFragmentUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &InsertFragmentDto) -> Result<InsertFragmentResultDto> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let (result, snapshot) = execute_insert_fragment(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());

        uow.commit()?;
        Ok(result)
    }
}

impl UndoRedoCommand for InsertFragmentUseCase {
    fn undo(&mut self) -> Result<()> {
        let snapshot = self
            .undo_snapshot
            .as_ref()
            .ok_or_else(|| anyhow!("No snapshot available for undo"))?
            .clone();

        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;
        uow.restore_document(&snapshot)?;
        uow.commit()?;
        Ok(())
    }

    fn redo(&mut self) -> Result<()> {
        let dto = self
            .last_dto
            .as_ref()
            .ok_or_else(|| anyhow!("No DTO available for redo"))?
            .clone();

        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;
        let (_, snapshot) = execute_insert_fragment(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
