use std::rc::{Rc, Weak};

use crate::block::Block;
use crate::format::{CharFormat, FormattedElement, FrameFormat};
use crate::frame::Frame;
use crate::text_document::Element::BlockElement;
use crate::text_document::{ElementManager, InsertMode, ModelError};
use crate::{ChangeReason, Element};

#[derive(Clone)]
pub struct TextCursor {
    element_manager: Rc<ElementManager>,
    position: usize,
    anchor_position: usize,
}

impl TextCursor {
    pub(crate) fn new(element_manager: Rc<ElementManager>) -> Self {
        Self {
            element_manager,
            position: Default::default(),
            anchor_position: Default::default(),
        }
    }

    pub fn position(&self) -> usize {
        let mut position = self.position;

        let end_of_document = self.element_manager.root_frame().end();
        if position > end_of_document {
            position = end_of_document;
        }

        position
    }

    pub fn anchor_position(&self) -> usize {
        let mut anchor_position = self.anchor_position;

        let end_of_document = self.element_manager.root_frame().end();
        if anchor_position > end_of_document {
            anchor_position = end_of_document;
        }

        anchor_position
    }

    /// set the cursor position, with or without the anchor depending of move_mode. Ensure that the cursor position is in the document.
    pub fn set_position(&mut self, position: usize, move_mode: MoveMode) {
        let mut position = position;

        let end_of_document = self.element_manager.root_frame().end();
        if position > end_of_document {
            position = end_of_document;
        }

        match move_mode {
            MoveMode::MoveAnchor => {
                self.position = position;
                self.anchor_position = position;
            }
            MoveMode::KeepAnchor => self.position = position,
        }
    }

    /// Give the current block under the cursor position
    pub fn current_block(&self) -> Weak<Block> {
        Rc::downgrade(&self.current_block_rc())
    }

    fn current_block_rc(&self) -> Rc<Block> {
        self.element_manager
            .find_block(self.position)
            .unwrap_or_else(|| self.element_manager.last_block().unwrap())
    }

    // split block at position, like if a new line is inserted
    pub fn insert_block(&mut self) -> Result<Weak<Block>, ModelError> {
        // fix positions
        let left_position = self.position.min(self.anchor_position);
        let right_position = self.anchor_position.max(self.position);

        let mut new_position = left_position;
        let mut removed_characters_count = 0;
        if left_position != right_position {
            // for now, new_position is wrong, to be implemented
            (new_position, removed_characters_count) = self
                .remove_with_signal(left_position, right_position, false)
                .unwrap();
        }

        // find reference block
        let old_block_rc = self
            .element_manager
            .find_block(new_position)
            .ok_or_else(|| {
                ModelError::ElementNotFound(format!("block not found at {}", new_position))
            })?;

        let _u = old_block_rc.uuid();

        let new_block =
            old_block_rc.split(old_block_rc.convert_position_from_document(new_position))?;
        let _w = new_block.uuid();
        let _order = self
            .element_manager
            .get_element_order(self.element_manager.get(new_block.uuid()).unwrap())
            .unwrap();

        // if new block empty, create empty child text element

        if new_block.list_all_children().is_empty() {
            self.element_manager
                .insert_new_text(new_block.uuid(), InsertMode::AsChild)?;
        }

        let beginning_of_new_block = new_position + 1;

        // reset cursor position and selection
        self.set_position(beginning_of_new_block, MoveMode::MoveAnchor);

        // signaling changes
        self.element_manager
            .signal_for_text_change(new_position, removed_characters_count, 1);

        self.element_manager.signal_for_element_change(
            self.element_manager
                .get_parent_element(&Element::BlockElement(old_block_rc))
                .unwrap(),
            ChangeReason::ChildrenChanged,
        );

        Ok(Rc::downgrade(&new_block))
    }

    /// Give the current frame under the cursor position
    pub fn current_frame(&self) -> Weak<Frame> {
        Rc::downgrade(&self.current_frame_rc())
    }

