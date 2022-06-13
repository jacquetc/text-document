use std::rc::{Rc, Weak};
use std::{cell::RefCell};
use std::ops::Deref;
use crate::text_document::ElementManager;

use crate::{block::Block, format::FrameFormat};

#[derive(Clone)]
pub struct Frame {
      uuid: usize,
  element_manager: Weak<ElementManager>,
    /// Describes frame-specific properties
    frame_format: RefCell<FrameFormat>,
}

impl PartialEq for Frame {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid && self.frame_format == other.frame_format
    }
}

impl Frame {
    pub(crate) fn new(uuid: usize, element_manager: Weak<ElementManager>) -> Self {


        Frame {
            element_manager,
            uuid,
            frame_format: RefCell::new(FrameFormat { ..Default::default() })
        }
    }

    pub(crate) fn uuid(&self) -> usize{
            self.uuid
    }

    pub fn first_cursor_position(&self) -> usize {
0
    }



}


#[derive(Clone, PartialEq)]
pub(crate) enum Element {
    FrameElement(Rc<Frame>),
    BlockElement(Rc<Block>),
}
