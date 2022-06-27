use std::{cell::{RefCell, Cell}, rc::Weak};

use crate::{format::ImageFormat, text_document::{ElementTrait, ElementManager, Element, ModelError}};

#[derive(Default, Clone)]
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
    pub(crate) fn image_format(&self) -> ImageFormat {
        self.image_format.borrow().clone()
    }

    pub(crate) fn set_image_format(&self, image_format: &ImageFormat) {
        self.image_format.replace(image_format.clone());
    }

    pub fn text(&self) -> String {
        self.text.borrow().clone()
    }

    pub fn len(&self) -> usize {
        self.text.borrow().len()
    }
}

impl ElementTrait for Image {


    fn uuid(&self) -> usize{
        self.uuid.get()
}

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