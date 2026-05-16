//! Ropey-backed storage backend.
//!
//! Holds character data in a single document-wide `ropey::Rope`,
//! with structural entities (Frames, Tables, Lists, Resources) in
//! `im::HashMap` tables and per-block character formatting in
//! `format_runs`. See the migration plan §1.5 for the relationship
//! inlining and §1.6 for the rope layout (block boundary `\n` +
//! U+FFFC table anchor).

use crate::database::block_offset_index::BlockOffsetIndex;
use crate::entities::*;
use crate::format_runs::{FormatRun, ImageAnchor};
use crate::snapshot::{StoreSnapshot, StoreSnapshotTrait};
use crate::types::EntityId;
use im::HashMap;
use ropey::Rope;
use std::collections::HashMap as StdHashMap;
use std::sync::RwLock;

// ─────────────────────────────────────────────────────────────────────────────
// The Store
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct RopeStore {
    // ── Character content (shared across all blocks, including cells) ──
    pub rope: RwLock<Rope>,

    // ── Structural entity tables ──────────────────────────────────────
    pub roots: RwLock<HashMap<EntityId, Root>>,
    pub documents: RwLock<HashMap<EntityId, Document>>,
    pub frames: RwLock<HashMap<EntityId, Frame>>,
    pub blocks: RwLock<HashMap<EntityId, Block>>,
    pub lists: RwLock<HashMap<EntityId, List>>,
    pub resources: RwLock<HashMap<EntityId, Resource>>,
    pub tables: RwLock<HashMap<EntityId, Table>>,
    pub table_cells: RwLock<HashMap<EntityId, TableCell>>,

    // ── Per-block character formatting + image anchors ────────────────
    pub format_runs: RwLock<HashMap<EntityId, Vec<FormatRun>>>,
    pub block_images: RwLock<HashMap<EntityId, Vec<ImageAnchor>>>,

    // ── Document-wide block ordering (sorted by rope position) ────────
    pub block_offsets: RwLock<BlockOffsetIndex>,

    // ── ID counters ───────────────────────────────────────────────────
    // Never restored by undo (only by transaction rollback).
    pub counters: RwLock<StdHashMap<String, EntityId>>,

    // ── Savepoints (in-memory, transaction-scoped) ────────────────────
    savepoints: RwLock<StdHashMap<u64, RopeStoreSnapshot>>,
    next_savepoint_id: RwLock<u64>,
}

