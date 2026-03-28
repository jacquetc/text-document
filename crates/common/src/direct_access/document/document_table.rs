use crate::database::hashmap_store::{
    HashMapStore, delete_from_backward_junction, junction_get, junction_remove, junction_set,
};
use crate::entities::*;
use crate::error::RepositoryError;
use crate::types::EntityId;
use crate::{impl_relationship_methods, impl_write_relationship_methods};
use std::collections::HashMap;
use std::sync::RwLock;

use super::document_repository::{DocumentRelationshipField, DocumentTable, DocumentTableRO};

pub struct DocumentHashMapTable<'a> {
    store: &'a HashMapStore,
}

impl<'a> DocumentHashMapTable<'a> {
    pub fn new(store: &'a HashMapStore) -> Self {
        Self { store }
    }

    fn resolve_junction(
        &self,
        field: &DocumentRelationshipField,
    ) -> &RwLock<HashMap<EntityId, Vec<EntityId>>> {
        match field {
            DocumentRelationshipField::Frames => &self.store.jn_frame_from_document_frames,
            DocumentRelationshipField::Lists => &self.store.jn_list_from_document_lists,
            DocumentRelationshipField::Resources => &self.store.jn_resource_from_document_resources,
            DocumentRelationshipField::Tables => &self.store.jn_table_from_document_tables,
        }
    }

    fn hydrate(&self, entity: &mut Document) {
        entity.frames = junction_get(&self.store.jn_frame_from_document_frames, &entity.id);
        entity.lists = junction_get(&self.store.jn_list_from_document_lists, &entity.id);
        entity.resources =
            junction_get(&self.store.jn_resource_from_document_resources, &entity.id);
        entity.tables = junction_get(&self.store.jn_table_from_document_tables, &entity.id);
    }
}

impl<'a> DocumentTable for DocumentHashMapTable<'a> {
    fn create(&mut self, entity: &Document) -> Result<Document, RepositoryError> {
        self.create_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn create_multi(&mut self, entities: &[Document]) -> Result<Vec<Document>, RepositoryError> {
        let mut created = Vec::with_capacity(entities.len());
        let mut docs = self.store.documents.write().unwrap();

        for entity in entities {
            let new_entity = if entity.id == EntityId::default() {
                let id = self.store.next_id("document");
                Document {
                    id,
                    ..entity.clone()
                }
            } else {
                if docs.contains_key(&entity.id) {
                    return Err(RepositoryError::DuplicateId {
                        entity: "Document",
                        id: entity.id,
                    });
                }
                entity.clone()
            };

            docs.insert(new_entity.id, new_entity.clone());
            junction_set(
                &self.store.jn_frame_from_document_frames,
                new_entity.id,
                new_entity.frames.clone(),
            );
            junction_set(
                &self.store.jn_list_from_document_lists,
                new_entity.id,
                new_entity.lists.clone(),
            );
            junction_set(
                &self.store.jn_resource_from_document_resources,
                new_entity.id,
                new_entity.resources.clone(),
            );
            junction_set(
                &self.store.jn_table_from_document_tables,
                new_entity.id,
                new_entity.tables.clone(),
            );
            created.push(new_entity);
        }
        Ok(created)
    }

    fn get(&self, id: &EntityId) -> Result<Option<Document>, RepositoryError> {
        let docs = self.store.documents.read().unwrap();
        match docs.get(id) {
            Some(entity) => {
                let mut e = entity.clone();
                drop(docs);
                self.hydrate(&mut e);
                Ok(Some(e))
            }
            None => Ok(None),
        }
    }

    fn get_multi(&self, ids: &[EntityId]) -> Result<Vec<Option<Document>>, RepositoryError> {
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            result.push(self.get(id)?);
        }
        Ok(result)
    }

    fn get_all(&self) -> Result<Vec<Document>, RepositoryError> {
        let docs = self.store.documents.read().unwrap();
        let entries: Vec<Document> = docs.values().cloned().collect();
        drop(docs);
        let mut result = Vec::with_capacity(entries.len());
        for mut entity in entries {
            self.hydrate(&mut entity);
            result.push(entity);
        }
        Ok(result)
    }

