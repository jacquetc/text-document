use super::list_repository::{ListTable, ListTableRO};
use crate::entities::*;
use crate::impl_leaf_entity_table;

impl_leaf_entity_table! {
    entity: List,
    entity_name: "list",
    store_field: lists,
    table_trait: ListTable,
    table_ro_trait: ListTableRO,
    table_struct: ListHashMapTable,
    table_ro_struct: ListHashMapTableRO,
    backward_junctions: [
        (jn_list_from_block_list, "list_from_block_list_junction"),
        (jn_list_from_document_lists, "list_from_document_lists_junction")
    ],
}
