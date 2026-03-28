use crate::database::hashmap_store::{
    HashMapStore, delete_from_backward_junction, junction_get, junction_remove, junction_set,
};
use crate::entities::*;
use crate::error::RepositoryError;
use crate::types::EntityId;
use crate::{impl_relationship_methods, impl_write_relationship_methods};
use std::collections::HashMap;
use std::sync::RwLock;

use super::table_repository::{TableRelationshipField, TableTable, TableTableRO};

pub struct TableHashMapTable<'a> {
    store: &'a HashMapStore,
}

impl<'a> TableHashMapTable<'a> {
    pub fn new(store: &'a HashMapStore) -> Self {
        Self { store }
    }

    fn resolve_junction(
        &self,
        field: &TableRelationshipField,
    ) -> &RwLock<HashMap<EntityId, Vec<EntityId>>> {
        match field {
            TableRelationshipField::Cells => &self.store.jn_table_cell_from_table_cells,
        }
    }

    fn hydrate(&self, entity: &mut Table) {
        entity.cells = junction_get(&self.store.jn_table_cell_from_table_cells, &entity.id);
    }
}

impl<'a> TableTable for TableHashMapTable<'a> {
    fn create(&mut self, entity: &Table) -> Result<Table, RepositoryError> {
        self.create_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn create_multi(&mut self, entities: &[Table]) -> Result<Vec<Table>, RepositoryError> {
        let mut created = Vec::with_capacity(entities.len());
        let mut tables = self.store.tables.write().unwrap();

        for entity in entities {
            let new_entity = if entity.id == EntityId::default() {
                let id = self.store.next_id("table");
                Table {
                    id,
                    ..entity.clone()
                }
            } else {
                if tables.contains_key(&entity.id) {
                    return Err(RepositoryError::DuplicateId {
                        entity: "Table",
                        id: entity.id,
                    });
                }
                entity.clone()
            };

            tables.insert(new_entity.id, new_entity.clone());
            junction_set(
                &self.store.jn_table_cell_from_table_cells,
                new_entity.id,
                new_entity.cells.clone(),
            );
            created.push(new_entity);
        }
        Ok(created)
    }

    fn get(&self, id: &EntityId) -> Result<Option<Table>, RepositoryError> {
        let tables = self.store.tables.read().unwrap();
        match tables.get(id) {
            Some(entity) => {
                let mut e = entity.clone();
                drop(tables);
                self.hydrate(&mut e);
                Ok(Some(e))
            }
            None => Ok(None),
        }
    }

    fn get_multi(&self, ids: &[EntityId]) -> Result<Vec<Option<Table>>, RepositoryError> {
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            result.push(self.get(id)?);
        }
        Ok(result)
    }

    fn get_all(&self) -> Result<Vec<Table>, RepositoryError> {
        let tables = self.store.tables.read().unwrap();
        let entries: Vec<Table> = tables.values().cloned().collect();
        drop(tables);
        let mut result = Vec::with_capacity(entries.len());
        for mut entity in entries {
            self.hydrate(&mut entity);
            result.push(entity);
        }
        Ok(result)
    }

    fn update(&mut self, entity: &Table) -> Result<Table, RepositoryError> {
        self.update_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn update_multi(&mut self, entities: &[Table]) -> Result<Vec<Table>, RepositoryError> {
        let mut tables = self.store.tables.write().unwrap();
        for entity in entities {
            tables.insert(entity.id, entity.clone());
        }
        drop(tables);
        let ids: Vec<EntityId> = entities.iter().map(|e| e.id).collect();
        let result = self.get_multi(&ids)?;
        Ok(result.into_iter().flatten().collect())
    }

    fn update_with_relationships(&mut self, entity: &Table) -> Result<Table, RepositoryError> {
        self.update_with_relationships_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn update_with_relationships_multi(
        &mut self,
        entities: &[Table],
    ) -> Result<Vec<Table>, RepositoryError> {
        let mut tables = self.store.tables.write().unwrap();
        for entity in entities {
            tables.insert(entity.id, entity.clone());
            junction_set(
                &self.store.jn_table_cell_from_table_cells,
                entity.id,
                entity.cells.clone(),
            );
        }
        drop(tables);
        let ids: Vec<EntityId> = entities.iter().map(|e| e.id).collect();
        let result = self.get_multi(&ids)?;
        Ok(result.into_iter().flatten().collect())
    }

    fn remove(&mut self, id: &EntityId) -> Result<(), RepositoryError> {
        self.remove_multi(std::slice::from_ref(id))
    }

    fn remove_multi(&mut self, ids: &[EntityId]) -> Result<(), RepositoryError> {
        let mut tables = self.store.tables.write().unwrap();
        for id in ids {
            tables.remove(id);
            junction_remove(&self.store.jn_table_cell_from_table_cells, id);
            // backward: from document tables + frame table
            delete_from_backward_junction(&self.store.jn_table_from_document_tables, id);
            delete_from_backward_junction(&self.store.jn_table_from_frame_table, id);
        }
        Ok(())
    }

    impl_write_relationship_methods!(TableHashMapTable<'a>, TableRelationshipField);
}

pub struct TableHashMapTableRO<'a> {
    store: &'a HashMapStore,
}

impl<'a> TableHashMapTableRO<'a> {
    pub fn new(store: &'a HashMapStore) -> Self {
        Self { store }
    }

    fn resolve_junction(
        &self,
        field: &TableRelationshipField,
    ) -> &RwLock<HashMap<EntityId, Vec<EntityId>>> {
        match field {
            TableRelationshipField::Cells => &self.store.jn_table_cell_from_table_cells,
        }
    }

    fn hydrate(&self, entity: &mut Table) {
        entity.cells = junction_get(&self.store.jn_table_cell_from_table_cells, &entity.id);
    }
}

impl<'a> TableTableRO for TableHashMapTableRO<'a> {
    fn get(&self, id: &EntityId) -> Result<Option<Table>, RepositoryError> {
        let tables = self.store.tables.read().unwrap();
        match tables.get(id) {
            Some(entity) => {
                let mut e = entity.clone();
                drop(tables);
                self.hydrate(&mut e);
                Ok(Some(e))
            }
            None => Ok(None),
        }
    }

    fn get_multi(&self, ids: &[EntityId]) -> Result<Vec<Option<Table>>, RepositoryError> {
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            result.push(self.get(id)?);
        }
        Ok(result)
    }

    fn get_all(&self) -> Result<Vec<Table>, RepositoryError> {
        let tables = self.store.tables.read().unwrap();
        let entries: Vec<Table> = tables.values().cloned().collect();
        drop(tables);
        let mut result = Vec::with_capacity(entries.len());
        for mut entity in entries {
            self.hydrate(&mut entity);
            result.push(entity);
        }
        Ok(result)
    }

    impl_relationship_methods!(TableHashMapTableRO<'a>, TableRelationshipField);
}