    fn current_frame_rc(&self) -> Rc<Frame> {
        self.element_manager
            .find_frame(self.position)
            .unwrap_or_else(|| self.element_manager.root_frame())
    }

    pub fn set_frame_format(&mut self, frame_format: FrameFormat) -> Result<(), ModelError> {
        let current_frame = self
            .current_frame()
            .upgrade()
            .ok_or_else(|| ModelError::ElementNotFound("()".to_string()))?;

        current_frame.set_format(&frame_format)
    }

    /// insert a frame at the cursor position
    pub fn insert_frame(&mut self) -> Result<Weak<Frame>, ModelError> {
        // fix positions
        let left_position = self.position.min(self.anchor_position);
        let right_position = self.anchor_position.max(self.position);

        let mut new_position = left_position;
        let mut removed_characters_count = 0;
        if left_position != right_position {
            // for now, new_position is wrong, to be implemented
            (new_position, removed_characters_count) = self
                .remove_with_signal(left_position, right_position, false)
                .unwrap();
        }

        // find reference block
        let old_block_rc = self
            .element_manager
            .find_block(new_position)
            .unwrap_or_else(|| self.element_manager.last_block().unwrap());

        let new_block =
            old_block_rc.split(old_block_rc.convert_position_from_document(new_position))?;

        // if new block empty, create text

        if new_block.list_all_children().is_empty() {
            self.element_manager
                .insert_new_text(new_block.uuid(), InsertMode::AsChild)?;
        }

        // insert frame with block and text element
        let frame = self
            .element_manager
            .insert_new_frame(old_block_rc.uuid(), InsertMode::After)?;
        let block = self
            .element_manager
            .insert_new_block(frame.uuid(), InsertMode::AsChild)?;
        let _text = self
            .element_manager
            .insert_new_text(block.uuid(), InsertMode::AsChild)?;

        // reset cursor position and selection
        self.set_position(block.position(), MoveMode::MoveAnchor);

        // signaling changes
        self.element_manager
            .signal_for_text_change(new_position, removed_characters_count, 1);

        self.element_manager.signal_for_element_change(
            self.element_manager
                .get_parent_element(&Element::FrameElement(frame.clone()))
                .unwrap(),
            ChangeReason::ChildrenChanged,
        );

        Ok(Rc::downgrade(&frame))
    }

    /// Insert plain text and return (start position, end position)
    pub fn insert_plain_text<S: Into<String>>(
        &mut self,
        plain_text: S,
    ) -> Result<(usize, usize), ModelError> {
        let plain_text: String = plain_text.into();

        // get char format
        let char_format: CharFormat = match self.char_format() {
            Some(char_format) => char_format,
            None => self.current_block_rc().char_format(),
        };

        // fix positions
        let left_position = self.position.min(self.anchor_position);
        let right_position = self.anchor_position.max(self.position);

        let mut new_position = left_position;
        let start_position = left_position;
        let mut removed_characters_count = 0;

        if left_position != right_position {
            // for now, new_position is wrong, to be implemented
            (new_position, removed_characters_count) = self
                .remove_with_signal(left_position, right_position, false)
                .unwrap();
        }

        let mut first_loop = true;

        let mut block = self
            .element_manager
            .find_block(new_position)
            .unwrap_or_else(|| self.element_manager.last_block().unwrap());

        let mut other_block_from_split = None;

        let lines = plain_text.split('\n');
        let mut index = 0;

        let count = lines.clone().count();

        for text_line in lines {
            // insert on existing targeted block
            if first_loop {
                let position_in_block = block.convert_position_from_document(new_position);

                // split targeted block
                if count > 1 {
                    other_block_from_split = block.split(position_in_block).ok();
                    new_position += 1;
                }

                block.insert_plain_text(text_line, position_in_block);

                first_loop = false;
            }
            // insertion of last line at the beginning of the second half of the split block
            else if count - 1 == index {
                match &other_block_from_split {
                    Some(block) => {
                        block.insert_plain_text(text_line, 0);
                    }
                    None => continue,
                }
            } else {
                // new blocks for the rest of the text_line
                block = self
                    .element_manager
                    .insert_new_block(block.uuid(), InsertMode::After)
                    .unwrap();
                block.set_plain_text(text_line);
                new_position += 1;
            }

            index += 1;
            new_position += text_line.len();
        }

        // reset cursor position and selection
        self.set_position(block.position(), MoveMode::MoveAnchor);

        // signaling changes
        self.element_manager.signal_for_text_change(
            start_position,
            removed_characters_count,
            plain_text.len(),
        );

        // if only one line, so one Block element changed
        if count == 1 {
            self.element_manager.signal_for_element_change(
                Element::BlockElement(block),
                ChangeReason::ChildrenChanged,
            );
        } else {
            self.element_manager.signal_for_element_change(
                self.element_manager
                    .get_parent_element(&Element::BlockElement(block))
                    .unwrap(),
                ChangeReason::ChildrenChanged,
            );
        }

        // set new cursor position
        self.set_position(new_position, MoveMode::MoveAnchor);

        Ok((start_position, new_position))
    }

