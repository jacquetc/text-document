use anyhow::{Result, anyhow};
use common::direct_access::frame::frame_repository::FrameRelationshipField;
use common::entities::{Block, Frame, InlineContent, InlineElement};
use common::types::EntityId;

/// Returns true for punctuation characters that should break undo merge groups.
pub fn is_word_boundary_punct(c: char) -> bool {
    matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '"' | '\'' | '-'
    )
}

/// Find the block containing the given document position from a sorted list of blocks.
///
/// Returns `(block, index_in_list, offset_within_block)`.
/// If `position` is beyond all blocks, falls back to the end of the last block.
pub fn find_block_at_position(blocks: &[Block], position: i64) -> Result<(Block, usize, i64)> {
    for (i, block) in blocks.iter().enumerate() {
        let block_start = block.document_position;
        let block_end = block_start + block.text_length;
        // The position is within this block (inclusive of block_end for appending at end)
        if position >= block_start && position <= block_end {
            let offset = position - block_start;
            return Ok((block.clone(), i, offset));
        }
    }
    // If position is beyond all blocks, use the last block
    if let Some(block) = blocks.last() {
        let offset = block.text_length;
        return Ok((block.clone(), blocks.len() - 1, offset));
    }
    Err(anyhow!("No blocks found in document"))
}

/// Find the inline element at a given offset within a block, and compute
/// the offset within that element.
///
/// Returns `(element, index_in_list, offset_within_element)`.
pub fn find_element_at_offset(
    elements: &[InlineElement],
    offset: i64,
) -> Result<(InlineElement, usize, i64)> {
    let mut running = 0i64;
    for (i, elem) in elements.iter().enumerate() {
        let elem_len = match &elem.content {
            InlineContent::Text(s) => s.chars().count() as i64,
            InlineContent::Image { .. } => 1,
            InlineContent::Empty => 0,
        };
        if offset <= running + elem_len {
            return Ok((elem.clone(), i, offset - running));
        }
        running += elem_len;
    }
    // Fall back to last element at its end
    if let Some(elem) = elements.last() {
        let elem_len = match &elem.content {
            InlineContent::Text(s) => s.chars().count() as i64,
            InlineContent::Image { .. } => 1,
            InlineContent::Empty => 0,
        };
        return Ok((elem.clone(), elements.len() - 1, elem_len));
    }
    Err(anyhow!("No inline elements found in block"))
}

/// Collect all block IDs in document order by traversing child_order recursively.
///
/// Negative entries in child_order are nested frame IDs (convention: -frame_id);
/// this function recurses into them to include their blocks in the linear sequence.
///
/// Table anchor frames (where `frame.table` is set) are expanded by calling
/// `get_table_cell_frames` to obtain cell frame IDs in row-major order, then
/// recursing into each cell frame.
///
/// Accepts closures so it can work with any UoW trait that provides frame access.
pub fn collect_block_ids_recursive<F, G, T>(
    get_frame: &F,
    get_relationship: &G,
    get_table_cell_frames: &T,
    frame_id: &EntityId,
) -> Result<Vec<EntityId>>
where
    F: Fn(&EntityId) -> Result<Option<Frame>>,
    G: Fn(&EntityId, &FrameRelationshipField) -> Result<Vec<EntityId>>,
    T: Fn(&EntityId) -> Result<Vec<EntityId>>,
{
    let frame = get_frame(frame_id)?.ok_or_else(|| anyhow!("Frame not found"))?;

    if !frame.child_order.is_empty() {
        let mut block_ids = Vec::new();
        for &entry in &frame.child_order {
            if entry > 0 {
                block_ids.push(entry as EntityId);
            } else if entry < 0 {
                let sub_frame_id = (-entry) as EntityId;
                let sub_frame = get_frame(&sub_frame_id)?
                    .ok_or_else(|| anyhow!("Sub-frame not found"))?;
                if let Some(table_id) = sub_frame.table {
                    // Table anchor frame: collect blocks from cell frames
                    let cell_frame_ids = get_table_cell_frames(&table_id)?;
                    for cf_id in cell_frame_ids {
                        let cf_blocks = collect_block_ids_recursive(
                            get_frame,
                            get_relationship,
                            get_table_cell_frames,
                            &cf_id,
                        )?;
                        block_ids.extend(cf_blocks);
                    }
                } else {
                    let sub_ids = collect_block_ids_recursive(
                        get_frame,
                        get_relationship,
                        get_table_cell_frames,
                        &sub_frame_id,
                    )?;
                    block_ids.extend(sub_ids);
                }
            }
        }
        Ok(block_ids)
    } else {
        get_relationship(frame_id, &FrameRelationshipField::Blocks)
    }
}
