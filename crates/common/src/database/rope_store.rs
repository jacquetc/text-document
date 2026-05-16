//! Ropey-backed storage backend (Phase 2 skeleton).
//!
//! Mirrors the shape of [`HashMapStore`](super::hashmap_store::HashMapStore)
//! but replaces per-block `plain_text: String` with a single
//! document-wide `ropey::Rope`, and collapses the 12 junction tables
//! into inline `Vec<EntityId>` fields on parent entities (see the
//! migration plan §1.5).
//!
//! **Status (Phase 2 step 1)**: skeleton only. Every method body is
//! `unimplemented!()`. Nothing in the workspace uses this type yet —
//! `DbContext` continues to wrap `HashMapStore`. Steps 2–4 implement
//! the snapshot/savepoint methods, wire `BlockOffsetIndex`, and start
//! swapping use cases over. Step 7 deletes `HashMapStore`.

#![allow(dead_code)]

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
    /// `im::HashMap`s are HAMT-shared).
    pub fn snapshot(&self) -> RopeStoreSnapshot {
        unimplemented!("RopeStore::snapshot — Phase 2 step 2")
    }

    /// Restore from a snapshot (overwrites counters — savepoint semantic).
    pub fn restore(&self, _snap: &RopeStoreSnapshot) {
        unimplemented!("RopeStore::restore — Phase 2 step 2")
    }

    /// Restore everything *except* counters (undo semantic — IDs must
    /// remain monotonically increasing across undo/redo cycles).
    pub fn restore_without_counters(&self, _snap: &RopeStoreSnapshot) {
        unimplemented!("RopeStore::restore_without_counters — Phase 2 step 2")
    }

    pub fn create_savepoint(&self) -> u64 {
        unimplemented!("RopeStore::create_savepoint — Phase 2 step 2")
    }

    pub fn restore_savepoint(&self, _savepoint_id: u64) {
        unimplemented!("RopeStore::restore_savepoint — Phase 2 step 2")
    }

    pub fn discard_savepoint(&self, _savepoint_id: u64) {
        unimplemented!("RopeStore::discard_savepoint — Phase 2 step 2")
    }

    pub(crate) fn next_id(&self, _entity_name: &str) -> EntityId {
        unimplemented!("RopeStore::next_id — Phase 2 step 2")
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
