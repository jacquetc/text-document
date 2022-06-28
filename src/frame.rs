use std::cell::Cell;
use std::rc::{Rc, Weak};
use std::{cell::RefCell};
use std::ops::Deref;
use crate::text_document::{ElementManager, ElementTrait, Element, ModelError};

use crate::{block::Block, format::FrameFormat};

#[derive(Clone, Debug)]
pub struct Frame {
      uuid: Cell<usize>,
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
    pub(crate) fn new(element_manager: Weak<ElementManager>) -> Self {


        Frame {
            element_manager,
            uuid: Default::default(),
            frame_format: RefCell::new(FrameFormat { ..Default::default() })
        }
    }

    pub fn first_cursor_position(&self) -> usize {
0
    }





}

impl ElementTrait for Frame {

    fn uuid(&self) -> usize{
            self.uuid.get()
    }

    fn set_uuid(&self, uuid: usize) {
        self.uuid.set(uuid);
    }
    fn verify_rule_with_parent(&self, parent_element: &Element) -> Result<(), ModelError> {
            match parent_element {
                Element::FrameElement(_) => Ok(()),
                Element::BlockElement(_) => Err(ModelError::WrongParent),
                Element::TextElement(_) => Err(ModelError::WrongParent),
                Element::ImageElement(_) => Err(ModelError::WrongParent),
            }
        }
}


