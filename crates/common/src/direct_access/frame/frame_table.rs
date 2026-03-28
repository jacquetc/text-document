use crate::{impl_relationship_methods, impl_write_relationship_methods};
use crate::database::hashmap_store::{
    HashMapStore, delete_from_backward_junction, junction_get, junction_remove, junction_restore,
    junction_set, junction_snapshot, junction_snapshot_backward,
};
use crate::entities::*;
use crate::error::RepositoryError;
use crate::snapshot::{TableLevelSnapshot, TableSnapshot};
use crate::types::EntityId;
use std::collections::HashMap;
use std::sync::RwLock;

use super::frame_repository::{FrameRelationshipField, FrameTable, FrameTableRO};

pub struct FrameHashMapTable<'a> {
    store: &'a HashMapStore,
}

impl<'a> FrameHashMapTable<'a> {
    pub fn new(store: &'a HashMapStore) -> Self {
        Self { store }
    }

    fn resolve_junction(
        &self,
        field: &FrameRelationshipField,
    ) -> &RwLock<HashMap<EntityId, Vec<EntityId>>> {
        match field {
            FrameRelationshipField::Blocks => &self.store.jn_block_from_frame_blocks,
            FrameRelationshipField::ParentFrame => &self.store.jn_frame_from_frame_parent_frame,
            FrameRelationshipField::Table => &self.store.jn_table_from_frame_table,
        }
    }

    fn hydrate(&self, entity: &mut Frame) {
        entity.blocks = junction_get(&self.store.jn_block_from_frame_blocks, &entity.id);
        entity.parent_frame = junction_get(
            &self.store.jn_frame_from_frame_parent_frame,
            &entity.id,
        )
        .into_iter()
        .next();
        entity.table = junction_get(&self.store.jn_table_from_frame_table, &entity.id)
            .into_iter()
            .next();
    }
}

impl<'a> FrameTable for FrameHashMapTable<'a> {
    fn create(&mut self, entity: &Frame) -> Result<Frame, RepositoryError> {
        self.create_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn create_multi(&mut self, entities: &[Frame]) -> Result<Vec<Frame>, RepositoryError> {
        let mut created = Vec::with_capacity(entities.len());
        let mut frames = self.store.frames.write().unwrap();

        for entity in entities {
            let new_entity = if entity.id == EntityId::default() {
                let id = self.store.next_id("frame");
                Frame {
                    id,
                    ..entity.clone()
                }
            } else {
                if frames.contains_key(&entity.id) {
                    return Err(RepositoryError::DuplicateId {
                        entity: "Frame",
                        id: entity.id,
                    });
                }
                entity.clone()
            };

            frames.insert(new_entity.id, new_entity.clone());
            junction_set(
                &self.store.jn_block_from_frame_blocks,
                new_entity.id,
                new_entity.blocks.clone(),
            );
            junction_set(
                &self.store.jn_frame_from_frame_parent_frame,
                new_entity.id,
                new_entity.parent_frame.into_iter().collect(),
            );
            junction_set(
                &self.store.jn_table_from_frame_table,
                new_entity.id,
                new_entity.table.into_iter().collect(),
            );
            created.push(new_entity);
        }
        Ok(created)
    }

    fn get(&self, id: &EntityId) -> Result<Option<Frame>, RepositoryError> {
        let frames = self.store.frames.read().unwrap();
        match frames.get(id) {
            Some(entity) => {
                let mut e = entity.clone();
                drop(frames);
                self.hydrate(&mut e);
                Ok(Some(e))
            }
            None => Ok(None),
        }
    }

    fn get_multi(&self, ids: &[EntityId]) -> Result<Vec<Option<Frame>>, RepositoryError> {
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            result.push(self.get(id)?);
        }
        Ok(result)
    }

    fn get_all(&self) -> Result<Vec<Frame>, RepositoryError> {
        let frames = self.store.frames.read().unwrap();
        let entries: Vec<Frame> = frames.values().cloned().collect();
        drop(frames);
        let mut result = Vec::with_capacity(entries.len());
        for mut entity in entries {
            self.hydrate(&mut entity);
            result.push(entity);
        }
        Ok(result)
    }

