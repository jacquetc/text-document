use anyhow::{Result, anyhow};
use common::entities::{Block, InlineContent, InlineElement};

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