impl RopeStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// O(1) snapshot of the entire store (rope is Arc-shared, all
    /// `im::HashMap`s are HAMT-shared; `BlockOffsetIndex` is a small
    /// `Vec` cloned outright).
    pub fn snapshot(&self) -> RopeStoreSnapshot {
        RopeStoreSnapshot {
            rope: self.rope.read().unwrap().clone(),
            roots: self.roots.read().unwrap().clone(),
            documents: self.documents.read().unwrap().clone(),
            frames: self.frames.read().unwrap().clone(),
            blocks: self.blocks.read().unwrap().clone(),
            lists: self.lists.read().unwrap().clone(),
            resources: self.resources.read().unwrap().clone(),
            tables: self.tables.read().unwrap().clone(),
            table_cells: self.table_cells.read().unwrap().clone(),
            format_runs: self.format_runs.read().unwrap().clone(),
            block_images: self.block_images.read().unwrap().clone(),
            block_offsets: self.block_offsets.read().unwrap().clone(),
            counters: self.counters.read().unwrap().clone(),
        }
    }

    /// Restore from a snapshot. Overwrites counters too — used for
    /// transaction rollback (`Drop` of an uncommitted write txn).
    pub fn restore(&self, snap: &RopeStoreSnapshot) {
        *self.rope.write().unwrap() = snap.rope.clone();
        *self.roots.write().unwrap() = snap.roots.clone();
        *self.documents.write().unwrap() = snap.documents.clone();
        *self.frames.write().unwrap() = snap.frames.clone();
        *self.blocks.write().unwrap() = snap.blocks.clone();
        *self.lists.write().unwrap() = snap.lists.clone();
        *self.resources.write().unwrap() = snap.resources.clone();
        *self.tables.write().unwrap() = snap.tables.clone();
        *self.table_cells.write().unwrap() = snap.table_cells.clone();
        *self.format_runs.write().unwrap() = snap.format_runs.clone();
        *self.block_images.write().unwrap() = snap.block_images.clone();
        *self.block_offsets.write().unwrap() = snap.block_offsets.clone();
        *self.counters.write().unwrap() = snap.counters.clone();
    }

    /// Restore everything *except* counters — used for undo, where IDs
    /// must remain monotonically increasing across undo/redo cycles.
    pub fn restore_without_counters(&self, snap: &RopeStoreSnapshot) {
        *self.rope.write().unwrap() = snap.rope.clone();
        *self.roots.write().unwrap() = snap.roots.clone();
        *self.documents.write().unwrap() = snap.documents.clone();
        *self.frames.write().unwrap() = snap.frames.clone();
        *self.blocks.write().unwrap() = snap.blocks.clone();
        *self.lists.write().unwrap() = snap.lists.clone();
        *self.resources.write().unwrap() = snap.resources.clone();
        *self.tables.write().unwrap() = snap.tables.clone();
        *self.table_cells.write().unwrap() = snap.table_cells.clone();
        *self.format_runs.write().unwrap() = snap.format_runs.clone();
        *self.block_images.write().unwrap() = snap.block_images.clone();
        *self.block_offsets.write().unwrap() = snap.block_offsets.clone();
        // counters intentionally not restored
    }

    pub fn create_savepoint(&self) -> u64 {
        let snap = self.snapshot();
        let mut id_counter = self.next_savepoint_id.write().unwrap();
        let id = *id_counter;
        *id_counter += 1;
        self.savepoints.write().unwrap().insert(id, snap);
        id
    }

    pub fn restore_savepoint(&self, savepoint_id: u64) {
        let snap = self
            .savepoints
            .read()
            .unwrap()
            .get(&savepoint_id)
            .expect("savepoint not found")
            .clone();
        self.restore(&snap);
    }

    pub fn discard_savepoint(&self, savepoint_id: u64) {
        self.savepoints.write().unwrap().remove(&savepoint_id);
    }

    /// Get-and-increment counter for an entity type.
    pub(crate) fn next_id(&self, entity_name: &str) -> EntityId {
        let mut counters = self.counters.write().unwrap();
        let counter = counters.entry(entity_name.to_string()).or_insert(1);
        let id = *counter;
        *counter += 1;
        id
    }

    /// Type-erased store snapshot (for the generic undo path).
    pub fn store_snapshot(&self) -> StoreSnapshot {
        StoreSnapshot::new(self.snapshot())
    }

    /// Restore from a type-erased store snapshot (undo semantic —
    /// counters preserved).
    pub fn restore_store_snapshot(&self, snap: &StoreSnapshot) {
        let s = snap
            .downcast_ref::<RopeStoreSnapshot>()
            .expect("StoreSnapshot must contain RopeStoreSnapshot");
        self.restore_without_counters(s);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Snapshot
// ─────────────────────────────────────────────────────────────────────────────

/// O(1)-clone snapshot. `Rope::clone()` shares the Arc-d B+ tree root;
/// every `im::HashMap::clone()` is HAMT-structural.
#[derive(Debug, Clone, Default)]
pub struct RopeStoreSnapshot {
    pub(crate) rope: Rope,
    pub(crate) roots: HashMap<EntityId, Root>,
    pub(crate) documents: HashMap<EntityId, Document>,
    pub(crate) frames: HashMap<EntityId, Frame>,
    pub(crate) blocks: HashMap<EntityId, Block>,
    pub(crate) lists: HashMap<EntityId, List>,
    pub(crate) resources: HashMap<EntityId, Resource>,
    pub(crate) tables: HashMap<EntityId, Table>,
    pub(crate) table_cells: HashMap<EntityId, TableCell>,
    pub(crate) format_runs: HashMap<EntityId, Vec<FormatRun>>,
    pub(crate) block_images: HashMap<EntityId, Vec<ImageAnchor>>,
    pub(crate) block_offsets: BlockOffsetIndex,
    pub(crate) counters: StdHashMap<String, EntityId>,
}

impl StoreSnapshotTrait for RopeStoreSnapshot {
    fn clone_box(&self) -> Box<dyn StoreSnapshotTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
