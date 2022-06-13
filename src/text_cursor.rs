use std::rc::{Rc, Weak};

use crate::block::Block;
use crate::format::{BlockFormat, FrameFormat, CharFormat};
use crate::frame::Frame;
use crate::text_document::{ElementManager};
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

    fn current_block_rc(&self) ->Rc<Block> {
        self.element_manager
            .find_block(self.position)
            .unwrap_or(self.element_manager.last_block())
 
    }


    pub fn insert_block(&self, block_format: BlockFormat) -> Weak<Block> {
todo!()
    }

    pub fn current_frame(&self) -> Weak<Block> {
        Rc::downgrade(&self.element_manager
            .find_block(self.position)
            .unwrap_or(self.element_manager.last_block()))
    }
    pub fn insert_frame(&self, frame_format: FrameFormat) -> Weak<Frame> {
        Rc::downgrade(&self.element_manager.insert_frame_using_position(self.position))
    }

    pub fn insert_plain_text<S: Into<String>>(&mut self, plain_text: S){
        let char_format: CharFormat = match self.char_format() {
            Some(char_format) => char_format,
            None => self.current_block_rc().char_format(),
        };


        self.position = self.element_manager.insert_plain_text(plain_text, self.position, char_format);
    }

    pub fn char_format(&self) -> Option<CharFormat> {
        let block_rc = self.current_block_rc();

            block_rc.char_format_at(block_rc.convert_position_from_document(self.position))
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