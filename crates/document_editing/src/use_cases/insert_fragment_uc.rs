use super::editing_helpers::{find_block_at_position, find_element_at_offset};
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
use common::types::{EntityId, ROOT_ENTITY_ID};
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

fn execute_insert_fragment(
    uow: &mut Box<dyn InsertFragmentUnitOfWorkTrait>,
    dto: &InsertFragmentDto,
) -> Result<(InsertFragmentResultDto, EntityTreeSnapshot)> {
    const MAX_FRAGMENT_SIZE: usize = 64 * 1024 * 1024; // 64 MB
    if dto.fragment_data.len() > MAX_FRAGMENT_SIZE {
        return Err(anyhow!(
            "Fragment data exceeds maximum size ({} bytes, limit {})",
            dto.fragment_data.len(),
            MAX_FRAGMENT_SIZE
        ));
    }

    let fragment_data: FragmentData = serde_json::from_str(&dto.fragment_data)
        .map_err(|e| anyhow!("Invalid fragment_data JSON: {}", e))?;

    if fragment_data.blocks.is_empty() {
        return Err(anyhow!("Fragment contains no blocks"));
    }

    let root = uow
        .get_root(&ROOT_ENTITY_ID)?
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
    let mut blocks: Vec<Block> = blocks_opt.into_iter().flatten().collect();
    blocks.sort_by_key(|b| b.document_position);

    let (current_block, block_idx, offset) = find_block_at_position(&blocks, dto.position)?;

    // Get current block's elements for splitting
    let element_ids =
        uow.get_block_relationship(&current_block.id, &BlockRelationshipField::Elements)?;
    let elements_opt = uow.get_inline_element_multi(&element_ids)?;
    let elements: Vec<InlineElement> = elements_opt.into_iter().flatten().collect();

    let plain_chars: Vec<char> = current_block.plain_text.chars().collect();
    let split_pos = (offset as usize).min(plain_chars.len());

    // ── Inline merge: single block with no block-level formatting ──
    if fragment_data.blocks.len() == 1 && fragment_data.blocks[0].is_inline_only() {
        let frag_block = &fragment_data.blocks[0];
        let inserted_plain = &frag_block.plain_text;
        let inserted_len = inserted_plain.chars().count() as i64;

        if inserted_len == 0 {
            return Ok((
                InsertFragmentResultDto {
                    new_position: dto.position,
                    blocks_added: 0,
                },
                snapshot,
            ));
        }

        let now = chrono::Utc::now();

        // Find element at the cursor offset
        let (target_elem, elem_idx, local_offset) = find_element_at_offset(&elements, offset)?;

        // Split the target element: keep "before" text in place
        let after_text = match &target_elem.content {
            InlineContent::Text(s) => {
                let chars: Vec<char> = s.chars().collect();
                let lo = local_offset as usize;
                let before: String = chars[..lo].iter().collect();
                let after: String = chars[lo..].iter().collect();

                let mut updated = target_elem.clone();
                updated.content = InlineContent::Text(before);
                updated.updated_at = now;
                uow.update_inline_element(&updated)?;

                after
            }
            _ => String::new(),
        };

        // Create new inline elements from fragment
        let mut insert_idx = (elem_idx + 1) as i32;
        for frag_elem in &frag_block.elements {
            let elem = frag_elem.to_entity();
            uow.create_inline_element(&elem, current_block.id, insert_idx)?;
            insert_idx += 1;
        }

        // Create element for remaining text (preserving original formatting)
        if !after_text.is_empty() {
            let mut after_elem = target_elem.clone();
            after_elem.id = 0;
            after_elem.content = InlineContent::Text(after_text);
            after_elem.created_at = now;
            after_elem.updated_at = now;
            uow.create_inline_element(&after_elem, current_block.id, insert_idx)?;
        }

        // Update block metadata
        let new_plain: String = plain_chars[..split_pos].iter().collect::<String>()
            + inserted_plain
            + &plain_chars[split_pos..].iter().collect::<String>();
        let mut updated_block = current_block.clone();
        updated_block.plain_text = new_plain;
        updated_block.text_length += inserted_len;
        updated_block.updated_at = now;
        uow.update_block(&updated_block)?;

        // Shift subsequent blocks
        let mut blocks_to_update: Vec<Block> = Vec::new();
        for b in &blocks[(block_idx + 1)..] {
            let mut ub = b.clone();
            ub.document_position += inserted_len;
            ub.updated_at = now;
            blocks_to_update.push(ub);
        }
        if !blocks_to_update.is_empty() {
            uow.update_block_multi(&blocks_to_update)?;
        }

        // Update document character count
        let mut updated_doc = document.clone();
        updated_doc.character_count += inserted_len;
        updated_doc.updated_at = now;
        uow.update_document(&updated_doc)?;

        return Ok((
            InsertFragmentResultDto {
                new_position: dto.position + inserted_len,
                blocks_added: 0,
            },
            snapshot,
        ));
    }

    // ── Block-splitting path (multi-block or block-level content) ──
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

    // Helper: create fragment elements on a block
    fn create_frag_elements(
        uow: &mut Box<dyn InsertFragmentUnitOfWorkTrait>,
        elements: &[common::parser_tools::fragment_schema::FragmentElement],
        block_id: EntityId,
    ) -> Result<()> {
        for frag_elem in elements {
            let elem = frag_elem.to_entity();
            uow.create_inline_element(&elem, block_id, -1)?;
        }
        Ok(())
    }

    if fragment_data.blocks.len() >= 2 {
        // ── Multi-block merge: first→current, middle→new, last→tail ──

        let first_frag = &fragment_data.blocks[0];
        let first_len = first_frag.plain_text.chars().count() as i64;

        let mut updated_current = current_block.clone();
        updated_current.plain_text = text_before.clone() + &first_frag.plain_text;
        updated_current.text_length = text_before.chars().count() as i64 + first_len;
        updated_current.updated_at = now;
        uow.update_block(&updated_current)?;

        create_frag_elements(uow, &first_frag.elements, current_block.id)?;

        let mut new_block_ids: Vec<EntityId> = Vec::new();
        let mut total_new_chars: i64 = first_len;
        let mut running_position =
            current_block.document_position + updated_current.text_length + 1;

        // Create middle blocks (indices 1..n-1)
        for frag_block in &fragment_data.blocks[1..fragment_data.blocks.len() - 1] {
            let block_text_len = frag_block.plain_text.chars().count() as i64;

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
                fmt_line_height: frag_block.line_height,
                fmt_non_breakable_lines: frag_block.non_breakable_lines,
                fmt_direction: frag_block.direction.clone(),
                fmt_background_color: frag_block.background_color.clone(),
            };

            let insert_index = (block_idx + 1 + new_block_ids.len()) as i32;
            let created_block = uow.create_block(&new_block, frame_id, insert_index)?;

            create_frag_elements(uow, &frag_block.elements, created_block.id)?;

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

        // Merge last fragment block's inline content into the tail block
        let last_frag = &fragment_data.blocks[fragment_data.blocks.len() - 1];
        let last_len = last_frag.plain_text.chars().count() as i64;

        let tail_plain = last_frag.plain_text.clone() + &text_after;
        let tail_block = Block {
            id: 0,
            created_at: now,
            updated_at: now,
            elements: vec![],
            list: current_block.list,
            text_length: tail_plain.chars().count() as i64,
            document_position: running_position,
            plain_text: tail_plain,
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
            fmt_line_height: current_block.fmt_line_height,
            fmt_non_breakable_lines: current_block.fmt_non_breakable_lines,
            fmt_direction: current_block.fmt_direction.clone(),
            fmt_background_color: current_block.fmt_background_color.clone(),
        };

        let tail_insert_index = (block_idx + 1 + new_block_ids.len()) as i32;
        let created_tail = uow.create_block(&tail_block, frame_id, tail_insert_index)?;

        // Last block's elements first, then the original after-elements
        create_frag_elements(uow, &last_frag.elements, created_tail.id)?;
        for after_elem in &after_elements {
            uow.create_inline_element(after_elem, created_tail.id, -1)?;
        }

        total_new_chars += last_len;

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

        let middle_count = fragment_data.blocks.len() as i64 - 2;
        let blocks_added = middle_count + 1;
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

        let mut updated_doc = document.clone();
        updated_doc.block_count += blocks_added;
        updated_doc.character_count += total_new_chars;
        updated_doc.updated_at = now;
        uow.update_document(&updated_doc)?;

        let new_position = created_tail.document_position + last_len;

        Ok((
            InsertFragmentResultDto {
                new_position,
                blocks_added,
            },
            snapshot,
        ))
    } else {
        // ── Single block with block-level formatting ──
        let frag_block = &fragment_data.blocks[0];
        let block_text_len = frag_block.plain_text.chars().count() as i64;

        let mut updated_current = current_block.clone();
        updated_current.plain_text = text_before.clone();
        updated_current.text_length = text_before.chars().count() as i64;
        updated_current.updated_at = now;
        uow.update_block(&updated_current)?;

        let mut running_position =
            current_block.document_position + updated_current.text_length + 1;

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
            fmt_line_height: frag_block.line_height,
            fmt_non_breakable_lines: frag_block.non_breakable_lines,
            fmt_direction: frag_block.direction.clone(),
            fmt_background_color: frag_block.background_color.clone(),
        };

        let created_block = uow.create_block(&new_block, frame_id, (block_idx + 1) as i32)?;
        create_frag_elements(uow, &frag_block.elements, created_block.id)?;

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

        running_position += block_text_len + 1;

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
            fmt_line_height: current_block.fmt_line_height,
            fmt_non_breakable_lines: current_block.fmt_non_breakable_lines,
            fmt_direction: current_block.fmt_direction.clone(),
            fmt_background_color: current_block.fmt_background_color.clone(),
        };

        let created_tail = uow.create_block(&tail_block, frame_id, (block_idx + 2) as i32)?;
        for after_elem in &after_elements {
            uow.create_inline_element(after_elem, created_tail.id, -1)?;
        }

        let mut updated_frame = frame.clone();
        let child_order_insert_pos = (block_idx + 1).min(updated_frame.child_order.len());
        let new_child_ids = [created_block.id as i64, created_tail.id as i64];
        for (i, id) in new_child_ids.iter().enumerate() {
            updated_frame
                .child_order
                .insert(child_order_insert_pos + i, *id);
        }
        updated_frame.updated_at = now;
        updated_frame.blocks =
            uow.get_frame_relationship(&frame_id, &FrameRelationshipField::Blocks)?;
        uow.update_frame(&updated_frame)?;

        let blocks_added: i64 = 2;
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

        let mut updated_doc = document.clone();
        updated_doc.block_count += blocks_added;
        updated_doc.character_count += block_text_len;
        updated_doc.updated_at = now;
        uow.update_document(&updated_doc)?;

        Ok((
            InsertFragmentResultDto {
                new_position: running_position,
                blocks_added: 1,
            },
            snapshot,
        ))
    }
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
