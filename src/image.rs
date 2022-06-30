use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use crate::{
    format::{ImageFormat, FormattedElement},
    text_document::{Element, ElementManager, ElementTrait, ModelError},
    Block,
};
use crate::format::IsFormat;

#[derive(Default, Clone, Debug)]
pub struct Image {
    uuid: Cell<usize>,
    element_manager: Weak<ElementManager>,
    text: RefCell<String>,
    image_format: RefCell<ImageFormat>,
}

impl PartialEq for Image {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid && self.image_format == other.image_format
    }
}

impl Image {
    pub(crate) fn new(element_manager: Weak<ElementManager>) -> Self {
        Image {
            element_manager,
            uuid: Default::default(),
            text: RefCell::new(char::from_u32(0xfffc).unwrap().to_string()),
            ..Default::default()
        }
    }

    pub fn uuid(&self) -> usize {
        self.uuid.get()
    }

    pub(crate) fn image_format(&self) -> ImageFormat {
        self.format()
    }


    pub fn text(&self) -> String {
        self.text.borrow().clone()
    }

    pub fn text_length(&self) -> usize {
        self.text.borrow().len()
    }

    fn parent_bloc_rc(&self) -> Rc<Block> {
        let element_manager = self.element_manager.upgrade().unwrap();

        match element_manager
            .get_parent_element_using_uuid(self.uuid())
            .unwrap()
        {
            Element::BlockElement(block) => block,
            _ => unreachable!(),
        }
    }
    pub fn position_in_block(&self) -> usize {
        let parent_block = self.parent_bloc_rc();
        parent_block.position_of_child(self.uuid())
    }

    pub fn start(&self) -> usize {
        let parent_block = self.parent_bloc_rc();

        parent_block.position() + self.position_in_block()
    }

    pub fn end(&self) -> usize {
        self.start() + self.text_length()
    }
}

impl ElementTrait for Image {
    fn set_uuid(&self, uuid: usize) {
        self.uuid.set(uuid);
    }

    fn verify_rule_with_parent(&self, parent_element: &Element) -> Result<(), ModelError> {
        match parent_element {
            Element::FrameElement(_) => Err(ModelError::WrongParent),
            Element::BlockElement(_) => Ok(()),
            Element::TextElement(_) => Err(ModelError::WrongParent),
            Element::ImageElement(_) => Err(ModelError::WrongParent),
        }
    }
}


impl FormattedElement<ImageFormat> for Image {
    fn format(&self)-> ImageFormat {
           self.image_format.borrow().clone()
    }

    fn set_format(&self, format: &ImageFormat) -> Result<(), ModelError> {
        self.image_format.replace(format.clone());
        Ok(())
 }

    fn merge_format(&self, format: &ImageFormat) -> Result<ImageFormat, ModelError> {
        self.image_format.borrow_mut().merge(format)
    }

}