    fn update(&mut self, entity: &Document) -> Result<Document, RepositoryError> {
        self.update_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn update_multi(&mut self, entities: &[Document]) -> Result<Vec<Document>, RepositoryError> {
        let mut docs = self.store.documents.write().unwrap();
        for entity in entities {
            docs.insert(entity.id, entity.clone());
        }
        drop(docs);
        let ids: Vec<EntityId> = entities.iter().map(|e| e.id).collect();
        let result = self.get_multi(&ids)?;
        Ok(result.into_iter().flatten().collect())
    }

    fn update_with_relationships(
        &mut self,
        entity: &Document,
    ) -> Result<Document, RepositoryError> {
        self.update_with_relationships_multi(std::slice::from_ref(entity))
            .map(|v| v.into_iter().next().unwrap())
    }

    fn update_with_relationships_multi(
        &mut self,
        entities: &[Document],
    ) -> Result<Vec<Document>, RepositoryError> {
        let mut docs = self.store.documents.write().unwrap();
        for entity in entities {
            docs.insert(entity.id, entity.clone());
            junction_set(
                &self.store.jn_frame_from_document_frames,
                entity.id,
                entity.frames.clone(),
            );
            junction_set(
                &self.store.jn_list_from_document_lists,
                entity.id,
                entity.lists.clone(),
            );
            junction_set(
                &self.store.jn_resource_from_document_resources,
                entity.id,
                entity.resources.clone(),
            );
            junction_set(
                &self.store.jn_table_from_document_tables,
                entity.id,
                entity.tables.clone(),
            );
        }
        drop(docs);
        let ids: Vec<EntityId> = entities.iter().map(|e| e.id).collect();
        let result = self.get_multi(&ids)?;
        Ok(result.into_iter().flatten().collect())
    }

    fn remove(&mut self, id: &EntityId) -> Result<(), RepositoryError> {
        self.remove_multi(std::slice::from_ref(id))
    }

    fn remove_multi(&mut self, ids: &[EntityId]) -> Result<(), RepositoryError> {
        let mut docs = self.store.documents.write().unwrap();
        for id in ids {
            docs.remove(id);
            junction_remove(&self.store.jn_frame_from_document_frames, id);
            junction_remove(&self.store.jn_list_from_document_lists, id);
            junction_remove(&self.store.jn_resource_from_document_resources, id);
            junction_remove(&self.store.jn_table_from_document_tables, id);
            delete_from_backward_junction(&self.store.jn_document_from_root_document, id);
        }
        Ok(())
    }

    impl_write_relationship_methods!(DocumentHashMapTable<'a>, DocumentRelationshipField);
}

pub struct DocumentHashMapTableRO<'a> {
    store: &'a HashMapStore,
}

impl<'a> DocumentHashMapTableRO<'a> {
    pub fn new(store: &'a HashMapStore) -> Self {
        Self { store }
    }

    fn resolve_junction(
        &self,
        field: &DocumentRelationshipField,
    ) -> &RwLock<HashMap<EntityId, Vec<EntityId>>> {
        match field {
            DocumentRelationshipField::Frames => &self.store.jn_frame_from_document_frames,
            DocumentRelationshipField::Lists => &self.store.jn_list_from_document_lists,
            DocumentRelationshipField::Resources => &self.store.jn_resource_from_document_resources,
            DocumentRelationshipField::Tables => &self.store.jn_table_from_document_tables,
        }
    }

    fn hydrate(&self, entity: &mut Document) {
        entity.frames = junction_get(&self.store.jn_frame_from_document_frames, &entity.id);
        entity.lists = junction_get(&self.store.jn_list_from_document_lists, &entity.id);
        entity.resources =
            junction_get(&self.store.jn_resource_from_document_resources, &entity.id);
        entity.tables = junction_get(&self.store.jn_table_from_document_tables, &entity.id);
    }
}

impl<'a> DocumentTableRO for DocumentHashMapTableRO<'a> {
    fn get(&self, id: &EntityId) -> Result<Option<Document>, RepositoryError> {
        let docs = self.store.documents.read().unwrap();
        match docs.get(id) {
            Some(entity) => {
                let mut e = entity.clone();
                drop(docs);
                self.hydrate(&mut e);
                Ok(Some(e))
            }
            None => Ok(None),
        }
    }

    fn get_multi(&self, ids: &[EntityId]) -> Result<Vec<Option<Document>>, RepositoryError> {
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            result.push(self.get(id)?);
        }
        Ok(result)
    }

    fn get_all(&self) -> Result<Vec<Document>, RepositoryError> {
        let docs = self.store.documents.read().unwrap();
        let entries: Vec<Document> = docs.values().cloned().collect();
        drop(docs);
        let mut result = Vec::with_capacity(entries.len());
        for mut entity in entries {
            self.hydrate(&mut entity);
            result.push(entity);
        }
        Ok(result)
    }

    impl_relationship_methods!(DocumentHashMapTableRO<'a>, DocumentRelationshipField);
}
