use super::editing_helpers::{find_block_at_position, find_element_at_offset};
use crate::InsertMarkdownAtPositionDto;
use crate::InsertMarkdownAtPositionResultDto;
use anyhow::{Result, anyhow};
use common::database::CommandUnitOfWork;
use common::direct_access::block::block_repository::BlockRelationshipField;
use common::direct_access::document::document_repository::DocumentRelationshipField;
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::direct_access::root::root_repository::RootRelationshipField;
use common::entities::{Block, Document, Frame, InlineContent, InlineElement, List, Root};
use common::parser_tools::content_parser;
use common::snapshot::EntityTreeSnapshot;
use common::types::{EntityId, ROOT_ENTITY_ID};
use common::undo_redo::UndoRedoCommand;
use std::any::Any;

pub trait InsertMarkdownAtPositionUnitOfWorkFactoryTrait: Send + Sync {
    fn create(&self) -> Box<dyn InsertMarkdownAtPositionUnitOfWorkTrait>;
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
pub trait InsertMarkdownAtPositionUnitOfWorkTrait: CommandUnitOfWork {}

/// Macro that generates the content insertion function for a specific UoW trait type.
macro_rules! impl_content_insert {
    ($fn_name:ident, $trait_type:ident) => {
        fn $fn_name(
            uow: &mut Box<dyn $trait_type>,
            position: i64,
            anchor: i64,
            parsed_blocks: &[content_parser::ParsedBlock],
        ) -> Result<(i64, i64, EntityTreeSnapshot)> {
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

            if position != anchor {
                return Err(anyhow!(
                    "Selection replacement is not supported. Use delete_text first."
                ));
            }

            let frame_ids =
                uow.get_document_relationship(&doc_id, &DocumentRelationshipField::Frames)?;
            let frame_id = *frame_ids
                .first()
                .ok_or_else(|| anyhow!("Document has no frames"))?;

            let frame = uow
                .get_frame(&frame_id)?
                .ok_or_else(|| anyhow!("Frame not found"))?;

            let block_ids =
                uow.get_frame_relationship(&frame_id, &FrameRelationshipField::Blocks)?;

            let blocks_opt = uow.get_block_multi(&block_ids)?;
            let mut blocks: Vec<Block> = blocks_opt.into_iter().flatten().collect();
            blocks.sort_by_key(|b| b.document_position);

            let (current_block, block_idx, offset) = find_block_at_position(&blocks, position)?;

            let element_ids =
                uow.get_block_relationship(&current_block.id, &BlockRelationshipField::Elements)?;
            let elements_opt = uow.get_inline_element_multi(&element_ids)?;
            let elements: Vec<InlineElement> = elements_opt.into_iter().flatten().collect();

            let plain_chars: Vec<char> = current_block.plain_text.chars().collect();
            let split_pos = (offset as usize).min(plain_chars.len());

            // ── Inline merge: single block with no block-level formatting ──
            if parsed_blocks.len() == 1 && parsed_blocks[0].is_inline_only() {
                let parsed = &parsed_blocks[0];
                let inserted_plain: String = parsed.spans.iter().map(|s| s.text.as_str()).collect();
                let inserted_len = inserted_plain.chars().count() as i64;

                if inserted_len == 0 {
                    return Ok((position, 0, snapshot));
                }

                let now = chrono::Utc::now();

                // Find element at the cursor offset
                let (target_elem, elem_idx, local_offset) =
                    find_element_at_offset(&elements, offset)?;

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

                // Create new inline elements from parsed spans
                let mut insert_idx = (elem_idx + 1) as i32;
                for span in &parsed.spans {
                    if span.text.is_empty() {
                        continue;
                    }
                    let elem = InlineElement {
                        id: 0,
                        created_at: now,
                        updated_at: now,
                        content: InlineContent::Text(span.text.clone()),
                        fmt_font_family: if span.code {
                            Some("monospace".to_string())
                        } else {
                            None
                        },
                        fmt_font_bold: if span.bold { Some(true) } else { None },
                        fmt_font_italic: if span.italic { Some(true) } else { None },
                        fmt_font_underline: if span.underline { Some(true) } else { None },
                        fmt_font_strikeout: if span.strikeout { Some(true) } else { None },
                        fmt_anchor_href: span.link_href.clone(),
                        fmt_is_anchor: if span.link_href.is_some() {
                            Some(true)
                        } else {
                            None
                        },
                        ..Default::default()
                    };
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
                    + &inserted_plain
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

                return Ok((position + inserted_len, 0, snapshot));
            }

            // ── Block-splitting path (multi-block or block-level content) ──
            let text_before: String = plain_chars[..split_pos].iter().collect();
            let text_after: String = plain_chars[split_pos..].iter().collect();

            let now = chrono::Utc::now();

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

            // Helper: create inline elements from parsed spans on a block
            macro_rules! create_span_elements {
                ($uow:expr, $spans:expr, $block_id:expr, $now:expr) => {
                    for span in $spans {
                        if span.text.is_empty() {
                            continue;
                        }
                        let elem = InlineElement {
                            id: 0,
                            created_at: $now,
                            updated_at: $now,
                            content: InlineContent::Text(span.text.clone()),
                            fmt_font_family: if span.code {
                                Some("monospace".to_string())
                            } else {
                                None
                            },
                            fmt_font_bold: if span.bold { Some(true) } else { None },
                            fmt_font_italic: if span.italic { Some(true) } else { None },
                            fmt_font_underline: if span.underline { Some(true) } else { None },
                            fmt_font_strikeout: if span.strikeout { Some(true) } else { None },
                            fmt_anchor_href: span.link_href.clone(),
                            fmt_is_anchor: if span.link_href.is_some() {
                                Some(true)
                            } else {
                                None
                            },
                            ..Default::default()
                        };
                        $uow.create_inline_element(&elem, $block_id, -1)?;
                    }
                };
            }

            if parsed_blocks.len() >= 2 {
                // ── Multi-block: merge inline-only first/last, standalone otherwise ──
                let first_parsed = &parsed_blocks[0];
                let last_parsed = &parsed_blocks[parsed_blocks.len() - 1];
                let merge_first = first_parsed.is_inline_only();
                let merge_last = last_parsed.is_inline_only();

                let first_plain: String =
                    first_parsed.spans.iter().map(|s| s.text.as_str()).collect();
                let first_len = first_plain.chars().count() as i64;

                let mut updated_current = current_block.clone();
                if merge_first {
                    updated_current.plain_text = text_before.clone() + &first_plain;
                    updated_current.text_length = text_before.chars().count() as i64 + first_len;
                } else {
                    updated_current.plain_text = text_before.clone();
                    updated_current.text_length = text_before.chars().count() as i64;
                }
                updated_current.updated_at = now;
                uow.update_block(&updated_current)?;

                if merge_first {
                    create_span_elements!(uow, &first_parsed.spans, current_block.id, now);
                }

                let mut new_block_ids: Vec<EntityId> = Vec::new();
                let mut total_new_chars: i64 = if merge_first { first_len } else { 0 };
                let mut running_position =
                    current_block.document_position + updated_current.text_length + 1;

                let middle_start = if merge_first { 1 } else { 0 };
                let middle_end = if merge_last {
                    parsed_blocks.len() - 1
                } else {
                    parsed_blocks.len()
                };

                for parsed in &parsed_blocks[middle_start..middle_end] {
                    let block_plain: String =
                        parsed.spans.iter().map(|s| s.text.as_str()).collect();
                    let block_text_len = block_plain.chars().count() as i64;

                    let list_id = if let Some(ref list_style) = parsed.list_style {
                        let list = List {
                            id: 0,
                            created_at: now,
                            updated_at: now,
                            style: list_style.clone(),
                            indent: 1,
                            prefix: String::new(),
                            suffix: String::new(),
                        };
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
                        plain_text: block_plain,
                        fmt_alignment: None,
                        fmt_top_margin: None,
                        fmt_bottom_margin: None,
                        fmt_left_margin: None,
                        fmt_right_margin: None,
                        fmt_heading_level: parsed.heading_level,
                        fmt_indent: None,
                        fmt_text_indent: None,
                        fmt_marker: None,
                        fmt_tab_positions: vec![],
                        fmt_line_height: None,
                        fmt_non_breakable_lines: None,
                        fmt_direction: None,
                        fmt_background_color: None,
                        fmt_is_code_block: None,
                        fmt_code_language: None,
                    };

                    let insert_index = (block_idx + 1 + new_block_ids.len()) as i32;
                    let created_block = uow.create_block(&new_block, frame_id, insert_index)?;

                    create_span_elements!(uow, &parsed.spans, created_block.id, now);

                    if parsed.spans.is_empty() || parsed.spans.iter().all(|s| s.text.is_empty()) {
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

                let last_plain: String =
                    last_parsed.spans.iter().map(|s| s.text.as_str()).collect();
                let last_len = last_plain.chars().count() as i64;

                let tail_plain = if merge_last {
                    total_new_chars += last_len;
                    last_plain + &text_after
                } else {
                    text_after.clone()
                };
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
                    fmt_is_code_block: current_block.fmt_is_code_block,
                    fmt_code_language: current_block.fmt_code_language.clone(),
                };

                let tail_insert_index = (block_idx + 1 + new_block_ids.len()) as i32;
                let created_tail = uow.create_block(&tail_block, frame_id, tail_insert_index)?;

                if merge_last {
                    create_span_elements!(uow, &last_parsed.spans, created_tail.id, now);
                }
                for after_elem in &after_elements {
                    uow.create_inline_element(after_elem, created_tail.id, -1)?;
                }

                let mut updated_frame = frame.clone();
                let child_order_insert_pos = (block_idx + 1).min(updated_frame.child_order.len());
                let mut new_child_ids: Vec<i64> =
                    new_block_ids.iter().map(|id| *id as i64).collect();
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

                let standalone_count = (middle_end - middle_start) as i64;
                let blocks_added = standalone_count + 1;
                let original_next_pos =
                    current_block.document_position + current_block.text_length + 1;
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

                let new_position = if merge_last {
                    created_tail.document_position + last_len
                } else {
                    created_tail.document_position
                };
                Ok((new_position, blocks_added, snapshot))
            } else {
                // ── Single block with block-level formatting ──
                let parsed = &parsed_blocks[0];
                let block_plain: String = parsed.spans.iter().map(|s| s.text.as_str()).collect();
                let block_text_len = block_plain.chars().count() as i64;

                let mut updated_current = current_block.clone();
                updated_current.plain_text = text_before.clone();
                updated_current.text_length = text_before.chars().count() as i64;
                updated_current.updated_at = now;
                uow.update_block(&updated_current)?;

                let mut running_position =
                    current_block.document_position + updated_current.text_length + 1;

                let list_id = if let Some(ref list_style) = parsed.list_style {
                    let list = List {
                        id: 0,
                        created_at: now,
                        updated_at: now,
                        style: list_style.clone(),
                        indent: 1,
                        prefix: String::new(),
                        suffix: String::new(),
                    };
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
                    plain_text: block_plain,
                    fmt_alignment: None,
                    fmt_top_margin: None,
                    fmt_bottom_margin: None,
                    fmt_left_margin: None,
                    fmt_right_margin: None,
                    fmt_heading_level: parsed.heading_level,
                    fmt_indent: None,
                    fmt_text_indent: None,
                    fmt_marker: None,
                    fmt_tab_positions: vec![],
                    fmt_line_height: None,
                    fmt_non_breakable_lines: None,
                    fmt_direction: None,
                    fmt_background_color: None,
                    fmt_is_code_block: None,
                    fmt_code_language: None,
                };

                let created_block =
                    uow.create_block(&new_block, frame_id, (block_idx + 1) as i32)?;
                create_span_elements!(uow, &parsed.spans, created_block.id, now);

                if parsed.spans.is_empty() || parsed.spans.iter().all(|s| s.text.is_empty()) {
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
                    fmt_is_code_block: current_block.fmt_is_code_block,
                    fmt_code_language: current_block.fmt_code_language.clone(),
                };

                let created_tail =
                    uow.create_block(&tail_block, frame_id, (block_idx + 2) as i32)?;
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
                let original_next_pos =
                    current_block.document_position + current_block.text_length + 1;
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

                Ok((running_position, 1, snapshot))
            }
        }
    };
}

impl_content_insert!(
    execute_content_insert,
    InsertMarkdownAtPositionUnitOfWorkTrait
);

fn execute_insert_markdown(
    uow: &mut Box<dyn InsertMarkdownAtPositionUnitOfWorkTrait>,
    dto: &InsertMarkdownAtPositionDto,
) -> Result<(InsertMarkdownAtPositionResultDto, EntityTreeSnapshot)> {
    let parsed_blocks = content_parser::parse_markdown(&dto.markdown);
    let (new_position, blocks_added, snapshot) =
        execute_content_insert(uow, dto.position, dto.anchor, &parsed_blocks)?;
    Ok((
        InsertMarkdownAtPositionResultDto {
            new_position,
            blocks_added,
        },
        snapshot,
    ))
}

pub struct InsertMarkdownAtPositionUseCase {
    uow_factory: Box<dyn InsertMarkdownAtPositionUnitOfWorkFactoryTrait>,
    undo_snapshot: Option<EntityTreeSnapshot>,
    last_dto: Option<InsertMarkdownAtPositionDto>,
}

impl InsertMarkdownAtPositionUseCase {
    pub fn new(uow_factory: Box<dyn InsertMarkdownAtPositionUnitOfWorkFactoryTrait>) -> Self {
        InsertMarkdownAtPositionUseCase {
            uow_factory,
            undo_snapshot: None,
            last_dto: None,
        }
    }

    pub fn execute(
        &mut self,
        dto: &InsertMarkdownAtPositionDto,
    ) -> Result<InsertMarkdownAtPositionResultDto> {
        let mut uow = self.uow_factory.create();
        uow.begin_transaction()?;

        let (result, snapshot) = execute_insert_markdown(&mut uow, dto)?;
        self.undo_snapshot = Some(snapshot);
        self.last_dto = Some(dto.clone());

        uow.commit()?;
        Ok(result)
    }
}

impl UndoRedoCommand for InsertMarkdownAtPositionUseCase {
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
        let (_, snapshot) = execute_insert_markdown(&mut uow, &dto)?;
        self.undo_snapshot = Some(snapshot);
        uow.commit()?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