    fn update(&mut self, entity: &Frame) -> Result<Frame, RepositoryError> {
        self.update_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn update_multi(&mut self, entities: &[Frame]) -> Result<Vec<Frame>, RepositoryError> {
        let mut frames = self.store.frames.write().unwrap();
        for entity in entities {
            frames.insert(entity.id, entity.clone());
        }
        drop(frames);
        let ids: Vec<EntityId> = entities.iter().map(|e| e.id).collect();
        let result = self.get_multi(&ids)?;
        Ok(result.into_iter().flatten().collect())
    }

    fn update_with_relationships(&mut self, entity: &Frame) -> Result<Frame, RepositoryError> {
        self.update_with_relationships_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn update_with_relationships_multi(
        &mut self,
        entities: &[Frame],
    ) -> Result<Vec<Frame>, RepositoryError> {
        let mut frames = self.store.frames.write().unwrap();
        for entity in entities {
            frames.insert(entity.id, entity.clone());
            junction_set(
                &self.store.jn_block_from_frame_blocks,
                entity.id,
                entity.blocks.clone(),
            );
            junction_set(
                &self.store.jn_frame_from_frame_parent_frame,
                entity.id,
                entity.parent_frame.into_iter().collect(),
            );
            junction_set(
                &self.store.jn_table_from_frame_table,
                entity.id,
                entity.table.into_iter().collect(),
            );
        }
        drop(frames);
        let ids: Vec<EntityId> = entities.iter().map(|e| e.id).collect();
        let result = self.get_multi(&ids)?;
        Ok(result.into_iter().flatten().collect())
    }

    fn remove(&mut self, id: &EntityId) -> Result<(), RepositoryError> {
        self.remove_multi(std::slice::from_ref(id))
    }

    fn remove_multi(&mut self, ids: &[EntityId]) -> Result<(), RepositoryError> {
        let mut frames = self.store.frames.write().unwrap();
        for id in ids {
            frames.remove(id);
            junction_remove(&self.store.jn_block_from_frame_blocks, id);
            junction_remove(&self.store.jn_frame_from_frame_parent_frame, id);
            junction_remove(&self.store.jn_table_from_frame_table, id);
            // backward junctions
            delete_from_backward_junction(
                &self.store.jn_frame_from_table_cell_cell_frame,
                id,
            );
            delete_from_backward_junction(&self.store.jn_frame_from_document_frames, id);
            // self-referential backward
            delete_from_backward_junction(&self.store.jn_frame_from_frame_parent_frame, id);
        }
        Ok(())
    }

    impl_write_relationship_methods!(FrameHashMapTable<'a>, FrameRelationshipField);

    fn snapshot_rows(&self, ids: &[EntityId]) -> Result<TableLevelSnapshot, RepositoryError> {
        let frames = self.store.frames.read().unwrap();
        let mut rows = Vec::new();
        for id in ids {
            if let Some(entity) = frames.get(id) {
                let bytes = postcard::to_allocvec(entity)
                    .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
                rows.push((*id, bytes));
            }
        }

        let forward_junctions = vec![
            junction_snapshot(
                &self.store.jn_block_from_frame_blocks,
                ids,
                "block_from_frame_blocks_junction",
            ),
            junction_snapshot(
                &self.store.jn_frame_from_frame_parent_frame,
                ids,
                "frame_from_frame_parent_frame_junction",
            ),
            junction_snapshot(
                &self.store.jn_table_from_frame_table,
                ids,
                "table_from_frame_table_junction",
            ),
        ];

        let mut backward_junctions = Vec::new();
        if let Some(snap) = junction_snapshot_backward(
            &self.store.jn_frame_from_table_cell_cell_frame,
            ids,
            "frame_from_table_cell_cell_frame_junction",
        ) {
            backward_junctions.push(snap);
        }
        if let Some(snap) = junction_snapshot_backward(
            &self.store.jn_back_document_frames,
            ids,
            "frame_from_document_frames_junction",
        ) {
            backward_junctions.push(snap);
        }
        if let Some(snap) = junction_snapshot_backward(
            &self.store.jn_back_frame_parent_frame,
            ids,
            "frame_from_frame_parent_frame_junction",
        ) {
            backward_junctions.push(snap);
        }

        Ok(TableLevelSnapshot {
            entity_rows: TableSnapshot {
                table_name: "frame".to_string(),
                rows,
            },
            forward_junctions,
            backward_junctions,
        })
    }

    fn restore_rows(&mut self, snap: &TableLevelSnapshot) -> Result<(), RepositoryError> {
        let mut frames = self.store.frames.write().unwrap();
        for (id, bytes) in &snap.entity_rows.rows {
            let entity: Frame = postcard::from_bytes(bytes)
                .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
            frames.insert(*id, entity);
        }
        drop(frames);
        for js in &snap.forward_junctions {
            match js.table_name.as_str() {
                "block_from_frame_blocks_junction" => {
                    junction_restore(&self.store.jn_block_from_frame_blocks, js);
                }
                "frame_from_frame_parent_frame_junction" => {
                    junction_restore(&self.store.jn_frame_from_frame_parent_frame, js);
                }
                "table_from_frame_table_junction" => {
                    junction_restore(&self.store.jn_table_from_frame_table, js);
                }
                _ => {}
            }
        }
        for js in &snap.backward_junctions {
            match js.table_name.as_str() {
                "frame_from_table_cell_cell_frame_junction" => {
                    junction_restore(&self.store.jn_frame_from_table_cell_cell_frame, js);
                }
                "frame_from_document_frames_junction" => {
                    junction_restore(&self.store.jn_back_document_frames, js);
                }
                "frame_from_frame_parent_frame_junction" => {
                    junction_restore(&self.store.jn_back_frame_parent_frame, js);
                }
                _ => {}
            }
        }
        Ok(())
    }
}

pub struct FrameHashMapTableRO<'a> {
    store: &'a HashMapStore,
}

impl<'a> FrameHashMapTableRO<'a> {
    pub fn new(store: &'a HashMapStore) -> Self {
        Self { store }
    }

    fn resolve_junction(
        &self,
        field: &FrameRelationshipField,
    ) -> &RwLock<HashMap<EntityId, Vec<EntityId>>> {
        match field {
            FrameRelationshipField::Blocks => &self.store.jn_block_from_frame_blocks,
            FrameRelationshipField::ParentFrame => &self.store.jn_frame_from_frame_parent_frame,
            FrameRelationshipField::Table => &self.store.jn_table_from_frame_table,
        }
    }

    fn hydrate(&self, entity: &mut Frame) {
        entity.blocks = junction_get(&self.store.jn_block_from_frame_blocks, &entity.id);
        entity.parent_frame = junction_get(
            &self.store.jn_frame_from_frame_parent_frame,
            &entity.id,
        )
        .into_iter()
        .next();
        entity.table = junction_get(&self.store.jn_table_from_frame_table, &entity.id)
            .into_iter()
            .next();
    }
}

impl<'a> FrameTableRO for FrameHashMapTableRO<'a> {
    fn get(&self, id: &EntityId) -> Result<Option<Frame>, RepositoryError> {
        let frames = self.store.frames.read().unwrap();
        match frames.get(id) {
            Some(entity) => {
                let mut e = entity.clone();
                drop(frames);
                self.hydrate(&mut e);
                Ok(Some(e))
            }
            None => Ok(None),
        }
    }

    fn get_multi(&self, ids: &[EntityId]) -> Result<Vec<Option<Frame>>, RepositoryError> {
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            result.push(self.get(id)?);
        }
        Ok(result)
    }

    fn get_all(&self) -> Result<Vec<Frame>, RepositoryError> {
        let frames = self.store.frames.read().unwrap();
        let entries: Vec<Frame> = frames.values().cloned().collect();
        drop(frames);
        let mut result = Vec::with_capacity(entries.len());
        for mut entity in entries {
            self.hydrate(&mut entity);
            result.push(entity);
        }
        Ok(result)
    }

    impl_relationship_methods!(FrameHashMapTableRO<'a>, FrameRelationshipField);
}
