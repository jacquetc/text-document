use super::resource_repository::{ResourceTable, ResourceTableRO};
use crate::entities::*;
use crate::impl_leaf_entity_table;

impl_leaf_entity_table! {
    entity: Resource,
    entity_name: "resource",
    store_field: resources,
    table_trait: ResourceTable,
    table_ro_trait: ResourceTableRO,
    table_struct: ResourceHashMapTable,
    table_ro_struct: ResourceHashMapTableRO,
    backward_junctions: [
        (jn_resource_from_document_resources, "resource_from_document_resources_junction")
    ],
}
