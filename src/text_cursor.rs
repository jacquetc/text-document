use std::rc::{Rc, Weak};

use crate::block::Block;
use crate::format::{BlockFormat, CharFormat, FrameFormat};
use crate::frame::{self, Frame};
use crate::text_document::Element::{BlockElement, FrameElement};
use crate::text_document::{ElementManager, ElementTrait, InsertMode, ModelError};

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

    pub fn set_position(&mut self, position: usize, move_mode: MoveMode) {
        match move_mode {
            MoveMode::MoveAnchor => {
                self.position = position;
                self.anchor_position = position;
            }
            MoveMode::KeepAnchor => self.position = position,
        }
    }

    pub fn current_block(&self) -> Weak<Block> {
        Rc::downgrade(&self.current_block_rc())
    }

    fn current_block_rc(&self) -> Rc<Block> {
        self.element_manager
            .find_block(self.position)
            .unwrap_or(self.element_manager.last_block().unwrap())
    }

    // split block at position, like if a new line is inserted
    pub fn insert_block(&mut self, block_format: BlockFormat) -> Result<Weak<Block>, ModelError> {
        // find reference block
        let old_block_rc = self
            .element_manager
            .find_block(self.position)
            .unwrap_or(self.element_manager.last_block().unwrap());

        let new_block =
            old_block_rc.split(old_block_rc.convert_position_from_document(self.position))?;

        // if new block empty, create empty child text element

        if &new_block.list_all_children().len() == &0 {
            self.element_manager
                .insert_new_text(new_block.uuid(), InsertMode::AsChild)?;
        }

        Ok(Rc::downgrade(&new_block))
    }

    pub fn current_frame(&self) -> Weak<Block> {
        Rc::downgrade(
            &self
                .element_manager
                .find_block(self.position)
                .unwrap_or(self.element_manager.last_block().unwrap()),
        )
    }
    pub fn insert_frame(&mut self, frame_format: FrameFormat) -> Weak<Frame> {
        // find reference block
        let old_block_rc = self
            .element_manager
            .find_block(self.position)
            .unwrap_or(self.element_manager.last_block().unwrap());

        let block_uuid = old_block_rc.uuid();

        let parent_frame = self
            .element_manager
            .get_parent_frame(&BlockElement(old_block_rc))
            .unwrap_or(self.element_manager.root_frame());
        let parent_uuid = parent_frame.uuid();

        // create block
        let new_frame = self
            .element_manager
            .insert_new_frame(block_uuid, InsertMode::After);

        // split and move fragments from one block to another
        todo!();

        match new_frame {
            Ok(frame) => Rc::downgrade(&frame),
            Err(_) => Weak::new(),
        }
    }

    /// Insert plain text and return position
    pub fn insert_plain_text<S: Into<String>>(&mut self, plain_text: S) -> usize {
        // get char format
        let char_format: CharFormat = match self.char_format() {
            Some(char_format) => char_format,
            None => self.current_block_rc().char_format(),
        };

        // fix positions
        let left_position = self.position.min(self.anchor_position);
        let right_position = self.anchor_position.max(self.position);

        if left_position != right_position {
            self.remove(left_position, right_position, false);
        }
        let mut new_position = left_position;
        /*
        let mut first_loop = true;


        let mut block = self
            .find_block(new_position)
            .unwrap_or(self.last_block().unwrap());
        for text in plain_text.into().split("\n") {
            if first_loop {
                block.insert_plain_text(
                    text,
                    block.convert_position_from_document(new_position),
                    &char_format,
                );

                first_loop = false;
            } else {
                block = self
                    .self_weak
                    .upgrade()
                    .unwrap()
                    .insert_block_using_position(new_position);
                block.set_plain_text(text, &char_format);
                new_position += 1;
            }

            new_position += text.len();
        } */

        // if send_change_signals {
        //     self.signal_for_cursor_change(position, 0, new_position);
        // }

        new_position
    }

    pub fn char_format(&self) -> Option<CharFormat> {
        let block_rc = self.current_block_rc();

        block_rc.char_format_at(block_rc.convert_position_from_document(self.position))
    }

    /// Remove elements between two positions. Split blocks if needed. Frames in superior level (i.e. children)
    ///  are completely removed even if only a part of it is selected
    pub(crate) fn remove(
        &self,
        position: usize,
        anchor_position: usize,
        send_change_signals: bool,
    ) -> Result<usize, ModelError> {
        let left_position = position.min(anchor_position);
        let right_position = anchor_position.max(position);

        let top_block =
            self.element_manager
                .find_block(left_position)
                .ok_or(ModelError::ElementNotFound(
                    "tob block not found".to_string(),
                ))?;
        let bottom_block =
            self.element_manager
                .find_block(right_position)
                .ok_or(ModelError::ElementNotFound(
                    "bottom block not found".to_string(),
                ))?;

            let left_position_in_block = top_block.convert_position_from_document(left_position);
            let right_position_in_block = top_block.convert_position_from_document(right_position);
        // same block:
        if top_block == bottom_block {
            top_block.remove_between_positions(left_position_in_block, right_position_in_block)?;
        }

        let top_block_level = self.element_manager.get_level(top_block.uuid());
        let bottom_block_level = self.element_manager.get_level(bottom_block.uuid());

        // determine if any element between top and bottom block is inferior than both, in this case the common ancestor is deleted whole

        let min_level = top_block_level.min(bottom_block_level);
        let has_ancestor_element = self
            .element_manager
            .list_all_children(0)
            .iter()
            .skip_while(|element| element.uuid() != top_block.uuid())
            .skip(1)
            .take_while(|element| element.uuid() != bottom_block.uuid())
            .any(|element| {
                let level = self.element_manager.get_level(element.uuid());
                level < min_level
            });

        if has_ancestor_element {
            // find this common ancestor
            let common_ancestor = self.element_manager.find_common_ancestor(top_block.uuid(), bottom_block.uuid());
            self.element_manager.remove(vec![common_ancestor]);


        }
        // if top block's level is superior than (is a child of) bottom block
        else if top_block_level > bottom_block_level {


            bottom_block.remove_between_positions(0, right_position_in_block)?;

            //find ancestor which is direct child of bottom_block parent
            let sibling_ancestor = self.element_manager.find_ancestor_of_first_which_is_sibling_of_second(top_block.uuid(), bottom_block.uuid()).ok_or(ModelError::ElementNotFound(
                "sibling ancestor not found".to_string(),
            ))?;

            self.element_manager.remove(vec![sibling_ancestor]);

            
            self.element_manager.remove(self
                .element_manager
                .list_all_children(0)
                .iter()
                .skip_while(|element| element.uuid() != top_block.uuid())
                .skip(1)
                .take_while(|element| element.uuid() != bottom_block.uuid()).map(|element|  element.uuid()).collect());
    

        }
        // if bottom block's level is superior than (is a child of) top block
        else if top_block_level < bottom_block_level {

            top_block.remove_between_positions(left_position_in_block, top_block.length())?;

            self.element_manager.remove(self
            .element_manager
            .list_all_children(0)
            .iter()
            .skip_while(|element| element.uuid() != top_block.uuid())
            .skip(1)
            .take_while(|element| element.uuid() != bottom_block.uuid()).map(|element|  element.uuid()).collect());


        }
        // if bottom block's level is strictly at the same level than top block
        else {
            top_block.remove_between_positions(left_position_in_block, top_block.length())?;

            self.element_manager.remove(self
                .element_manager
                .list_all_children(0)
                .iter()
                .skip_while(|element| element.uuid() != top_block.uuid())
                .skip(1)
                .take_while(|element| element.uuid() != bottom_block.uuid()).map(|element|  element.uuid()).collect());
    
    
            bottom_block.remove_between_positions(0, right_position_in_block)?;

        }


        todo!( adapt it to removed )
        Ok(left_position) 
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
