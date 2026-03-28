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

use super::block_repository::{BlockRelationshipField, BlockTable, BlockTableRO};

pub struct BlockHashMapTable<'a> {
    store: &'a HashMapStore,
}

impl<'a> BlockHashMapTable<'a> {
    pub fn new(store: &'a HashMapStore) -> Self {
        Self { store }
    }

    fn resolve_junction(
        &self,
        field: &BlockRelationshipField,
    ) -> &RwLock<HashMap<EntityId, Vec<EntityId>>> {
        match field {
            BlockRelationshipField::Elements => &self.store.jn_inline_element_from_block_elements,
            BlockRelationshipField::List => &self.store.jn_list_from_block_list,
        }
    }

    fn hydrate(&self, entity: &mut Block) {
        entity.elements = junction_get(
            &self.store.jn_inline_element_from_block_elements,
            &entity.id,
        );
        entity.list = junction_get(&self.store.jn_list_from_block_list, &entity.id)
            .into_iter()
            .next();
    }
}

impl<'a> BlockTable for BlockHashMapTable<'a> {
    fn create(&mut self, entity: &Block) -> Result<Block, RepositoryError> {
        self.create_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn create_multi(&mut self, entities: &[Block]) -> Result<Vec<Block>, RepositoryError> {
        let mut created = Vec::with_capacity(entities.len());
        let mut blocks = self.store.blocks.write().unwrap();

        for entity in entities {
            let new_entity = if entity.id == EntityId::default() {
                let id = self.store.next_id("block");
                Block {
                    id,
                    ..entity.clone()
                }
            } else {
                if blocks.contains_key(&entity.id) {
                    return Err(RepositoryError::DuplicateId {
                        entity: "Block",
                        id: entity.id,
                    });
                }
                entity.clone()
            };

            blocks.insert(new_entity.id, new_entity.clone());
            junction_set(
                &self.store.jn_inline_element_from_block_elements,
                new_entity.id,
                new_entity.elements.clone(),
            );
            junction_set(
                &self.store.jn_list_from_block_list,
                new_entity.id,
                new_entity.list.into_iter().collect(),
            );
            created.push(new_entity);
        }
        Ok(created)
    }

    fn get(&self, id: &EntityId) -> Result<Option<Block>, RepositoryError> {
        let blocks = self.store.blocks.read().unwrap();
        match blocks.get(id) {
            Some(entity) => {
                let mut e = entity.clone();
                drop(blocks);
                self.hydrate(&mut e);
                Ok(Some(e))
            }
            None => Ok(None),
        }
    }

    fn get_multi(&self, ids: &[EntityId]) -> Result<Vec<Option<Block>>, RepositoryError> {
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            result.push(self.get(id)?);
        }
        Ok(result)
    }

    fn get_all(&self) -> Result<Vec<Block>, RepositoryError> {
        let blocks = self.store.blocks.read().unwrap();
        let entries: Vec<Block> = blocks.values().cloned().collect();
        drop(blocks);
        let mut result = Vec::with_capacity(entries.len());
        for mut entity in entries {
            self.hydrate(&mut entity);
            result.push(entity);
        }
        Ok(result)
    }

