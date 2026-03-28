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

use super::table_cell_repository::{TableCellRelationshipField, TableCellTable, TableCellTableRO};

pub struct TableCellHashMapTable<'a> {
    store: &'a HashMapStore,
}

impl<'a> TableCellHashMapTable<'a> {
    pub fn new(store: &'a HashMapStore) -> Self {
        Self { store }
    }

    fn resolve_junction(
        &self,
        field: &TableCellRelationshipField,
    ) -> &RwLock<HashMap<EntityId, Vec<EntityId>>> {
        match field {
            TableCellRelationshipField::CellFrame => {
                &self.store.jn_frame_from_table_cell_cell_frame
            }
        }
    }

    fn hydrate(&self, entity: &mut TableCell) {
        entity.cell_frame = junction_get(
            &self.store.jn_frame_from_table_cell_cell_frame,
            &entity.id,
        )
        .into_iter()
        .next();
    }
}

impl<'a> TableCellTable for TableCellHashMapTable<'a> {
    fn create(&mut self, entity: &TableCell) -> Result<TableCell, RepositoryError> {
        self.create_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn create_multi(&mut self, entities: &[TableCell]) -> Result<Vec<TableCell>, RepositoryError> {
        let mut created = Vec::with_capacity(entities.len());
        let mut cells = self.store.table_cells.write().unwrap();

        for entity in entities {
            let new_entity = if entity.id == EntityId::default() {
                let id = self.store.next_id("table_cell");
                TableCell {
                    id,
                    ..entity.clone()
                }
            } else {
                if cells.contains_key(&entity.id) {
                    return Err(RepositoryError::DuplicateId {
                        entity: "TableCell",
                        id: entity.id,
                    });
                }
                entity.clone()
            };

            cells.insert(new_entity.id, new_entity.clone());
            junction_set(
                &self.store.jn_frame_from_table_cell_cell_frame,
                new_entity.id,
                new_entity.cell_frame.into_iter().collect(),
            );
            created.push(new_entity);
        }
        Ok(created)
    }

    fn get(&self, id: &EntityId) -> Result<Option<TableCell>, RepositoryError> {
        let cells = self.store.table_cells.read().unwrap();
        match cells.get(id) {
            Some(entity) => {
                let mut e = entity.clone();
                drop(cells);
                self.hydrate(&mut e);
                Ok(Some(e))
            }
            None => Ok(None),
        }
    }

    fn get_multi(&self, ids: &[EntityId]) -> Result<Vec<Option<TableCell>>, RepositoryError> {
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            result.push(self.get(id)?);
        }
        Ok(result)
    }

    fn get_all(&self) -> Result<Vec<TableCell>, RepositoryError> {
        let cells = self.store.table_cells.read().unwrap();
        let entries: Vec<TableCell> = cells.values().cloned().collect();
        drop(cells);
        let mut result = Vec::with_capacity(entries.len());
        for mut entity in entries {
            self.hydrate(&mut entity);
            result.push(entity);
        }
        Ok(result)
    }

