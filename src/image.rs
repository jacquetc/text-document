use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use crate::{
    format::{FormatChangeResult, IsFormat},
    ElementUuid,
};
use crate::{
    format::{FormattedElement, ImageFormat},
    text_document::{Element, ElementManager, ElementTrait, ModelError},
    Block,
};

#[derive(Default, Clone, Debug)]
pub struct Image {
    uuid: Cell<ElementUuid>,
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
            text: RefCell::new("\u{FFFC}".to_string()),
            ..Default::default()
        }
    }

    pub fn uuid(&self) -> ElementUuid {
        self.uuid.get()
    }

    pub(crate) fn image_format(&self) -> ImageFormat {
        self.format()
    }

    pub fn plain_text(&self) -> String {
        " ".to_string()
    }

    pub fn text_length(&self) -> usize {
        1
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
    fn format(&self) -> ImageFormat {
        self.image_format.borrow().clone()
    }

    fn set_format(&self, format: &ImageFormat) -> FormatChangeResult {
        if &*self.image_format.borrow() == format {
            Ok(None)
        } else {
            self.image_format.replace(format.clone());
            Ok(Some(()))
        }
    }

    fn merge_format(&self, format: &ImageFormat) -> FormatChangeResult {
        self.image_format.borrow_mut().merge_with(format)
    }
}

#[cfg(test)]
mod tests {

    use crate::InsertMode;

    use super::*;

    #[test]
    fn basics() {
        let image = Image::new(Weak::new());

        assert_eq!(image.uuid(), 0);
        assert_eq!(image.plain_text(), " ");
        assert_eq!(image.text_length(), 1);
        assert_eq!(image.image_format(), ImageFormat::new());

        let image_bis = Image::new(Weak::new());

        assert_eq!(image, image_bis);
    }

    #[test]
    fn position() {
        let element_manager_rc = ElementManager::new_rc();
        ElementManager::create_root_frame(element_manager_rc.clone());

        let image = element_manager_rc
            .insert_new_image(1, InsertMode::AsChild)
            .unwrap();

        assert_eq!(image.parent_bloc_rc().uuid(), 1);
        assert_eq!(image.start(), 0);
        assert_eq!(image.end(), 1);
    }
}