    fn update(&mut self, entity: &Block) -> Result<Block, RepositoryError> {
        self.update_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn update_multi(&mut self, entities: &[Block]) -> Result<Vec<Block>, RepositoryError> {
        let mut blocks = self.store.blocks.write().unwrap();
        for entity in entities {
            blocks.insert(entity.id, entity.clone());
        }
        drop(blocks);
        let ids: Vec<EntityId> = entities.iter().map(|e| e.id).collect();
        let result = self.get_multi(&ids)?;
        Ok(result.into_iter().flatten().collect())
    }

    fn update_with_relationships(&mut self, entity: &Block) -> Result<Block, RepositoryError> {
        self.update_with_relationships_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn update_with_relationships_multi(
        &mut self,
        entities: &[Block],
    ) -> Result<Vec<Block>, RepositoryError> {
        let mut blocks = self.store.blocks.write().unwrap();
        for entity in entities {
            blocks.insert(entity.id, entity.clone());
            junction_set(
                &self.store.jn_inline_element_from_block_elements,
                entity.id,
                entity.elements.clone(),
            );
            junction_set(
                &self.store.jn_list_from_block_list,
                entity.id,
                entity.list.into_iter().collect(),
            );
        }
        drop(blocks);
        let ids: Vec<EntityId> = entities.iter().map(|e| e.id).collect();
        let result = self.get_multi(&ids)?;
        Ok(result.into_iter().flatten().collect())
    }

    fn remove(&mut self, id: &EntityId) -> Result<(), RepositoryError> {
        self.remove_multi(std::slice::from_ref(id))
    }

    fn remove_multi(&mut self, ids: &[EntityId]) -> Result<(), RepositoryError> {
        let mut blocks = self.store.blocks.write().unwrap();
        for id in ids {
            blocks.remove(id);
            junction_remove(&self.store.jn_inline_element_from_block_elements, id);
            junction_remove(&self.store.jn_list_from_block_list, id);
            delete_from_backward_junction(&self.store.jn_back_frame_blocks, id);
        }
        Ok(())
    }

    impl_write_relationship_methods!(BlockHashMapTable<'a>, BlockRelationshipField);

    fn snapshot_rows(&self, ids: &[EntityId]) -> Result<TableLevelSnapshot, RepositoryError> {
        let blocks = self.store.blocks.read().unwrap();
        let mut rows = Vec::new();
        for id in ids {
            if let Some(entity) = blocks.get(id) {
                let bytes = postcard::to_allocvec(entity)
                    .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
                rows.push((*id, bytes));
            }
        }

        let forward_junctions = vec![
            junction_snapshot(
                &self.store.jn_inline_element_from_block_elements,
                ids,
                "inline_element_from_block_elements_junction",
            ),
            junction_snapshot(
                &self.store.jn_list_from_block_list,
                ids,
                "list_from_block_list_junction",
            ),
        ];

        let mut backward_junctions = Vec::new();
        if let Some(snap) = junction_snapshot_backward(
            &self.store.jn_back_frame_blocks,
            ids,
            "block_from_frame_blocks_junction",
        ) {
            backward_junctions.push(snap);
        }

        Ok(TableLevelSnapshot {
            entity_rows: TableSnapshot {
                table_name: "block".to_string(),
                rows,
            },
            forward_junctions,
            backward_junctions,
        })
    }

    fn restore_rows(&mut self, snap: &TableLevelSnapshot) -> Result<(), RepositoryError> {
        let mut blocks = self.store.blocks.write().unwrap();
        for (id, bytes) in &snap.entity_rows.rows {
            let entity: Block = postcard::from_bytes(bytes)
                .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
            blocks.insert(*id, entity);
        }
        drop(blocks);
        for js in &snap.forward_junctions {
            match js.table_name.as_str() {
                "inline_element_from_block_elements_junction" => {
                    junction_restore(&self.store.jn_inline_element_from_block_elements, js);
                }
                "list_from_block_list_junction" => {
                    junction_restore(&self.store.jn_list_from_block_list, js);
                }
                _ => {}
            }
        }
        for js in &snap.backward_junctions {
            if js.table_name == "block_from_frame_blocks_junction" {
                junction_restore(&self.store.jn_back_frame_blocks, js);
            }
        }
        Ok(())
    }
}

pub struct BlockHashMapTableRO<'a> {
    store: &'a HashMapStore,
}

impl<'a> BlockHashMapTableRO<'a> {
    pub fn new(store: &'a HashMapStore) -> Self {
        Self { store }
    }

    fn resolve_junction(
        &self,
        field: &BlockRelationshipField,
    ) -> &RwLock<HashMap<EntityId, Vec<EntityId>>> {
        match field {
            BlockRelationshipField::Elements => &self.store.jn_inline_element_from_block_elements,
            BlockRelationshipField::List => &self.store.jn_list_from_block_list,
        }
    }

    fn hydrate(&self, entity: &mut Block) {
        entity.elements = junction_get(
            &self.store.jn_inline_element_from_block_elements,
            &entity.id,
        );
        entity.list = junction_get(&self.store.jn_list_from_block_list, &entity.id)
            .into_iter()
            .next();
    }
}

impl<'a> BlockTableRO for BlockHashMapTableRO<'a> {
    fn get(&self, id: &EntityId) -> Result<Option<Block>, RepositoryError> {
        let blocks = self.store.blocks.read().unwrap();
        match blocks.get(id) {
            Some(entity) => {
                let mut e = entity.clone();
                drop(blocks);
                self.hydrate(&mut e);
                Ok(Some(e))
            }
            None => Ok(None),
        }
    }

    fn get_multi(&self, ids: &[EntityId]) -> Result<Vec<Option<Block>>, RepositoryError> {
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            result.push(self.get(id)?);
        }
        Ok(result)
    }

    fn get_all(&self) -> Result<Vec<Block>, RepositoryError> {
        let blocks = self.store.blocks.read().unwrap();
        let entries: Vec<Block> = blocks.values().cloned().collect();
        drop(blocks);
        let mut result = Vec::with_capacity(entries.len());
        for mut entity in entries {
            self.hydrate(&mut entity);
            result.push(entity);
        }
        Ok(result)
    }

    impl_relationship_methods!(BlockHashMapTableRO<'a>, BlockRelationshipField);
}
