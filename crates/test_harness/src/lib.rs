//! Shared test setup utilities for text-document crate tests.
//!
//! Provides helpers to create an in-memory document with content,
//! export text, and traverse the entity tree. This crate depends only
//! on `common` and `direct_access` — it reimplements plain-text import
//! and export directly via entity controllers, so it does **not** depend
//! on `document_io` or any feature crate, breaking the circular
//! dev-dependency chain.

use anyhow::Result;
use common::database::db_context::DbContext;
use common::entities::InlineContent;
use common::event::EventHub;
use common::types::EntityId;
use common::undo_redo::UndoRedoManager;
use std::sync::Arc;

// Re-export commonly used types and controllers for convenience
pub use common::direct_access::block::block_repository::BlockRelationshipField;
pub use common::direct_access::document::document_repository::DocumentRelationshipField;
pub use common::direct_access::frame::frame_repository::FrameRelationshipField;
pub use common::direct_access::root::root_repository::RootRelationshipField;

pub use common::direct_access::table::table_repository::TableRelationshipField;
pub use common::direct_access::table_cell::table_cell_repository::TableCellRelationshipField;
pub use direct_access::block::block_controller;
pub use direct_access::block::dtos::{BlockRelationshipDto, CreateBlockDto, UpdateBlockDto};
pub use direct_access::document::document_controller;
pub use direct_access::document::dtos::CreateDocumentDto;
pub use direct_access::frame::dtos::CreateFrameDto;
pub use direct_access::frame::frame_controller;
pub use direct_access::inline_element::dtos::{CreateInlineElementDto, UpdateInlineElementDto};
pub use direct_access::inline_element::inline_element_controller;
pub use direct_access::list::dtos::CreateListDto as CreateListEntityDto;
pub use direct_access::list::list_controller;
pub use direct_access::root::dtos::CreateRootDto;
pub use direct_access::root::root_controller;
pub use direct_access::table::dtos::{CreateTableDto, TableDto};
pub use direct_access::table::table_controller;
pub use direct_access::table_cell::dtos::{CreateTableCellDto, TableCellDto};
pub use direct_access::table_cell::table_cell_controller;

/// Create an in-memory database with a Root and empty Document.
///
/// Returns `(DbContext, Arc<EventHub>, UndoRedoManager)`.
pub fn setup() -> Result<(DbContext, Arc<EventHub>, UndoRedoManager)> {
    let db_context = DbContext::new()?;
    let event_hub = Arc::new(EventHub::new());
    let mut undo_redo_manager = UndoRedoManager::new();

    let root = root_controller::create_orphan(&db_context, &event_hub, &CreateRootDto::default())?;

    let _doc = document_controller::create(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &CreateDocumentDto::default(),
        root.id,
        -1,
    )?;

    Ok((db_context, event_hub, undo_redo_manager))
}

