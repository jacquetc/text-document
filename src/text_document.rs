use std::borrow::BorrowMut;
use std::cell::{RefCell, Cell};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use crate::block::Block;
use crate::frame::Element;
use crate::frame::Element::{FrameElement, BlockElement};
use crate::{format::Format, frame::Frame};

#[derive(Default, PartialEq, Clone)]
pub struct TextDocument {
    formats: Vec<Format>,
    id_with_element_hash: RefCell<HashMap<usize, Box<Element>>>,
    order_with_id_map: RefCell<BTreeMap<usize, usize>>,
    child_id_with_parent_id_hash: RefCell<HashMap<usize, usize>>,
    id_counter: Cell<usize>,
}

impl TextDocument {
    pub fn new_rc() -> Rc<Self> {
        let document = Rc::new(Self {
            ..Default::default()
        });

        // root frame:
        document.id_with_element_hash.borrow_mut().insert(0, Box::new(Element::FrameElement(Frame::new())));
        document.order_with_id_map.borrow_mut().insert(0,0);
        document.child_id_with_parent_id_hash.borrow_mut().insert(0, 0);
        document.id_counter.set(document.id_counter.get() + 1);

        document
    }  

    fn insert_frame(&self) -> &Box<Element> {

        
    }

    pub fn block_count(&self) -> usize {

        let mut counter = 1;
        self.id_with_element_hash.borrow().values().for_each(|element| {
            counter += match element.as_ref() {
                BlockElement(_) =>  1,
                _ => 0,
            }
        });
        counter
    }

    pub(crate) fn block_list(&self) -> Vec<&Block> {
        self.id_with_element_hash.borrow().values().into_iter().skip_while(|x| match x.as_ref() {
            BlockElement(_) => true,
            _ => false,
        }).collect()
        
    }

    pub fn root_frame(&self) -> &Box<Element> {
        self.id_with_element_hash.borrow()
            .entry(0)
            .or_insert(Box::new(FrameElement(Frame::new())))
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
