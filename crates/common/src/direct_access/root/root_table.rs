use crate::database::hashmap_store::{
    HashMapStore, junction_get, junction_get_relationships_from_right_ids, junction_move_ids,
    junction_remove, junction_set,
};
use crate::entities::*;
use crate::error::RepositoryError;
use crate::types::EntityId;
use std::collections::HashMap;
use std::sync::RwLock;

use super::root_repository::{RootRelationshipField, RootTable, RootTableRO};

pub struct RootHashMapTable<'a> {
    store: &'a HashMapStore,
}

impl<'a> RootHashMapTable<'a> {
    pub fn new(store: &'a HashMapStore) -> Self {
        Self { store }
    }

    fn resolve_junction(
        &self,
        field: &RootRelationshipField,
    ) -> &RwLock<HashMap<EntityId, Vec<EntityId>>> {
        match field {
            RootRelationshipField::Document => &self.store.jn_document_from_root_document,
        }
    }

    fn hydrate(&self, entity: &mut Root) -> bool {
        match junction_get(&self.store.jn_document_from_root_document, &entity.id)
            .into_iter()
            .next()
        {
            Some(val) => {
                entity.document = val;
                true
            }
            None => {
                log::warn!(
                    "Root {} has incomplete junction data (missing document), treating as not found",
                    entity.id
                );
                false
            }
        }
    }
}

impl<'a> RootTable for RootHashMapTable<'a> {
    fn create(&mut self, entity: &Root) -> Result<Root, RepositoryError> {
        self.create_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn create_multi(&mut self, entities: &[Root]) -> Result<Vec<Root>, RepositoryError> {
        let mut created = Vec::with_capacity(entities.len());
        let mut roots = self.store.roots.write().unwrap();

        for entity in entities {
            let new_entity = if entity.id == EntityId::default() {
                let id = self.store.next_id("root");
                Root {
                    id,
                    ..entity.clone()
                }
            } else {
                if roots.contains_key(&entity.id) {
                    return Err(RepositoryError::DuplicateId {
                        entity: "Root",
                        id: entity.id,
                    });
                }
                entity.clone()
            };

            // one-to-one constraint check
            {
                let jn = self.store.jn_document_from_root_document.read().unwrap();
                for (existing_id, right_ids) in jn.iter() {
                    if *existing_id != new_entity.id && right_ids.contains(&new_entity.document) {
                        panic!(
                            "One-to-one constraint violation: Document {} is already referenced by Root {}",
                            new_entity.document, existing_id
                        );
                    }
                }
            }

            roots.insert(new_entity.id, new_entity.clone());
            junction_set(
                &self.store.jn_document_from_root_document,
                new_entity.id,
                vec![new_entity.document],
            );
            created.push(new_entity);
        }
        Ok(created)
    }

    fn get(&self, id: &EntityId) -> Result<Option<Root>, RepositoryError> {
        let entity = {
            let roots = self.store.roots.read().unwrap();
            roots.get(id).cloned()
        };
        match entity {
            Some(mut e) => {
                let complete = self.hydrate(&mut e);
                Ok(if complete { Some(e) } else { None })
            }
            None => Ok(None),
        }
    }

    fn get_multi(&self, ids: &[EntityId]) -> Result<Vec<Option<Root>>, RepositoryError> {
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            result.push(self.get(id)?);
        }
        Ok(result)
    }

    fn get_all(&self) -> Result<Vec<Root>, RepositoryError> {
        let roots = self.store.roots.read().unwrap();
        let entries: Vec<Root> = roots.values().cloned().collect();
        drop(roots);
        let mut result = Vec::with_capacity(entries.len());
        for mut entity in entries {
            if self.hydrate(&mut entity) {
                result.push(entity);
            }
        }
        Ok(result)
    }

