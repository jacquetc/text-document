use crate::format::{BlockFormat, CharFormat};
use crate::text_document::TextDocument;
use std::rc::Rc;
use std::cell::RefCell;

#[derive(Clone, PartialEq)]
pub struct Block {
    document: Rc<RefCell<TextDocument>>,
    text_fragments: Vec<TextFragment>,
    /// Describes block-specific properties
    block_format: BlockFormat,
    /// Describes the block's character format. The block's character format is used when inserting text into an empty block.
    char_format: CharFormat,
}

impl Block {
    pub(crate) fn new(document: Rc<RefCell<TextDocument>>) -> Self {
        Block {
            document,
            ..Default::default()
        }
    }


    pub fn document(&self) -> &RefCell<TextDocument> {
        self.document.as_ref()
    }

    pub fn position(&self) -> usize {
        0
    }
}

impl Default for Block {
    fn default() -> Self {
        Self {
            document: Default::default(),
            text_fragments: vec![TextFragment::new()],
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