    // select plain text between cursor position and the anchor position
    pub fn selected_text(&self) -> String {
        // fix positions
        let left_position = self.position.min(self.anchor_position);
        let right_position = self.anchor_position.max(self.position);
        if left_position == right_position {
            return String::new();
        }

        let top_block = match self.element_manager.find_block(left_position) {
            Some(block) => block,
            None => return String::new(),
        };
        let bottom_block = match self.element_manager.find_block(right_position) {
            Some(block) => block,
            None => return String::new(),
        };

        let left_position_in_block = top_block.convert_position_from_document(left_position);
        let right_position_in_block = bottom_block.convert_position_from_document(right_position);

        // same block:
        if top_block == bottom_block {
            top_block.plain_text_between_positions(left_position_in_block, right_position_in_block)
        } else {
            // first block
            let mut string_list = vec![top_block
                .plain_text_between_positions(left_position_in_block, top_block.text_length())];

            self.element_manager
                .list_all_children(0)
                .iter()
                .skip_while(|element| element.uuid() != top_block.uuid())
                .skip(1)
                .take_while(|element| element.uuid() != bottom_block.uuid())
                .filter_map(|element| match element {
                    BlockElement(block) => Some(block.plain_text()),
                    _ => None,
                })
                .for_each(|string| string_list.push(string));

            // last block
            string_list.push(bottom_block.plain_text_between_positions(0, right_position_in_block));

            let final_string = string_list.join("\n");

            // take into account \n
            let length_of_selection = right_position - left_position;

            final_string[0..length_of_selection].to_string()
        }
    }

    // fetch the char format at the cursor position
    pub fn char_format(&self) -> Option<CharFormat> {
        let block_rc = self.current_block_rc();

        block_rc.char_format_at(block_rc.convert_position_from_document(self.position))
    }

    /// Remove elements between two positions. Split blocks if needed. Frames in superior level (i.e. children)
    ///  are completely removed even if only a part of it is selected
    ///
    /// Return new position and number of removed chars
    pub fn remove(&mut self) -> Result<(usize, usize), ModelError> {
        self.remove_with_signal(self.position, self.anchor_position, true)
    }