    fn update(&mut self, entity: &Root) -> Result<Root, RepositoryError> {
        self.update_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn update_multi(&mut self, entities: &[Root]) -> Result<Vec<Root>, RepositoryError> {
        let mut roots = self.store.roots.write().unwrap();
        for entity in entities {
            roots.insert(entity.id, entity.clone());
        }
        drop(roots);
        let ids: Vec<EntityId> = entities.iter().map(|e| e.id).collect();
        let result = self.get_multi(&ids)?;
        Ok(result.into_iter().flatten().collect())
    }

    fn update_with_relationships(&mut self, entity: &Root) -> Result<Root, RepositoryError> {
        self.update_with_relationships_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn update_with_relationships_multi(
        &mut self,
        entities: &[Root],
    ) -> Result<Vec<Root>, RepositoryError> {
        let mut roots = self.store.roots.write().unwrap();
        for entity in entities {
            // one-to-one constraint check
            {
                let jn = self.store.jn_document_from_root_document.read().unwrap();
                for (existing_id, right_ids) in jn.iter() {
                    if *existing_id != entity.id && right_ids.contains(&entity.document) {
                        panic!(
                            "One-to-one constraint violation: Document {} is already referenced by Root {}",
                            entity.document, existing_id
                        );
                    }
                }
            }
            roots.insert(entity.id, entity.clone());
            junction_set(
                &self.store.jn_document_from_root_document,
                entity.id,
                vec![entity.document],
            );
        }
        drop(roots);
        let ids: Vec<EntityId> = entities.iter().map(|e| e.id).collect();
        let result = self.get_multi(&ids)?;
        Ok(result.into_iter().flatten().collect())
    }

    fn remove(&mut self, id: &EntityId) -> Result<(), RepositoryError> {
        self.remove_multi(std::slice::from_ref(id))
    }

    fn remove_multi(&mut self, ids: &[EntityId]) -> Result<(), RepositoryError> {
        let mut roots = self.store.roots.write().unwrap();
        for id in ids {
            roots.remove(id);
            junction_remove(&self.store.jn_document_from_root_document, id);
        }
        Ok(())
    }

    fn get_relationship(
        &self,
        id: &EntityId,
        field: &RootRelationshipField,
    ) -> Result<Vec<EntityId>, RepositoryError> {
        Ok(junction_get(self.resolve_junction(field), id))
    }

    fn get_relationship_many(
        &self,
        ids: &[EntityId],
        field: &RootRelationshipField,
    ) -> Result<std::collections::HashMap<EntityId, Vec<EntityId>>, RepositoryError> {
        let jn = self.resolve_junction(field);
        let mut map = std::collections::HashMap::new();
        for id in ids {
            map.insert(*id, junction_get(jn, id));
        }
        Ok(map)
    }

    fn get_relationship_count(
        &self,
        id: &EntityId,
        field: &RootRelationshipField,
    ) -> Result<usize, RepositoryError> {
        Ok(junction_get(self.resolve_junction(field), id).len())
    }

    fn get_relationship_in_range(
        &self,
        id: &EntityId,
        field: &RootRelationshipField,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<EntityId>, RepositoryError> {
        let all = junction_get(self.resolve_junction(field), id);
        Ok(all.into_iter().skip(offset).take(limit).collect())
    }

    fn get_relationships_from_right_ids(
        &self,
        field: &RootRelationshipField,
        right_ids: &[EntityId],
    ) -> Result<Vec<(EntityId, Vec<EntityId>)>, RepositoryError> {
        Ok(junction_get_relationships_from_right_ids(
            self.resolve_junction(field),
            right_ids,
        ))
    }

    fn set_relationship_multi(
        &mut self,
        field: &RootRelationshipField,
        relationships: Vec<(EntityId, Vec<EntityId>)>,
    ) -> Result<(), RepositoryError> {
        let jn = self.resolve_junction(field);
        for (left_id, entities) in relationships {
            junction_set(jn, left_id, entities);
        }
        Ok(())
    }

    fn set_relationship(
        &mut self,
        id: &EntityId,
        field: &RootRelationshipField,
        right_ids: &[EntityId],
    ) -> Result<(), RepositoryError> {
        junction_set(self.resolve_junction(field), *id, right_ids.to_vec());
        Ok(())
    }

    fn move_relationship_ids(
        &mut self,
        id: &EntityId,
        field: &RootRelationshipField,
        ids_to_move: &[EntityId],
        new_index: i32,
    ) -> Result<Vec<EntityId>, RepositoryError> {
        Ok(junction_move_ids(
            self.resolve_junction(field),
            id,
            ids_to_move,
            new_index,
        ))
    }
}

pub struct RootHashMapTableRO<'a> {
    store: &'a HashMapStore,
}

impl<'a> RootHashMapTableRO<'a> {
    pub fn new(store: &'a HashMapStore) -> Self {
        Self { store }
    }

    fn resolve_junction(
        &self,
        field: &RootRelationshipField,
    ) -> &RwLock<HashMap<EntityId, Vec<EntityId>>> {
        match field {
            RootRelationshipField::Document => &self.store.jn_document_from_root_document,
        }
    }

    fn hydrate(&self, entity: &mut Root) -> bool {
        match junction_get(&self.store.jn_document_from_root_document, &entity.id)
            .into_iter()
            .next()
        {
            Some(val) => {
                entity.document = val;
                true
            }
            None => {
                log::warn!(
                    "Root {} has incomplete junction data (missing document), treating as not found",
                    entity.id
                );
                false
            }
        }
    }
}

impl<'a> RootTableRO for RootHashMapTableRO<'a> {
    fn get(&self, id: &EntityId) -> Result<Option<Root>, RepositoryError> {
        let roots = self.store.roots.read().unwrap();
        match roots.get(id) {
            Some(entity) => {
                let mut e = entity.clone();
                drop(roots);
                let complete = self.hydrate(&mut e);
                Ok(if complete { Some(e) } else { None })
            }
            None => Ok(None),
        }
    }

    fn get_multi(&self, ids: &[EntityId]) -> Result<Vec<Option<Root>>, RepositoryError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            result.push(self.get(id)?);
        }
        Ok(result)
    }

