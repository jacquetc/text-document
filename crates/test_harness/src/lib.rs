//! Shared test setup utilities for text-document crate tests.
//!
//! Provides helpers to create an in-memory document with content,
//! export text, and traverse the entity tree — eliminating the need
//! for each feature crate to depend on `direct_access` and `document_io`
//! in its dev-dependencies.

use anyhow::Result;
use common::database::db_context::DbContext;
use common::direct_access::block::block_repository::BlockRelationshipField;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::event::EventHub;
use common::types::EntityId;
use common::undo_redo::UndoRedoManager;
use std::sync::Arc;

use direct_access::block::block_controller;
use direct_access::document::document_controller;
use direct_access::document::dtos::CreateDocumentDto;
use direct_access::frame::frame_controller;
use direct_access::inline_element::inline_element_controller;
use direct_access::root::dtos::CreateRootDto;
use direct_access::root::root_controller;

use document_io::document_io_controller;
use document_io::ImportPlainTextDto;

/// Create an in-memory database with a Root and empty Document.
///
/// Returns `(DbContext, Arc<EventHub>, UndoRedoManager)`.
pub fn setup() -> Result<(DbContext, Arc<EventHub>, UndoRedoManager)> {
    let db_context = DbContext::new()?;
    let event_hub = Arc::new(EventHub::new());
    let mut undo_redo_manager = UndoRedoManager::new();

    let root =
        root_controller::create_orphan(&db_context, &event_hub, &CreateRootDto::default())?;

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
/// Returns `(DbContext, Arc<EventHub>, UndoRedoManager)`.
pub fn setup_with_text(text: &str) -> Result<(DbContext, Arc<EventHub>, UndoRedoManager)> {
    let (db_context, event_hub, undo_redo_manager) = setup()?;

    document_io_controller::import_plain_text(
        &db_context,
        &event_hub,
        &ImportPlainTextDto {
            plain_text: text.to_string(),
        },
    )?;

    Ok((db_context, event_hub, undo_redo_manager))
}

/// Export the current document as plain text.
pub fn export_text(db_context: &DbContext, event_hub: &Arc<EventHub>) -> Result<String> {
    let dto = document_io_controller::export_plain_text(db_context, event_hub)?;
    Ok(dto.plain_text)
}

/// Get the first frame's block IDs (sorted by document_position).
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
    let block_ids = frame_controller::get_relationship(
        db_context,
        &frame_id,
        &FrameRelationshipField::Blocks,
    )?;
    Ok(block_ids)
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

// Re-export commonly used types and controllers for convenience
pub use direct_access::block::block_controller;
pub use direct_access::document::document_controller;
pub use direct_access::frame::frame_controller;
pub use direct_access::inline_element::inline_element_controller;
pub use direct_access::root::root_controller;

pub use common::direct_access::block::block_repository::BlockRelationshipField;
pub use common::direct_access::document::document_repository::DocumentRelationshipField;
pub use common::direct_access::frame::frame_repository::FrameRelationshipField;
pub use common::direct_access::root::root_repository::RootRelationshipField;

pub use document_io::document_io_controller;
pub use document_io::ImportPlainTextDto;

/// Basic document statistics retrieved directly from entity data.
pub struct BasicStats {
    pub character_count: i64,
    pub block_count: i64,
    pub frame_count: i64,
}

/// Get basic document statistics by reading the Document entity directly
/// — avoids depending on the document_inspection crate.
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
