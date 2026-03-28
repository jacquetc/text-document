use crate::impl_leaf_entity_table;
use crate::entities::*;
use super::inline_element_repository::{InlineElementTable, InlineElementTableRO};

impl_leaf_entity_table! {
    entity: InlineElement,
    entity_name: "inline_element",
    store_field: inline_elements,
    table_trait: InlineElementTable,
    table_ro_trait: InlineElementTableRO,
    table_struct: InlineElementHashMapTable,
    table_ro_struct: InlineElementHashMapTableRO,
    backward_junctions: [
        (jn_inline_element_from_block_elements, "inline_element_from_block_elements_junction")
    ],
}
