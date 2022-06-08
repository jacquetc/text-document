use std::rc::Rc;
use std::cell::RefCell;

use crate::{format::Format, frame::Frame};

#[derive(Default, PartialEq, Clone)]
pub struct TextDocument {
    formats: Vec<Format>,
    root_frame: Option<Rc<Frame>>,
}

impl TextDocument {
    pub fn new_rc_cell() -> Rc<RefCell<Self>> {
        let document_rc_cell = Rc::new(RefCell::new(Self { root_frame: None, ..Default::default()}));
        
        // create the root_frame: 
        let root_frame = Rc::new(Frame::new(document_rc_cell.clone()));
        document_rc_cell.borrow_mut().root_frame = Some(root_frame);

        document_rc_cell
    }


    pub fn block_count(&self) -> usize {}


    pub fn root_frame(&self) -> Option<&Rc<Frame>> {
        self.root_frame.as_ref()
    }
}


#[derive(Default, PartialEq, Clone)]
pub struct TextDocumentOption {
    pub tabs: Vec<Tab>,
    pub text_direction: TextDirection,
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
