use std::borrow::Borrow;
use std::{cell::RefCell, rc::Rc};
use std::ops::Deref;

use crate::{block::Block, format::FrameFormat, text_document::TextDocument};

#[derive(Clone, PartialEq)]
pub struct Frame {
    document: Rc<TextDocument>,
    elements: Vec<Element>,
    /// Describes frame-specific properties
    frame_format: RefCell<FrameFormat>,
}

impl Frame {
    pub(crate) fn new(document: Rc<TextDocument>) -> Self {
        // create a first empty block
        let first_block = Block::new(document);

        Frame {
            elements: vec![Element::BlockElement(first_block)],
            ..Default::default()
        }
    }

    pub(crate) fn set_document(&mut self, document: Rc<TextDocument>) {
        self.document = document;
    }

    pub fn document(&self) -> &TextDocument {
        self.document.borrow()
    }

    pub fn first_cursor_position(&self) -> usize {
        match &self.elements[0] {
            // search recursively for the block
            Element::FrameElement(frame) => match &frame[0] {
                Element::FrameElement(sub_frame) => sub_frame.first_cursor_position(),
                Element::BlockElement(sub_block) => sub_block.position(),
            },

            Element::BlockElement(block) => block.position(),
        }
    }

    pub(crate) fn recursive_block_count(&self) -> usize{

        let mut counter: usize = 0;

        for element in &self.elements {
            match element {
                Element::FrameElement(frame) => counter += frame.recursive_block_count(),
                Element::BlockElement(_) => counter += 1,
            }

        }
        counter

        
    }

    pub(crate) fn block_count(&self) -> usize{
        let mut counter: usize = 0;

        for element in &self.elements {
            match element {
                Element::FrameElement(_) => continue,
                Element::BlockElement(_) => counter += 1,
            }

        }
        counter
    }

    pub(crate) fn recursive_block_list(&self) -> Vec<&Block> {

        
        let mut block_list = Vec::new();

        for element in &self.elements {
            match element {
                Element::FrameElement(frame) => block_list.extend(frame.recursive_block_list()),
                Element::BlockElement(block) => block_list.push(block),
            }

        }
        block_list
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
pub enum Element {
    FrameElement(Frame),
    BlockElement(Block),
}
