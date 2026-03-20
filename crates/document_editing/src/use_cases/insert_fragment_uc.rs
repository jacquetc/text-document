use crate::InsertFragmentDto;
use crate::InsertFragmentResultDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::direct_access::block::block_repository::BlockRelationshipField;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::entities::{Block, Document, Frame, InlineContent, InlineElement, List, Root};
use common::parser_tools::fragment_schema::FragmentData;
use common::snapshot::EntityTreeSnapshot;
use common::types::EntityId;
use common::undo_redo::UndoRedoCommand;
use std::any::Any;

pub trait InsertFragmentUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn InsertFragmentUnitOfWorkTrait>;
}

#[macros::uow_action(entity = "Root", action = "Get")]
#[macros::uow_action(entity = "Root", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Get")]
#[macros::uow_action(entity = "Document", action = "Update")]
#[macros::uow_action(entity = "Document", action = "GetRelationship")]
#[macros::uow_action(entity = "Document", action = "Snapshot")]
#[macros::uow_action(entity = "Document", action = "Restore")]
#[macros::uow_action(entity = "Frame", action = "Get")]
#[macros::uow_action(entity = "Frame", action = "Update")]
#[macros::uow_action(entity = "Frame", action = "GetRelationship")]
#[macros::uow_action(entity = "Block", action = "Get")]
#[macros::uow_action(entity = "Block", action = "GetMulti")]
#[macros::uow_action(entity = "Block", action = "Update")]
#[macros::uow_action(entity = "Block", action = "UpdateMulti")]
#[macros::uow_action(entity = "Block", action = "Create")]
#[macros::uow_action(entity = "Block", action = "GetRelationship")]
#[macros::uow_action(entity = "InlineElement", action = "Get")]
#[macros::uow_action(entity = "InlineElement", action = "GetMulti")]
#[macros::uow_action(entity = "InlineElement", action = "Update")]
#[macros::uow_action(entity = "InlineElement", action = "Create")]
#[macros::uow_action(entity = "List", action = "Create")]
pub trait InsertFragmentUnitOfWorkTrait: CommandUnitOfWork {}

fn find_block_at_position(blocks: &[Block], position: i64) -> Result<(Block, usize, i64)> {
    for (i, block) in blocks.iter().enumerate() {
        let block_start = block.document_position;
        let block_end = block_start + block.text_length;
        if position >= block_start && position <= block_end {
            let offset = position - block_start;
            return Ok((block.clone(), i, offset));
        }
    }
    if let Some(block) = blocks.last() {
        let offset = block.text_length;
        return Ok((block.clone(), blocks.len() - 1, offset));
    }
    Err(anyhow!("No blocks found in document"))
}