    /// same as 'remove()' but with signal argument
    fn remove_with_signal(
        &mut self,
        position: usize,
        anchor_position: usize,
        send_change_signals: bool,
    ) -> Result<(usize, usize), ModelError> {
        let new_position;
        let mut removed_characters_count;

        let left_position = position.min(anchor_position);
        let right_position = anchor_position.max(position);

        let top_block = self
            .element_manager
            .find_block(left_position)
            .ok_or_else(|| ModelError::ElementNotFound("tob block not found".to_string()))?;
        let bottom_block = self
            .element_manager
            .find_block(right_position)
            .ok_or_else(|| ModelError::ElementNotFound("bottom block not found".to_string()))?;

        let left_position_in_block = top_block.convert_position_from_document(left_position);
        let right_position_in_block = bottom_block.convert_position_from_document(right_position);

        // if selection is in the same block:
        if top_block == bottom_block {
            (new_position, removed_characters_count) = top_block
                .remove_between_positions(left_position_in_block, right_position_in_block)?;

            // reset cursor position and selection
            self.set_position(new_position, MoveMode::MoveAnchor);

            // signaling changes
            self.element_manager
                .signal_for_text_change(new_position, removed_characters_count, 0);

            if send_change_signals {
                self.element_manager.signal_for_element_change(
                    Element::BlockElement(top_block),
                    ChangeReason::ChildrenChanged,
                );
            }

            return Ok((new_position, removed_characters_count));
        }

        let top_block_level = self.element_manager.get_level(top_block.uuid());
        let bottom_block_level = self.element_manager.get_level(bottom_block.uuid());

        let mut parent_element_for_signal: Element;

        // determine if any element between top and bottom block is inferior than both, in this case the common ancestor is deleted whole

        // Frame  --> common ancestor, so it will be removed
        // |- Frame
        //    |- Block  --> top block, selection start
        //       |- Text
        // |- Frame
        //    |- Block  --> bottom block, selection end
        //       |- Text

        let min_level = top_block_level.min(bottom_block_level);
        let has_common_ancestor_element = self
            .element_manager
            .list_all_children(0)
            .iter()
            // keep all between top and bottom blocks
            .skip_while(|element| element.uuid() != top_block.uuid())
            .skip(1)
            .take_while(|element| element.uuid() != bottom_block.uuid())
            .any(|element| {
                let level = self.element_manager.get_level(element.uuid());
                level < min_level
            });

        if has_common_ancestor_element {
            // find this common ancestor
            let common_ancestor = self
                .element_manager
                .find_common_ancestor(top_block.uuid(), bottom_block.uuid());

            removed_characters_count = self
                .element_manager
                .get(common_ancestor)
                .unwrap()
                .text_length();
            new_position = match self.element_manager.previous_element(common_ancestor) {
                Some(element) => element.end_of_element(),
                // means that the common ancestor is, in fact, the root frame
                None => 0,
            };

            parent_element_for_signal = match self
                .element_manager
                .get_parent_element_using_uuid(common_ancestor)
            {
                Some(parent_of_ancestor) => parent_of_ancestor,
                None => Element::FrameElement(self.element_manager.root_frame()),
            };

            self.element_manager.remove(vec![common_ancestor]);

            // in case root frame is removed
            if common_ancestor == 0 {
                self.element_manager.clear();

                parent_element_for_signal =
                    Element::FrameElement(self.element_manager.root_frame());
            }
        }
        // if top block's level is superior than (is a child of) bottom block

        // Frame  --> common ancestor, so it will be removed
        // |- Frame
        //    |- Block  --> top block, selection start
        //       |- Text
        // |- Block  --> bottom block, selection end
        //    |- Text
        else if top_block_level > bottom_block_level {
            //find ancestor which is direct child of bottom_block parent
            let sibling_ancestor = self
                .element_manager
                .find_ancestor_of_first_which_is_sibling_of_second(
                    top_block.uuid(),
                    bottom_block.uuid(),
                )
                .ok_or_else(|| {
                    ModelError::ElementNotFound("sibling ancestor not found".to_string())
                })?;

            removed_characters_count = self
                .element_manager
                .get(sibling_ancestor)
                .unwrap()
                .text_length();

            new_position = match self.element_manager.previous_element(sibling_ancestor) {
                Some(element) => element.end_of_element(),
                // means that the common ancestor is, in fact, the root frame
                None => 0,
            };

            parent_element_for_signal = match self
                .element_manager
                .get_parent_element_using_uuid(bottom_block.uuid())
            {
                Some(parent_of_ancestor) => parent_of_ancestor,
                None => Element::FrameElement(self.element_manager.root_frame()),
            };

            self.element_manager.remove(vec![sibling_ancestor]);

            removed_characters_count += bottom_block
                .remove_between_positions(0, right_position_in_block)?
                .1;

            self.element_manager.remove(
                self.element_manager
                    .list_all_children(0)
                    .iter()
                    .skip_while(|element| element.uuid() != top_block.uuid())
                    .skip(1)
                    .take_while(|element| element.uuid() != bottom_block.uuid())
                    .filter_map(|element| {
                        if element.is_block() {
                            removed_characters_count += element.text_length() + 1;
                            return Some(element.uuid());
                        }

                        if element.is_frame() {
                            return Some(element.uuid());
                        }
                        None
                    })
                    .collect(),
            );
        }
        // if bottom block's level is superior than (is a child of) top block

        // Frame  --> common ancestor, so it will be removed
        // |- Block  --> top block, selection start
        //    |- Text
        // |- Frame
        //    |- Block  --> bottom block, selection end
        //       |- Text
        else if top_block_level < bottom_block_level {
            parent_element_for_signal = match self
                .element_manager
                .get_parent_element_using_uuid(top_block.uuid())
            {
                Some(parent_of_ancestor) => parent_of_ancestor,
                None => Element::FrameElement(self.element_manager.root_frame()),
            };

            (new_position, removed_characters_count) = top_block
                .remove_between_positions(left_position_in_block, top_block.text_length())?;
            self.element_manager.debug_elements();

            self.element_manager.remove(
                self.element_manager
                    .list_all_children(0)
                    .iter()
                    .skip_while(|element| element.uuid() != top_block.uuid())
                    .skip(1)
                    .take_while(|element| element.uuid() != bottom_block.uuid())
                    .filter_map(|element| {
                        if element.is_block() {
                            removed_characters_count += element.text_length() + 1;
                            return Some(element.uuid());
                        }
                        if element.is_frame() {
                            return Some(element.uuid());
                        }

                        None
                    })
                    .collect(),
            );
        }
        // if bottom block's level is strictly at the same level than top block

        // Frame
        // |- Frame  --> common ancestor, so it will be removed
        //    |- Block  --> top block, selection start
        //       |- Text
        //    |- Block  --> bottom block, selection end
        //       |- Text
        else {
            parent_element_for_signal = match self
                .element_manager
                .get_parent_element_using_uuid(top_block.uuid())
            {
                Some(parent_of_ancestor) => parent_of_ancestor,
                None => Element::FrameElement(self.element_manager.root_frame()),
            };

            (new_position, removed_characters_count) = top_block
                .remove_between_positions(left_position_in_block, top_block.text_length())?;

            self.element_manager.remove(
                self.element_manager
                    .list_all_children(0)
                    .iter()
                    .skip_while(|element| element.uuid() != top_block.uuid())
                    .skip(1)
                    .take_while(|element| element.uuid() != bottom_block.uuid())
                    .filter_map(|element| {
                        if element.is_block() {
                            removed_characters_count += element.text_length() + 1;
                            return Some(element.uuid());
                        }
                        None
                    })
                    .collect(),
            );

            removed_characters_count += bottom_block
                .remove_between_positions(0, right_position_in_block)?
                .1;

            top_block.merge_with(bottom_block)?;
            removed_characters_count += 1;
        }

        self.element_manager.fill_empty_frames();
        self.element_manager.recalculate_sort_order();

        // reset cursor position and selection
        self.set_position(new_position, MoveMode::MoveAnchor);

        // signaling changes
        self.element_manager
            .signal_for_text_change(new_position, removed_characters_count, 0);

        if send_change_signals {
            self.element_manager.signal_for_element_change(
                parent_element_for_signal,
                ChangeReason::ChildrenChanged,
            );
        }

        Ok((new_position, removed_characters_count))
    }