/// Create an in-memory database with a Root, Document, and imported text content.
///
/// Splits the text on `\n` and creates one Block + InlineElement per line,
/// mirroring what `document_io::import_plain_text` does but without depending
/// on the `document_io` crate.
///
/// Returns `(DbContext, Arc<EventHub>, UndoRedoManager)`.
pub fn setup_with_text(text: &str) -> Result<(DbContext, Arc<EventHub>, UndoRedoManager)> {
    let (db_context, event_hub, mut undo_redo_manager) = setup()?;

    // Get Root -> Document -> existing Frame
    let root_rels =
        root_controller::get_relationship(&db_context, &1, &RootRelationshipField::Document)?;
    let doc_id = root_rels[0];
    let frame_ids = document_controller::get_relationship(
        &db_context,
        &doc_id,
        &DocumentRelationshipField::Frames,
    )?;

    // Remove existing frames (the setup creates one empty frame)
    for fid in &frame_ids {
        frame_controller::remove(&db_context, &event_hub, &mut undo_redo_manager, None, fid)?;
    }

    // Create a fresh frame
    let frame = frame_controller::create(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &CreateFrameDto::default(),
        doc_id,
        -1,
    )?;

    // Split text into lines and create blocks
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut document_position: i64 = 0;
    let mut total_chars: i64 = 0;
    let mut child_order: Vec<i64> = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        let line_len = line.chars().count() as i64;

        let block_dto = CreateBlockDto {
            plain_text: line.to_string(),
            text_length: line_len,
            document_position,
            ..Default::default()
        };

        let block = block_controller::create(
            &db_context,
            &event_hub,
            &mut undo_redo_manager,
            None,
            &block_dto,
            frame.id,
            i as i32,
        )?;

        child_order.push(block.id as i64);

        let elem_dto = CreateInlineElementDto {
            content: InlineContent::Text(line.to_string()),
            ..Default::default()
        };

        inline_element_controller::create(
            &db_context,
            &event_hub,
            &mut undo_redo_manager,
            None,
            &elem_dto,
            block.id,
            0,
        )?;

        total_chars += line_len;
        document_position += line_len;
        if i < lines.len() - 1 {
            document_position += 1; // block separator
        }
    }

    // Update frame child_order to include all blocks
    let mut updated_frame = frame_controller::get(&db_context, &frame.id)?
        .ok_or_else(|| anyhow::anyhow!("Frame not found"))?;
    updated_frame.child_order = child_order;
    frame_controller::update(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &updated_frame.into(),
    )?;

    // Update document cached fields
    let mut doc = document_controller::get(&db_context, &doc_id)?
        .ok_or_else(|| anyhow::anyhow!("Document not found"))?;
    doc.character_count = total_chars;
    doc.block_count = lines.len() as i64;
    document_controller::update(
        &db_context,
        &event_hub,
        &mut undo_redo_manager,
        None,
        &doc.into(),
    )?;

    // Clear undo history so test starts clean
    undo_redo_manager.clear_all_stacks();

    Ok((db_context, event_hub, undo_redo_manager))
}

/// Export the current document as plain text by reading blocks and
/// concatenating their `plain_text` fields with `\n` separators.
pub fn export_text(db_context: &DbContext, _event_hub: &Arc<EventHub>) -> Result<String> {
    let block_ids = get_block_ids(db_context)?;
    let mut blocks = Vec::new();
    for id in &block_ids {
        if let Some(b) = block_controller::get(db_context, id)? {
            blocks.push(b);
        }
    }
    blocks.sort_by_key(|b| b.document_position);
    let text = blocks
        .iter()
        .map(|b| b.plain_text.as_str())
        .collect::<Vec<&str>>()
        .join("\n");
    Ok(text)
}

/// Get the first frame's block IDs.
pub fn get_block_ids(db_context: &DbContext) -> Result<Vec<EntityId>> {
    let root_rels =
        root_controller::get_relationship(db_context, &1, &RootRelationshipField::Document)?;
    let doc_id = root_rels[0];
    let frame_ids = document_controller::get_relationship(
        db_context,
        &doc_id,
        &DocumentRelationshipField::Frames,
    )?;
    let frame_id = frame_ids[0];
    frame_controller::get_relationship(db_context, &frame_id, &FrameRelationshipField::Blocks)
}

/// Get the element IDs for a given block.
pub fn get_element_ids(db_context: &DbContext, block_id: &EntityId) -> Result<Vec<EntityId>> {
    block_controller::get_relationship(db_context, block_id, &BlockRelationshipField::Elements)
}

/// Get the first block's element IDs.
pub fn get_first_block_element_ids(db_context: &DbContext) -> Result<Vec<EntityId>> {
    let block_ids = get_block_ids(db_context)?;
    get_element_ids(db_context, &block_ids[0])
}

/// Get the first frame ID for the document.
pub fn get_frame_id(db_context: &DbContext) -> Result<EntityId> {
    let root_rels =
        root_controller::get_relationship(db_context, &1, &RootRelationshipField::Document)?;
    let doc_id = root_rels[0];
    let frame_ids = document_controller::get_relationship(
        db_context,
        &doc_id,
        &DocumentRelationshipField::Frames,
    )?;
    Ok(frame_ids[0])
}

/// Get all table IDs in the document.
pub fn get_table_ids(db_context: &DbContext) -> Result<Vec<EntityId>> {
    let root_rels =
        root_controller::get_relationship(db_context, &1, &RootRelationshipField::Document)?;
    let doc_id = root_rels[0];
    document_controller::get_relationship(db_context, &doc_id, &DocumentRelationshipField::Tables)
}

/// Get all cell IDs for a given table.
pub fn get_table_cell_ids(db_context: &DbContext, table_id: &EntityId) -> Result<Vec<EntityId>> {
    table_controller::get_relationship(db_context, table_id, &TableRelationshipField::Cells)
}

