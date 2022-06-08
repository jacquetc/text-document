use std::cell::RefCell;
use std::rc::Rc;

use crate::text_document::TextDocument;
pub struct TextCursor {
    document: Rc<RefCell<TextDocument>>,
    position: usize,
    anchor_position: usize,
}

impl TextCursor {
    pub fn new(document: Rc<RefCell<TextDocument>>) -> Self {
        Self {
            document,
            ..Default::default()
        }
    }

    pub fn document(&self) -> &RefCell<TextDocument> {
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

    pub fn insert_block(&self){

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
