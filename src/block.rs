use crate::format::{BlockFormat, CharFormat};
use crate::text_document::TextDocument;
use std::borrow::Borrow;
use std::cell::{RefCell, Cell};
use std::rc::Rc;

#[derive(Clone, PartialEq)]
pub struct Block {
    document: Rc<TextDocument>,
    text_fragments: RefCell<Vec<TextFragment>>,
    /// Describes block-specific properties
    block_format: RefCell<BlockFormat>,
    /// Describes the block's character format. The block's character format is used when inserting text into an empty block.
    char_format: RefCell<CharFormat>,
}

impl Block {
    pub(crate) fn new(document: Rc<TextDocument>) -> Self {
        Block {
            document,
            ..Default::default()
        }
    }

    pub fn document(&self) -> &TextDocument {
        self.document.borrow()
    }

    /// Position of the cursor at the start of the block in the context of the document.
    pub fn position(&self) -> usize {
        let mut counter = 0;

        for block in self.document.block_list() {
            if block.eq(self) {
                break;
            }
            counter += block.borrow().length();
            counter += 1;
        }

        counter
    }

    // position of the end of the block in the context of the document
    pub fn end_position(&self) -> usize {
        self.position() + self.length()
    }

    /// Length of text in the block
    pub fn length(&self) -> usize {
        let mut counter: usize = 0;

        for fragment in self.text_fragments.borrow().into_iter() {
            counter += fragment.text.len();
        }

        counter
    }

    /// Number of this block in the whole document
    pub fn block_number(&self) -> usize {
        let mut counter = 0;

        for block in self.document.block_list() {
            if block.eq(self) {
                break;
            }

            counter += 1;
        }

        counter
    }
}

impl Default for Block {
    fn default() -> Self {
        Self {
            document: Default::default(),
            text_fragments: RefCell::new(vec![TextFragment::new()]),
            block_format: Default::default(),
            char_format: Default::default(),
        }
    }
}

#[derive(Default, Clone, PartialEq)]
pub(crate) struct TextFragment {
    text: String,
    char_format: CharFormat,
}

impl TextFragment {
    pub(crate) fn new() -> Self {
        TextFragment {
            ..Default::default()
        }
    }
}