    pub fn move_position(&mut self, move_operation: MoveOperation, move_mode: MoveMode) {
        match move_operation {
            MoveOperation::NoMove => (),
            MoveOperation::Start => self.set_position(0, move_mode),
            MoveOperation::StartOfLine => todo!(),
            MoveOperation::StartOfBlock => {
                self.set_position(self.current_block_rc().start(), move_mode)
            }
            MoveOperation::StartOfWord => todo!(),
            MoveOperation::PreviousBlock => todo!(),
            MoveOperation::PreviousCharacter => self.set_position(self.position - 1, move_mode),
            MoveOperation::PreviousWord => todo!(),
            MoveOperation::Up => todo!(),
            MoveOperation::Left => self.set_position(self.position - 1, move_mode),
            MoveOperation::WordLeft => todo!(),
            MoveOperation::End => {
                self.set_position(self.element_manager.root_frame().end(), move_mode)
            }
            MoveOperation::EndOfLine => todo!(),
            MoveOperation::EndOfWord => todo!(),
            MoveOperation::EndOfBlock => {
                self.set_position(self.current_block_rc().end(), move_mode)
            }
            MoveOperation::NextBlock => todo!(),
            MoveOperation::NextCharacter => self.set_position(self.position + 1, move_mode),
            MoveOperation::NextWord => todo!(),
            MoveOperation::Down => todo!(),
            MoveOperation::Right => self.set_position(self.position + 1, move_mode),
            MoveOperation::WordRight => todo!(),
            MoveOperation::NextCell => todo!(),
            MoveOperation::PreviousCell => todo!(),
            MoveOperation::NextRow => todo!(),
            MoveOperation::PreviousRow => todo!(),
        };
    }
}

