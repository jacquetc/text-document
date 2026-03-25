use crate::ExtractFragmentDto;
use crate::ExtractFragmentResultDto;
use anyhow::{Result, anyhow};
use common::database::QueryUnitOfWork;
use common::direct_access::block::block_repository::BlockRelationshipField;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::entities::{Block, InlineContent, InlineElement, List, Root};
use common::parser_tools::fragment_schema::{
    FragmentBlock, FragmentData, FragmentElement, FragmentList,
};
use common::types::{EntityId, ROOT_ENTITY_ID};

pub trait ExtractFragmentUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn ExtractFragmentUnitOfWorkTrait>;
}

#[macros::uow_action(entity = "Root", action = "GetRO")]
#[macros::uow_action(entity = "Root", action = "GetRelationshipRO")]
#[macros::uow_action(entity = "Document", action = "GetRelationshipRO")]
#[macros::uow_action(entity = "Frame", action = "GetRelationshipRO")]
#[macros::uow_action(entity = "Block", action = "GetMultiRO")]
#[macros::uow_action(entity = "Block", action = "GetRelationshipRO")]
#[macros::uow_action(entity = "InlineElement", action = "GetMultiRO")]
#[macros::uow_action(entity = "List", action = "GetRO")]
pub trait ExtractFragmentUnitOfWorkTrait: QueryUnitOfWork {}

pub struct ExtractFragmentUseCase {
    uow_factory: Box<dyn ExtractFragmentUnitOfWorkFactoryTrait>,
}

impl ExtractFragmentUseCase {
    pub fn new(uow_factory: Box<dyn ExtractFragmentUnitOfWorkFactoryTrait>) -> Self {
        ExtractFragmentUseCase { uow_factory }
    }

    pub fn execute(&mut self, dto: &ExtractFragmentDto) -> Result<ExtractFragmentResultDto> {
        let uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let start = dto.position.min(dto.anchor);
        let end = dto.position.max(dto.anchor);

        // Empty range
        if start == end {
            uow.end_transaction()?;
            let empty = FragmentData { blocks: vec![] };
            return Ok(ExtractFragmentResultDto {
                fragment_data: serde_json::to_string(&empty)?,
                plain_text: String::new(),
            });
        }

        // Get Root -> Document -> Frames -> Blocks
        let root = uow
            .get_root(&ROOT_ENTITY_ID)?
            .ok_or_else(|| anyhow!("Root entity not found"))?;
        let doc_ids = uow.get_root_relationship(&root.id, &RootRelationshipField::Document)?;
        let doc_id = *doc_ids
            .first()
            .ok_or_else(|| anyhow!("Root has no document"))?;

        let frame_ids =
            uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;

        // Collect blocks from all frames, not just the first one
        let mut all_block_ids: Vec<EntityId> = Vec::new();
        for frame_id in &frame_ids {
            let block_ids =
                uow.get_frame_relationship(frame_id, &FrameRelationshipField::Blocks)?;
            all_block_ids.extend(block_ids);
        }

        let blocks_opt = uow.get_block_multi(&all_block_ids)?;
        let mut blocks: Vec<Block> = blocks_opt.into_iter().flatten().collect();
        blocks.sort_by_key(|b| b.document_position);

        let mut fragment_blocks: Vec<FragmentBlock> = Vec::new();
        let mut plain_texts: Vec<String> = Vec::new();

        for block in &blocks {
            let block_start = block.document_position;
            let block_end = block_start + block.text_length;

            // Skip blocks entirely outside range
            if block_end < start || block_start >= end {
                continue;
            }

            // Calculate offsets within this block
            let local_start = if start > block_start {
                (start - block_start) as usize
            } else {
                0
            };
            let local_end = if end < block_end {
                (end - block_start) as usize
            } else {
                block.text_length as usize
            };

            // Get elements
            let element_ids =
                uow.get_block_relationship(&block.id, &BlockRelationshipField::Elements)?;
            let elements_opt = uow.get_inline_element_multi(&element_ids)?;
            let elements: Vec<InlineElement> = elements_opt.into_iter().flatten().collect();

            // Get list if any
            let list = if let Some(list_id) = block.list {
                uow.get_list(&list_id)?
            } else {
                None
            };

            // Extract elements within the local range
            let (extracted_elements, extracted_text) =
                extract_elements_in_range(&elements, local_start, local_end);

            let is_full_block = local_start == 0 && local_end == block.text_length as usize;

            let fragment_block = FragmentBlock {
                plain_text: extracted_text.clone(),
                elements: extracted_elements,
                heading_level: if is_full_block {
                    block.fmt_heading_level
                } else {
                    None
                },
                list: if is_full_block {
                    list.as_ref().map(FragmentList::from_entity)
                } else {
                    None
                },
                alignment: if is_full_block {
                    block.fmt_alignment.clone()
                } else {
                    None
                },
                indent: if is_full_block {
                    block.fmt_indent
                } else {
                    None
                },
                text_indent: if is_full_block {
                    block.fmt_text_indent
                } else {
                    None
                },
                marker: if is_full_block {
                    block.fmt_marker.clone()
                } else {
                    None
                },
                top_margin: if is_full_block {
                    block.fmt_top_margin
                } else {
                    None
                },
                bottom_margin: if is_full_block {
                    block.fmt_bottom_margin
                } else {
                    None
                },
                left_margin: if is_full_block {
                    block.fmt_left_margin
                } else {
                    None
                },
                right_margin: if is_full_block {
                    block.fmt_right_margin
                } else {
                    None
                },
                tab_positions: if is_full_block {
                    block.fmt_tab_positions.clone()
                } else {
                    vec![]
                },
                line_height: if is_full_block {
                    block.fmt_line_height
                } else {
                    None
                },
                non_breakable_lines: if is_full_block {
                    block.fmt_non_breakable_lines
                } else {
                    None
                },
                direction: if is_full_block {
                    block.fmt_direction.clone()
                } else {
                    None
                },
                background_color: if is_full_block {
                    block.fmt_background_color.clone()
                } else {
                    None
                },
            };

            plain_texts.push(extracted_text);
            fragment_blocks.push(fragment_block);
        }

        let fragment_data = FragmentData {
            blocks: fragment_blocks,
        };

        let fragment_json = serde_json::to_string(&fragment_data)?;
        let plain_text = plain_texts.join("\n");

        uow.end_transaction()?;

        Ok(ExtractFragmentResultDto {
            fragment_data: fragment_json,
            plain_text,
        })
    }
}