    fn get_all(&self) -> Result<Vec<Root>, RepositoryError> {
        let roots = self.store.roots.read().unwrap();
        let entries: Vec<Root> = roots.values().cloned().collect();
        drop(roots);
        let mut result = Vec::with_capacity(entries.len());
        for mut entity in entries {
            if self.hydrate(&mut entity) {
                result.push(entity);
            }
        }
        Ok(result)
    }

    fn get_relationship(
        &self,
        id: &EntityId,
        field: &RootRelationshipField,
    ) -> Result<Vec<EntityId>, RepositoryError> {
        Ok(junction_get(self.resolve_junction(field), id))
    }

    fn get_relationship_many(
        &self,
        ids: &[EntityId],
        field: &RootRelationshipField,
    ) -> Result<std::collections::HashMap<EntityId, Vec<EntityId>>, RepositoryError> {
        let jn = self.resolve_junction(field);
        let mut map = std::collections::HashMap::new();
        for id in ids {
            map.insert(*id, junction_get(jn, id));
        }
        Ok(map)
    }

    fn get_relationship_count(
        &self,
        id: &EntityId,
        field: &RootRelationshipField,
    ) -> Result<usize, RepositoryError> {
        Ok(junction_get(self.resolve_junction(field), id).len())
    }

    fn get_relationship_in_range(
        &self,
        id: &EntityId,
        field: &RootRelationshipField,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<EntityId>, RepositoryError> {
        let all = junction_get(self.resolve_junction(field), id);
        Ok(all.into_iter().skip(offset).take(limit).collect())
    }

    fn get_relationships_from_right_ids(
        &self,
        field: &RootRelationshipField,
        right_ids: &[EntityId],
    ) -> Result<Vec<(EntityId, Vec<EntityId>)>, RepositoryError> {
        Ok(junction_get_relationships_from_right_ids(
            self.resolve_junction(field),
            right_ids,
        ))
    }
}