fn execute_insert_fragment(
    uow: &mut Box<dyn InsertFragmentUnitOfWorkTrait>,
    dto: &InsertFragmentDto,
) -> Result<(InsertFragmentResultDto, EntityTreeSnapshot)> {
    let fragment_data: FragmentData = serde_json::from_str(&dto.fragment_data)
        .map_err(|e| anyhow!("Invalid fragment_data JSON: {}", e))?;

    if fragment_data.blocks.is_empty() {
        return Err(anyhow!("Fragment contains no blocks"));
    }

    let root = uow
        .get_root(&1)?
        .ok_or_else(|| anyhow!("Root entity not found"))?;
    let doc_ids = uow.get_root_relationship(&root.id, &RootRelationshipField::Document)?;
    let doc_id = *doc_ids
        .first()
        .ok_or_else(|| anyhow!("Root has no document"))?;

    let document = uow
        .get_document(&doc_id)?
        .ok_or_else(|| anyhow!("Document not found"))?;

    let snapshot = uow.snapshot_document(&[doc_id])?;

    if dto.position != dto.anchor {
        return Err(anyhow!(
            "Selection replacement is not supported. Use delete_text first."
        ));
    }

    let frame_ids = uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
    let frame_id = *frame_ids
        .first()
        .ok_or_else(|| anyhow!("Document has no frames"))?;

    let frame = uow
        .get_frame(&frame_id)?
        .ok_or_else(|| anyhow!("Frame not found"))?;

    let block_ids = uow.get_frame_relationship(&frame_id, &FrameRelationshipField::Blocks)?;
    let blocks_opt = uow.get_block_multi(&block_ids)?;
    let mut blocks: Vec<Block> = blocks_opt.into_iter().filter_map(|b| b).collect();
    blocks.sort_by_key(|b| b.document_position);

    let (current_block, block_idx, offset) = find_block_at_position(&blocks, dto.position)?;

    // Get current block's elements for splitting
    let element_ids =
        uow.get_block_relationship(&current_block.id, &BlockRelationshipField::Elements)?;
    let elements_opt = uow.get_inline_element_multi(&element_ids)?;
    let elements: Vec<InlineElement> = elements_opt.into_iter().filter_map(|e| e).collect();

    let plain_chars: Vec<char> = current_block.plain_text.chars().collect();
    let split_pos = (offset as usize).min(plain_chars.len());
    let text_before: String = plain_chars[..split_pos].iter().collect();
    let text_after: String = plain_chars[split_pos..].iter().collect();

    let now = chrono::Utc::now();

    // Split elements: find which go before and after the split point
    let mut after_elements: Vec<InlineElement> = Vec::new();
    let mut char_cursor: usize = 0;
    let mut split_found = false;

    for elem in &elements {
        let elem_char_len = match &elem.content {
            InlineContent::Text(s) => s.chars().count(),
            InlineContent::Image { .. } => 1,
            InlineContent::Empty => 0,
        };

        if !split_found {
            if char_cursor + elem_char_len <= split_pos {
                char_cursor += elem_char_len;
                continue;
            }
            split_found = true;
            let local_split = split_pos - char_cursor;

            match &elem.content {
                InlineContent::Text(s) => {
                    let chars: Vec<char> = s.chars().collect();
                    let before_text: String = chars[..local_split].iter().collect();
                    let after_text: String = chars[local_split..].iter().collect();

                    let mut updated = elem.clone();
                    updated.content = InlineContent::Text(before_text);
                    updated.updated_at = now;
                    uow.update_inline_element(&updated)?;

                    if !after_text.is_empty() {
                        let mut new_elem = elem.clone();
                        new_elem.id = 0;
                        new_elem.content = InlineContent::Text(after_text);
                        new_elem.created_at = now;
                        new_elem.updated_at = now;
                        after_elements.push(new_elem);
                    }
                }
                InlineContent::Image { .. } => {
                    if local_split == 0 {
                        let mut new_elem = elem.clone();
                        new_elem.id = 0;
                        new_elem.created_at = now;
                        new_elem.updated_at = now;
                        after_elements.push(new_elem);

                        let mut cleared = elem.clone();
                        cleared.content = InlineContent::Empty;
                        cleared.updated_at = now;
                        uow.update_inline_element(&cleared)?;
                    }
                }
                InlineContent::Empty => {}
            }
            char_cursor += elem_char_len;
        } else {
            let mut new_elem = elem.clone();
            new_elem.id = 0;
            new_elem.created_at = now;
            new_elem.updated_at = now;
            after_elements.push(new_elem);

            let mut cleared = elem.clone();
            cleared.content = InlineContent::Text(String::new());
            cleared.updated_at = now;
            uow.update_inline_element(&cleared)?;

            char_cursor += elem_char_len;
        }
    }

    if after_elements.is_empty() {
        after_elements.push(InlineElement {
            id: 0,
            created_at: now,
            updated_at: now,
            content: InlineContent::Text(text_after.clone()),
            ..Default::default()
        });
    }

    // Update current block with text before
    let mut updated_current = current_block.clone();
    updated_current.plain_text = text_before.clone();
    updated_current.text_length = text_before.chars().count() as i64;
    updated_current.updated_at = now;
    uow.update_block(&updated_current)?;

    // Create new blocks from fragment data
    let mut new_block_ids: Vec<EntityId> = Vec::new();
    let mut total_new_chars: i64 = 0;
    let mut running_position = current_block.document_position + updated_current.text_length + 1;

    for frag_block in &fragment_data.blocks {
        let block_text_len = frag_block.plain_text.chars().count() as i64;

        // Create list entity if fragment block has one
        let list_id = if let Some(ref frag_list) = frag_block.list {
            let list = frag_list.to_entity();
            let created_list = uow.create_list(&list, doc_id, -1)?;
            Some(created_list.id)
        } else {
            None
        };

        let new_block = Block {
            id: 0,
            created_at: now,
            updated_at: now,
            elements: vec![],
            list: list_id,
            text_length: block_text_len,
            document_position: running_position,
            plain_text: frag_block.plain_text.clone(),
            fmt_alignment: frag_block.alignment.clone(),
            fmt_top_margin: frag_block.top_margin,
            fmt_bottom_margin: frag_block.bottom_margin,
            fmt_left_margin: frag_block.left_margin,
            fmt_right_margin: frag_block.right_margin,
            fmt_heading_level: frag_block.heading_level,
            fmt_indent: frag_block.indent,
            fmt_text_indent: frag_block.text_indent,
            fmt_marker: frag_block.marker.clone(),
            fmt_tab_positions: frag_block.tab_positions.clone(),
        };

        let insert_index = (block_idx + 1 + new_block_ids.len()) as i32;
        let created_block = uow.create_block(&new_block, frame_id, insert_index)?;

        // Create inline elements from fragment
        for frag_elem in &frag_block.elements {
            let elem = frag_elem.to_entity();
            uow.create_inline_element(&elem, created_block.id, -1)?;
        }

        // If no elements, create an empty text element
        if frag_block.elements.is_empty() {
            let elem = InlineElement {
                id: 0,
                created_at: now,
                updated_at: now,
                content: InlineContent::Text(String::new()),
                ..Default::default()
            };
            uow.create_inline_element(&elem, created_block.id, -1)?;
        }

        new_block_ids.push(created_block.id);
        total_new_chars += block_text_len;
        running_position += block_text_len + 1;
    }

    // Create tail block with remaining content
    let tail_block = Block {
        id: 0,
        created_at: now,
        updated_at: now,
        elements: vec![],
        list: current_block.list,
        text_length: text_after.chars().count() as i64,
        document_position: running_position,
        plain_text: text_after,
        fmt_alignment: current_block.fmt_alignment.clone(),
        fmt_top_margin: current_block.fmt_top_margin,
        fmt_bottom_margin: current_block.fmt_bottom_margin,
        fmt_left_margin: current_block.fmt_left_margin,
        fmt_right_margin: current_block.fmt_right_margin,
        fmt_heading_level: current_block.fmt_heading_level,
        fmt_indent: current_block.fmt_indent,
        fmt_text_indent: current_block.fmt_text_indent,
        fmt_marker: current_block.fmt_marker.clone(),
        fmt_tab_positions: current_block.fmt_tab_positions.clone(),
    };

    let tail_insert_index = (block_idx + 1 + new_block_ids.len()) as i32;
    let created_tail = uow.create_block(&tail_block, frame_id, tail_insert_index)?;

    for after_elem in &after_elements {
        uow.create_inline_element(after_elem, created_tail.id, -1)?;
    }

    // Update frame child_order
    let mut updated_frame = frame.clone();
    let child_order_insert_pos = (block_idx + 1).min(updated_frame.child_order.len());
    let mut new_child_ids: Vec<i64> = new_block_ids.iter().map(|id| *id as i64).collect();
    new_child_ids.push(created_tail.id as i64);

    for (i, id) in new_child_ids.iter().enumerate() {
        updated_frame
            .child_order
            .insert(child_order_insert_pos + i, *id);
    }
    updated_frame.updated_at = now;
    updated_frame.blocks =
        uow.get_frame_relationship(&frame_id, &FrameRelationshipField::Blocks)?;
    uow.update_frame(&updated_frame)?;

    // Update subsequent block positions
    let blocks_added = fragment_data.blocks.len() as i64 + 1;
    let original_next_pos = current_block.document_position + current_block.text_length + 1;
    let new_next_pos = running_position + created_tail.text_length + 1;
    let pos_shift = new_next_pos - original_next_pos;

    let mut blocks_to_update: Vec<Block> = Vec::new();
    for b in &blocks[(block_idx + 1)..] {
        let mut ub = b.clone();
        ub.document_position += pos_shift;
        ub.updated_at = now;
        blocks_to_update.push(ub);
    }
    if !blocks_to_update.is_empty() {
        uow.update_block_multi(&blocks_to_update)?;
    }

    // Update document stats
    let mut updated_doc = document.clone();
    updated_doc.block_count += blocks_added;
    updated_doc.character_count += total_new_chars;
    updated_doc.updated_at = now;
    uow.update_document(&updated_doc)?;

    let new_position = running_position;
    let blocks_added_result = fragment_data.blocks.len() as i64;

    Ok((
        InsertFragmentResultDto {
            new_position,
            blocks_added: blocks_added_result,
        },
        snapshot,
    ))
}

pub struct InsertFragmentUseCase {
    uow_factory: Box<dyn InsertFragmentUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<InsertFragmentDto>,
}

impl InsertFragmentUseCase {
    pub fn new(uow_factory: Box<dyn InsertFragmentUnitOfWorkFactoryTrait>) -> Self {
        InsertFragmentUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(&mut self, dto: &InsertFragmentDto) -> Result<InsertFragmentResultDto> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let (result, snapshot) = execute_insert_fragment(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());

        uow.commit()?;
        Ok(result)
    }
}

impl UndoRedoCommand for InsertFragmentUseCase {
    fn undo(&mut self) -> Result<()> {
        let snapshot = self
            .undo_snapshot
            .as_ref()
            .ok_or_else(|| anyhow!("No snapshot available for undo"))?
            .clone();

        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;
        uow.restore_document(&snapshot)?;
        uow.commit()?;
        Ok(())
    }

    fn redo(&mut self) -> Result<()> {
        let dto = self
            .last_dto
            .as_ref()
            .ok_or_else(|| anyhow!("No DTO available for redo"))?
            .clone();

        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;
        let (_, snapshot) = execute_insert_fragment(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