/// Extract elements within a character range [local_start, local_end) of a block.
/// Returns the extracted FragmentElements and the concatenated plain text.
fn extract_elements_in_range(
    elements: &[InlineElement],
    local_start: usize,
    local_end: usize,
) -> (Vec<FragmentElement>, String) {
    let mut result_elements: Vec<FragmentElement> = Vec::new();
    let mut result_text = String::new();
    let mut char_cursor: usize = 0;

    for elem in elements {
        let elem_char_len = match &elem.content {
            InlineContent::Text(s) => s.chars().count(),
            InlineContent::Image { .. } => 1,
            InlineContent::Empty => 0,
        };

        let elem_start = char_cursor;
        let elem_end = char_cursor + elem_char_len;

        // Skip elements entirely before range
        if elem_end <= local_start {
            char_cursor = elem_end;
            continue;
        }
        // Stop if entirely after range
        if elem_start >= local_end {
            break;
        }

        // This element overlaps with [local_start, local_end)
        let take_start = local_start.saturating_sub(elem_start);
        let take_end = if local_end < elem_end {
            local_end - elem_start
        } else {
            elem_char_len
        };

        match &elem.content {
            InlineContent::Text(s) => {
                let chars: Vec<char> = s.chars().collect();
                let slice: String = chars[take_start..take_end].iter().collect();
                if !slice.is_empty() {
                    let mut fe = FragmentElement::from_entity(elem);
                    fe.content = InlineContent::Text(slice.clone());
                    result_elements.push(fe);
                    result_text.push_str(&slice);
                }
            }
            InlineContent::Image { .. } => {
                // Image is 1 char, include if in range
                if take_start == 0 && take_end == 1 {
                    result_elements.push(FragmentElement::from_entity(elem));
                    result_text.push('\u{FFFC}');
                }
            }
            InlineContent::Empty => {}
        }

        char_cursor = elem_end;
    }

    (result_elements, result_text)
}
