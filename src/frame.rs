use std::{rc::Rc, ops::Deref};
use std::cell::RefCell;

use crate::{block::Block, format::FrameFormat, text_document::TextDocument};

#[derive(Clone, PartialEq)]
pub struct Frame {
    document: Rc<RefCell<TextDocument>>,
    elements: Vec<Element>,
    /// Describes frame-specific properties
    frame_format: FrameFormat,
}

impl Frame {
    pub(crate) fn new(document: Rc<RefCell<TextDocument>>) -> Self {
        let first_block = Block::new(document);

        Frame {
            elements: vec![Element::BlockElement(first_block)], 
            ..Default::default()
        }
    }

    

    pub(crate) fn set_document(&mut self, document: Rc<RefCell<TextDocument>>) {
        self.document = document;
    }

    pub fn document(&self) -> &RefCell<TextDocument> {
        self.document.as_ref()
    }

    pub fn first_cursor_position(&self) -> usize {
        match self.elements[0] {
            Element::FrameElement(frame) => if frame == self.document.borrow().root_frame().unwrap_or_default().
              {

            }  else{frame.first_cursor_position()},
            Element::BlockElement(block) => block.position(),
            
        }
    }

}

impl Default for Frame {
    fn default() -> Self {
        Self {
            document: Default::default(),
            elements: Vec::new(),
            frame_format: Default::default(),
        }
    }
}

impl Deref for Frame {
    type Target = Vec<Element>;

    fn deref(&self) -> &Self::Target {
        &self.elements
    }
    
}

#[derive(Clone, PartialEq)]
enum Element {
    FrameElement(Frame),
    BlockElement(Block),
}
