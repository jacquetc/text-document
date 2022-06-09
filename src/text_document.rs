use std::cell::RefCell;
use std::rc::Rc;

use crate::block::Block;
use crate::{format::Format, frame::Frame};

#[derive(Default, PartialEq, Clone)]
pub struct TextDocument {
    formats: Vec<Format>,
    root_frame: Option<Box<Frame>>,
}

impl TextDocument {
    pub fn new() -> Rc<Self> {
        let mut document = Rc::new(Self {
            root_frame: None,
            ..Default::default()
        });

        let root_frame = Frame::new(document.clone());
        document.root_frame = Some(Box::new(root_frame));
        document
    }

    pub fn block_count(&self) -> usize {
        match &self.root_frame {
            Some(frame) => frame.recursive_block_count(),
            None => 0,
        }
    }

    pub(crate) fn block_list(&self) -> Vec<&Block> {
        match self.root_frame {
            Some(frame) => frame.recursive_block_list(),
            None => vec![],
        }
    }

    pub fn root_frame(&self) -> Option<&Box<Frame>> {
        self.root_frame.as_ref()
    }

    pub fn character_count(&self) -> usize {
        let mut counter: usize = 0;

        self.block_list().into_iter().for_each(|block| {
            counter += block.length();
        });

        counter
    }

    pub fn find_block(&self, position: usize) -> Option<&Block> {
        for block in self.block_list().into_iter() {

            if (block.position()..block.end_position()).contains(&position) {
                return Some(&block);
            }
        }

        None
    }

    pub fn last_block(&self) -> &Block {
        self.block_list().last().unwrap()
    }
}

#[derive(Default, PartialEq, Clone)]
pub struct TextDocumentOption {
    pub tabs: Vec<Tab>,
    pub text_direction: TextDirection,
    pub wrap_mode: WrapMode,
}

#[derive(Default, PartialEq, Clone)]
pub struct Tab {
    pub position: usize,
    pub tab_type: TabType,
    pub delimiter: char,
}

#[derive(PartialEq, Clone, Copy)]
pub enum TabType {
    LeftTab,
    RightTab,
    CenterTab,
    DelimiterTab,
}

impl Default for TabType {
    fn default() -> Self {
        TabType::LeftTab
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum TextDirection {
    LeftToRight,
    RightToLeft,
}

impl Default for TextDirection {
    fn default() -> Self {
        TextDirection::LeftToRight
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum WrapMode {
    NoWrap,
    WordWrap,
    WrapAnywhere,
    WrapAtWordBoundaryOrAnywhere,
}

impl Default for WrapMode {
    fn default() -> Self {
        WrapMode::WordWrap
    }
}