    fn update(&mut self, entity: &TableCell) -> Result<TableCell, RepositoryError> {
        self.update_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn update_multi(
        &mut self,
        entities: &[TableCell],
    ) -> Result<Vec<TableCell>, RepositoryError> {
        let mut cells = self.store.table_cells.write().unwrap();
        for entity in entities {
            cells.insert(entity.id, entity.clone());
        }
        drop(cells);
        let ids: Vec<EntityId> = entities.iter().map(|e| e.id).collect();
        let result = self.get_multi(&ids)?;
        Ok(result.into_iter().flatten().collect())
    }

    fn update_with_relationships(
        &mut self,
        entity: &TableCell,
    ) -> Result<TableCell, RepositoryError> {
        self.update_with_relationships_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn update_with_relationships_multi(
        &mut self,
        entities: &[TableCell],
    ) -> Result<Vec<TableCell>, RepositoryError> {
        let mut cells = self.store.table_cells.write().unwrap();
        for entity in entities {
            cells.insert(entity.id, entity.clone());
            junction_set(
                &self.store.jn_frame_from_table_cell_cell_frame,
                entity.id,
                entity.cell_frame.into_iter().collect(),
            );
        }
        drop(cells);
        let ids: Vec<EntityId> = entities.iter().map(|e| e.id).collect();
        let result = self.get_multi(&ids)?;
        Ok(result.into_iter().flatten().collect())
    }

    fn remove(&mut self, id: &EntityId) -> Result<(), RepositoryError> {
        self.remove_multi(std::slice::from_ref(id))
    }

    fn remove_multi(&mut self, ids: &[EntityId]) -> Result<(), RepositoryError> {
        let mut cells = self.store.table_cells.write().unwrap();
        for id in ids {
            cells.remove(id);
            junction_remove(&self.store.jn_frame_from_table_cell_cell_frame, id);
            // backward: from table cells junction
            delete_from_backward_junction(&self.store.jn_table_cell_from_table_cells, id);
        }
        Ok(())
    }

    impl_write_relationship_methods!(TableCellHashMapTable<'a>, TableCellRelationshipField);

    fn snapshot_rows(&self, ids: &[EntityId]) -> Result<TableLevelSnapshot, RepositoryError> {
        let cells = self.store.table_cells.read().unwrap();
        let mut rows = Vec::new();
        for id in ids {
            if let Some(entity) = cells.get(id) {
                let bytes = postcard::to_allocvec(entity)
                    .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
                rows.push((*id, bytes));
            }
        }

        let forward_junctions = vec![junction_snapshot(
            &self.store.jn_frame_from_table_cell_cell_frame,
            ids,
            "frame_from_table_cell_cell_frame_junction",
        )];

        let mut backward_junctions = Vec::new();
        if let Some(snap) = junction_snapshot_backward(
            &self.store.jn_table_cell_from_table_cells,
            ids,
            "table_cell_from_table_cells_junction",
        ) {
            backward_junctions.push(snap);
        }

        Ok(TableLevelSnapshot {
            entity_rows: TableSnapshot {
                table_name: "table_cell".to_string(),
                rows,
            },
            forward_junctions,
            backward_junctions,
        })
    }

    fn restore_rows(&mut self, snap: &TableLevelSnapshot) -> Result<(), RepositoryError> {
        let mut cells = self.store.table_cells.write().unwrap();
        for (id, bytes) in &snap.entity_rows.rows {
            let entity: TableCell = postcard::from_bytes(bytes)
                .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
            cells.insert(*id, entity);
        }
        drop(cells);
        for js in &snap.forward_junctions {
            if js.table_name == "frame_from_table_cell_cell_frame_junction" {
                junction_restore(&self.store.jn_frame_from_table_cell_cell_frame, js);
            }
        }
        for js in &snap.backward_junctions {
            if js.table_name == "table_cell_from_table_cells_junction" {
                junction_restore(&self.store.jn_table_cell_from_table_cells, js);
            }
        }
        Ok(())
    }
}

pub struct TableCellHashMapTableRO<'a> {
    store: &'a HashMapStore,
}

impl<'a> TableCellHashMapTableRO<'a> {
    pub fn new(store: &'a HashMapStore) -> Self {
        Self { store }
    }

    fn resolve_junction(
        &self,
        field: &TableCellRelationshipField,
    ) -> &RwLock<HashMap<EntityId, Vec<EntityId>>> {
        match field {
            TableCellRelationshipField::CellFrame => {
                &self.store.jn_frame_from_table_cell_cell_frame
            }
        }
    }

    fn hydrate(&self, entity: &mut TableCell) {
        entity.cell_frame = junction_get(
            &self.store.jn_frame_from_table_cell_cell_frame,
            &entity.id,
        )
        .into_iter()
        .next();
    }
}

impl<'a> TableCellTableRO for TableCellHashMapTableRO<'a> {
    fn get(&self, id: &EntityId) -> Result<Option<TableCell>, RepositoryError> {
        let cells = self.store.table_cells.read().unwrap();
        match cells.get(id) {
            Some(entity) => {
                let mut e = entity.clone();
                drop(cells);
                self.hydrate(&mut e);
                Ok(Some(e))
            }
            None => Ok(None),
        }
    }

    fn get_multi(&self, ids: &[EntityId]) -> Result<Vec<Option<TableCell>>, RepositoryError> {
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            result.push(self.get(id)?);
        }
        Ok(result)
    }

    fn get_all(&self) -> Result<Vec<TableCell>, RepositoryError> {
        let cells = self.store.table_cells.read().unwrap();
        let entries: Vec<TableCell> = cells.values().cloned().collect();
        drop(cells);
        let mut result = Vec::with_capacity(entries.len());
        for mut entity in entries {
            self.hydrate(&mut entity);
            result.push(entity);
        }
        Ok(result)
    }

    impl_relationship_methods!(TableCellHashMapTableRO<'a>, TableCellRelationshipField);
}
