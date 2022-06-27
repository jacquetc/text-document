use std::rc::{Rc, Weak};

use crate::block::Block;
use crate::format::{BlockFormat, CharFormat, FrameFormat};
use crate::frame::{self, Frame};
use crate::text_document::{ElementManager, ModelError, InsertMode, ElementTrait};
use crate::text_document::Element::{BlockElement, FrameElement};

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
        let old_block_rc = self.element_manager
            .find_block(self.position)
            .unwrap_or(self.element_manager.last_block().unwrap());


        let new_block = old_block_rc.split(old_block_rc.convert_position_from_document(self.position))?;



        // if new block empty, create empty child text element

        if &new_block.list_all_children().len() == &0 {
            self.element_manager.insert_new_text(new_block.uuid(), InsertMode::AsChild)?;
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
        let old_block_rc = self.element_manager
            .find_block(self.position)
            .unwrap_or(self.element_manager.last_block().unwrap());

        let block_uuid = old_block_rc.uuid();

        let parent_frame = self.element_manager
            .get_parent_frame(&BlockElement(old_block_rc))
            .unwrap_or(self.element_manager.root_frame());
        let parent_uuid = parent_frame.uuid();

        // create block
        let new_frame = self.element_manager.insert_new_frame(block_uuid, InsertMode::After);

        // split and move fragments from one block to another
        todo!();

        match new_frame {
            Ok(frame) => Rc::downgrade(&frame),
            Err(_) => Weak::new(),
        }
        
    }

    /// Insert plain text and return position
    pub fn insert_plain_text<S: Into<String>>(&mut self, plain_text: S) -> usize{

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


    pub(crate) fn remove(
        &self,
    position: usize,
    anchor_position: usize,
    send_change_signals: bool,
) -> Option<usize> {
    let left_position = position.min(anchor_position);
    let right_position = anchor_position.max(position);

    let top_block = self.element_manager.find_block(left_position)?;
    let bottom_block = self.element_manager.find_block(right_position)?;

    // same block:
    if top_block == bottom_block {
        top_block.remove_between_positions(left_position, right_position);
    }

    None

    /*         if send_change_signals {
            self.signal_for_cursor_change(position, 0, new_position);
    } */
}




}

/// If the anchor() is kept where it is and the position() is moved, the text in between will be selected.
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
