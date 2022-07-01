use crate::format::FormatChangeResult;
use crate::text_document::{Element, ElementManager, ElementTrait, ModelError};
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Weak;

use crate::format::FormattedElement;
use crate::format::FrameFormat;
use crate::format::IsFormat;

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
            frame_format: RefCell::new(FrameFormat {
                ..Default::default()
            }),
        }
    }

    pub fn uuid(&self) -> usize {
        self.uuid.get()
    }

    pub fn frame_format(&self) -> FrameFormat {
        self.format()
    }

    pub fn first_cursor_position(&self) -> usize {
        let element_manager = self.element_manager.upgrade().unwrap();
        element_manager
            .next_element(self.uuid())
            .unwrap()
            .start_of_element()
    }

    pub(crate) fn list_all_direct_children(&self) -> Vec<Element> {
        let element_manager = self.element_manager.upgrade().unwrap();
        element_manager.list_all_direct_children(self.uuid())
    }

    pub(crate) fn list_all_children(&self) -> Vec<Element> {
        let element_manager = self.element_manager.upgrade().unwrap();
        element_manager.list_all_children(self.uuid())
    }

    pub fn text_length(&self) -> usize {
        let char_count: usize = self
            .list_all_direct_children()
            .iter()
            .map(|element| -> usize {
                match element {
                    Element::FrameElement(frame) => frame.text_length() + 1,
                    Element::BlockElement(block) => block.text_length() + 1,
                    _ => 0,
                }
            })
            .sum();

        char_count - 1
    }

    pub fn start(&self) -> usize {
        self.first_cursor_position()
    }

    pub fn end(&self) -> usize {
        self.start() + self.text_length()
    }
}

impl ElementTrait for Frame {
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

impl FormattedElement<FrameFormat> for Frame {
    fn format(&self) -> FrameFormat {
        self.frame_format.borrow().clone()
    }
    fn set_format(&self, format: &FrameFormat) -> FormatChangeResult {
        if &*self.frame_format.borrow() == format {
            Ok(None)
        } else {
        self.frame_format.replace(format.clone());
        Ok(Some(()))
    }
    }

    fn merge_format(&self, format: &FrameFormat) -> Result<Option<()>, ModelError> {
        self.frame_format.borrow_mut().merge_with(format)
    }
}
