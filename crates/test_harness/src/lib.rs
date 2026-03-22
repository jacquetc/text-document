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
pub use direct_access::block::dtos::CreateBlockDto;
pub use direct_access::document::document_controller;
pub use direct_access::document::dtos::CreateDocumentDto;
pub use direct_access::frame::dtos::CreateFrameDto;
pub use direct_access::frame::frame_controller;
pub use direct_access::inline_element::dtos::CreateInlineElementDto;
pub use direct_access::inline_element::inline_element_controller;
pub use direct_access::root::dtos::CreateRootDto;
pub use direct_access::root::root_controller;
pub use direct_access::table::dtos::TableDto;
pub use direct_access::table::table_controller;
pub use direct_access::table_cell::dtos::TableCellDto;
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
