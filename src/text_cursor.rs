use std::rc::Rc;

use crate::block::Block;
use crate::text_document::TextDocument;
pub struct TextCursor {
    document: Rc<TextDocument>,
    position: usize,
    anchor_position: usize,
}

impl TextCursor {
    pub fn new(document: Rc<TextDocument>) -> Self {
        Self {
            document,
            ..Default::default()
        }
    }

    pub fn document(&self) -> &TextDocument {
        self.document.as_ref()
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

    pub fn current_block(&self) -> &Block {
        self.document
            .find_block(self.position)
            .unwrap_or_else(|| self.document.last_block())
    }

    pub fn insert_block(&self) {
        //let frame = self.document.borrow().find_frame(position);

    }
}

impl Default for TextCursor {
    fn default() -> Self {
        Self {
            document: Default::default(),
            position: Default::default(),
            anchor_position: Default::default(),
        }
    }
}

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