/// Get all cells for a table, sorted by row then column.
pub fn get_sorted_cells(db_context: &DbContext, table_id: &EntityId) -> Result<Vec<TableCellDto>> {
    let cell_ids = get_table_cell_ids(db_context, table_id)?;
    let cells_opt = table_cell_controller::get_multi(db_context, &cell_ids)?;
    let mut cells: Vec<TableCellDto> = cells_opt.into_iter().flatten().collect();
    cells.sort_by(|a, b| a.row.cmp(&b.row).then(a.column.cmp(&b.column)));
    Ok(cells)
}

/// Get all block IDs across all frames in the document (not just the first frame).
pub fn get_all_block_ids(db_context: &DbContext) -> Result<Vec<EntityId>> {
    let root_rels =
        root_controller::get_relationship(db_context, &1, &RootRelationshipField::Document)?;
    let doc_id = root_rels[0];
    let frame_ids = document_controller::get_relationship(
        db_context,
        &doc_id,
        &DocumentRelationshipField::Frames,
    )?;
    let mut all_block_ids = Vec::new();
    for fid in &frame_ids {
        let block_ids =
            frame_controller::get_relationship(db_context, fid, &FrameRelationshipField::Blocks)?;
        all_block_ids.extend(block_ids);
    }
    Ok(all_block_ids)
}

/// Basic document statistics retrieved directly from entity data.
pub struct BasicStats {
    pub character_count: i64,
    pub block_count: i64,
    pub frame_count: i64,
}