/// If the anchor() is kept where it is and the position() is moved, the text_line in between will be selected.
#[derive(Clone, Copy, PartialEq)]
pub enum MoveMode {
    /// Moves the anchor to the same position as the cursor itself.
    MoveAnchor,
    /// Keeps the anchor where it is.
    KeepAnchor,
}
impl Default for MoveMode {
    fn default() -> Self {
        MoveMode::MoveAnchor
    }
}

pub enum MoveOperation {
    /// Keep the cursor where it is.
    NoMove,
    /// Move to the start of the document.
    Start,
    /// Move to the start of the current line.
    StartOfLine,
    /// Move to the start of the current block.
    StartOfBlock,
    /// Move to the start of the current word.
    StartOfWord,
    /// Move to the start of the previous block.
    PreviousBlock,
    /// Move to the previous character.
    PreviousCharacter,
    /// Move to the beginning of the previous word.
    PreviousWord,
    /// Move up one line.
    Up,
    /// Move left one character.
    Left,
    /// Move left one word.
    WordLeft,
    /// Move to the end of the document.
    End,
    /// Move to the end of the current line.
    EndOfLine,
    /// Move to the end of the current word.
    EndOfWord,
    /// Move to the end of the current block.
    EndOfBlock,
    /// Move to the beginning of the next block.
    NextBlock,
    /// Move to the next character.
    NextCharacter,
    /// Move to the next word.
    NextWord,
    /// Move down one line.
    Down,
    /// Move right one character.
    Right,
    /// Move right one word.
    WordRight,
    /// Move to the beginning of the next table cell inside the current table. If the current cell is the last cell in the row, the cursor will move to the first cell in the next row.
    NextCell,
    /// Move to the beginning of the previous table cell inside the current table. If the current cell is the first cell in the row, the cursor will move to the last cell in the previous row.
    PreviousCell,
    /// Move to the first new cell of the next row in the current table.
    NextRow,
    /// Move to the last cell of the previous row in the current table.
    PreviousRow,
}
