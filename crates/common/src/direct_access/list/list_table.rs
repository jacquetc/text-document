use crate::impl_leaf_entity_table;
use crate::entities::*;
use super::list_repository::{ListTable, ListTableRO};

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