/// Get basic document statistics by reading the Document entity directly.
pub fn get_document_stats(db_context: &DbContext) -> Result<BasicStats> {
    let root_rels =
        root_controller::get_relationship(db_context, &1, &RootRelationshipField::Document)?;
    let doc_id = root_rels[0];
    let doc = document_controller::get(db_context, &doc_id)?
        .ok_or_else(|| anyhow::anyhow!("Document not found"))?;
    let frame_ids = document_controller::get_relationship(
        db_context,
        &doc_id,
        &DocumentRelationshipField::Frames,
    )?;
    Ok(BasicStats {
        character_count: doc.character_count,
        block_count: doc.block_count,
        frame_count: frame_ids.len() as i64,
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// Test-only helpers that build richer documents (tables, lists, images,
// frames) using entity controllers directly — no feature-crate dependency.
// ═══════════════════════════════════════════════════════════════════════════

pub struct InsertTableResult {
    pub table_id: EntityId,
}

/// Insert a `rows x columns` table at `position` using entity controllers.
///
/// Creates the Table, one Frame+Block+EmptyElement per cell, and adjusts
/// `document_position` for all subsequent blocks.
pub fn insert_table(
    db_context: &DbContext,
    event_hub: &Arc<EventHub>,
    undo_redo_manager: &mut UndoRedoManager,
    position: i64,
    rows: i64,
    columns: i64,
) -> Result<InsertTableResult> {
    let doc_id = get_doc_id(db_context)?;

    // Create table owned by document
    let table = table_controller::create(
        db_context,
        event_hub,
        undo_redo_manager,
        None,
        &CreateTableDto {
            rows,
            columns,
            ..Default::default()
        },
        doc_id,
        -1,
    )?;

    let table_size = rows * columns;
    let mut cell_blocks: Vec<EntityId> = Vec::new();

    for r in 0..rows {
        for c in 0..columns {
            // Create cell frame owned by document
            let cell_frame = frame_controller::create(
                db_context,
                event_hub,
                undo_redo_manager,
                None,
                &CreateFrameDto::default(),
                doc_id,
                -1,
            )?;

            // Create block in cell frame
            let block = block_controller::create(
                db_context,
                event_hub,
                undo_redo_manager,
                None,
                &CreateBlockDto::default(),
                cell_frame.id,
                0,
            )?;

            // Create empty element in block
            inline_element_controller::create(
                db_context,
                event_hub,
                undo_redo_manager,
                None,
                &CreateInlineElementDto {
                    content: InlineContent::Empty,
                    ..Default::default()
                },
                block.id,
                0,
            )?;

            // Create table cell owned by table
            table_cell_controller::create(
                db_context,
                event_hub,
                undo_redo_manager,
                None,
                &CreateTableCellDto {
                    row: r,
                    column: c,
                    row_span: 1,
                    column_span: 1,
                    cell_frame: Some(cell_frame.id),
                    ..Default::default()
                },
                table.id,
                -1,
            )?;

            cell_blocks.push(block.id);
        }
    }

    // Assign document_positions to cell blocks
    for (current_pos, &bid) in (position..).zip(cell_blocks.iter()) {
        let mut b = block_controller::get(db_context, &bid)?
            .ok_or_else(|| anyhow::anyhow!("Block not found"))?;
        b.document_position = current_pos;
        block_controller::update(db_context, event_hub, undo_redo_manager, None, &b.into())?;
    }

    // Shift existing blocks (not cell blocks) that are at or after position
    let all_bids = get_all_block_ids(db_context)?;
    for bid in &all_bids {
        if cell_blocks.contains(bid) {
            continue;
        }
        let b = block_controller::get(db_context, bid)?
            .ok_or_else(|| anyhow::anyhow!("Block not found"))?;
        if b.document_position >= position {
            let mut updated = b.clone();
            updated.document_position += table_size;
            block_controller::update(
                db_context,
                event_hub,
                undo_redo_manager,
                None,
                &updated.into(),
            )?;
        }
    }

    undo_redo_manager.clear_all_stacks();
    Ok(InsertTableResult { table_id: table.id })
}

pub struct CreateListResult {
    pub list_id: EntityId,
}

/// Create a list spanning blocks in `[position, anchor]` using entity controllers.
pub fn create_list(
    db_context: &DbContext,
    event_hub: &Arc<EventHub>,
    undo_redo_manager: &mut UndoRedoManager,
    position: i64,
    anchor: i64,
    style: common::entities::ListStyle,
) -> Result<CreateListResult> {
    let doc_id = get_doc_id(db_context)?;
    let sel_start = std::cmp::min(position, anchor);
    let sel_end = std::cmp::max(position, anchor);

    let list = list_controller::create(
        db_context,
        event_hub,
        undo_redo_manager,
        None,
        &CreateListEntityDto {
            style,
            ..Default::default()
        },
        doc_id,
        -1,
    )?;

    // Find overlapping blocks and assign them to the list
    let all_bids = get_all_block_ids(db_context)?;
    for bid in &all_bids {
        let b = block_controller::get(db_context, bid)?
            .ok_or_else(|| anyhow::anyhow!("Block not found"))?;
        let block_start = b.document_position;
        let block_end = block_start + b.text_length;
        if block_end >= sel_start && block_start <= sel_end {
            block_controller::set_relationship(
                db_context,
                event_hub,
                undo_redo_manager,
                None,
                &BlockRelationshipDto {
                    id: b.id,
                    field: BlockRelationshipField::List,
                    right_ids: vec![list.id],
                },
            )?;
        }
    }

    undo_redo_manager.clear_all_stacks();
    Ok(CreateListResult { list_id: list.id })
}

pub struct InsertImageResult {
    pub new_position: i64,
    pub element_id: EntityId,
}

/// Insert an image inline element at `position` using entity controllers.
///
/// Splits the text element at the insertion offset when needed.
pub fn insert_image(
    db_context: &DbContext,
    event_hub: &Arc<EventHub>,
    undo_redo_manager: &mut UndoRedoManager,
    position: i64,
    image_name: &str,
    width: i64,
    height: i64,
) -> Result<InsertImageResult> {
    // Find block containing position
    let all_bids = get_all_block_ids(db_context)?;
    let mut blocks = Vec::new();
    for bid in &all_bids {
        blocks.push(
            block_controller::get(db_context, bid)?
                .ok_or_else(|| anyhow::anyhow!("Block not found"))?,
        );
    }
    blocks.sort_by_key(|b| b.document_position);

    let (target_block, offset) = blocks
        .iter()
        .find_map(|b| {
            let s = b.document_position;
            let e = s + b.text_length;
            if position >= s && position <= e {
                Some((b.clone(), (position - s) as usize))
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow::anyhow!("No block at position {}", position))?;

    // Walk elements to find the one at offset
    let elem_ids = block_controller::get_relationship(
        db_context,
        &target_block.id,
        &BlockRelationshipField::Elements,
    )?;

    let mut running = 0usize;
    let mut insert_after_idx: i32 = -1;
    for (idx, eid) in elem_ids.iter().enumerate() {
        let elem = inline_element_controller::get(db_context, eid)?
            .ok_or_else(|| anyhow::anyhow!("Element not found"))?;
        let elen = match &elem.content {
            InlineContent::Text(s) => s.chars().count(),
            InlineContent::Image { .. } => 1,
            InlineContent::Empty => 0,
        };

        if running + elen > offset && offset > running {
            // Split text element
            if let InlineContent::Text(ref text) = elem.content {
                let chars: Vec<char> = text.chars().collect();
                let local = offset - running;
                let before: String = chars[..local].iter().collect();
                let after: String = chars[local..].iter().collect();

                // Shrink original to 'before'
                let mut upd: UpdateInlineElementDto = elem.clone().into();
                upd.content = InlineContent::Text(before);
                inline_element_controller::update(
                    db_context,
                    event_hub,
                    undo_redo_manager,
                    None,
                    &upd,
                )?;

                // Create 'after' element
                let after_entity: common::entities::InlineElement = elem.clone().into();
                let mut after_create = CreateInlineElementDto::from(after_entity);
                after_create.content = InlineContent::Text(after);
                inline_element_controller::create(
                    db_context,
                    event_hub,
                    undo_redo_manager,
                    None,
                    &after_create,
                    target_block.id,
                    (idx as i32) + 1,
                )?;
            }
            insert_after_idx = (idx as i32) + 1;
            break;
        }
        running += elen;
        if running >= offset {
            insert_after_idx = (idx as i32) + 1;
            break;
        }
    }
    if insert_after_idx < 0 {
        insert_after_idx = elem_ids.len() as i32;
    }

    // Create image element
    let img = inline_element_controller::create(
        db_context,
        event_hub,
        undo_redo_manager,
        None,
        &CreateInlineElementDto {
            content: InlineContent::Image {
                name: image_name.to_string(),
                width,
                height,
                quality: 100,
            },
            ..Default::default()
        },
        target_block.id,
        insert_after_idx,
    )?;

    // Update block text_length (+1) and shift subsequent blocks
    let mut upd_block = target_block.clone();
    upd_block.text_length += 1;
    block_controller::update(
        db_context,
        event_hub,
        undo_redo_manager,
        None,
        &upd_block.into(),
    )?;

    for b in &blocks {
        if b.id != target_block.id && b.document_position > target_block.document_position {
            let mut shifted = b.clone();
            shifted.document_position += 1;
            block_controller::update(
                db_context,
                event_hub,
                undo_redo_manager,
                None,
                &shifted.into(),
            )?;
        }
    }

    undo_redo_manager.clear_all_stacks();
    Ok(InsertImageResult {
        new_position: position + 1,
        element_id: img.id,
    })
}

pub struct InsertFrameResult {
    pub frame_id: EntityId,
}

/// Insert a sub-frame at `position` using entity controllers.
///
/// The new frame contains one empty block and is registered in
/// the document's frames collection.
pub fn insert_frame(
    db_context: &DbContext,
    event_hub: &Arc<EventHub>,
    undo_redo_manager: &mut UndoRedoManager,
    position: i64,
) -> Result<InsertFrameResult> {
    let doc_id = get_doc_id(db_context)?;

    let new_frame = frame_controller::create(
        db_context,
        event_hub,
        undo_redo_manager,
        None,
        &CreateFrameDto::default(),
        doc_id,
        -1,
    )?;

    let block = block_controller::create(
        db_context,
        event_hub,
        undo_redo_manager,
        None,
        &CreateBlockDto {
            document_position: position,
            ..Default::default()
        },
        new_frame.id,
        0,
    )?;

    inline_element_controller::create(
        db_context,
        event_hub,
        undo_redo_manager,
        None,
        &CreateInlineElementDto {
            content: InlineContent::Empty,
            ..Default::default()
        },
        block.id,
        0,
    )?;

    undo_redo_manager.clear_all_stacks();
    Ok(InsertFrameResult {
        frame_id: new_frame.id,
    })
}

fn get_doc_id(db_context: &DbContext) -> Result<EntityId> {
    let root_rels =
        root_controller::get_relationship(db_context, &1, &RootRelationshipField::Document)?;
    Ok(root_rels[0])
}
