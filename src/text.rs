use std::{
    cell::{Cell, RefCell},
    rc::{Weak, Rc},
};

use crate::{
    format::{CharFormat, FormattedElement, IsFormat},
    text_document::{Element, ElementManager, ElementTrait, ModelError}, Block,
};

#[derive(Default, Clone, Debug)]
pub struct Text {
    uuid: Cell<usize>,
    element_manager: Weak<ElementManager>,
    text: RefCell<String>,
    char_format: RefCell<CharFormat>,
}

impl PartialEq for Text {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid && self.char_format == other.char_format
    }
}

impl Text {
    pub(crate) fn new(element_manager: Weak<ElementManager>) -> Self {
        Text {
            element_manager,
            uuid: Default::default(),
            char_format: RefCell::new(CharFormat {
                ..Default::default()
            }),
            text: RefCell::new(String::new()),
        }
    }

    pub fn uuid(&self) -> usize {
        self.uuid.get()
    }
    pub(crate) fn char_format(&self) -> CharFormat {
        self.format()
    }

    pub(crate) fn set_char_format(&self, char_format: &CharFormat) {
    }

    pub fn plain_text(&self) -> String {
        self.text.borrow().clone()
    }

    pub(crate) fn set_text<S: Into<String>>(&self, text: S) {
        let plain_text: String = text.into();
        self.text.replace(plain_text.clone());
    }

    pub(crate) fn insert_plain_text<S: Into<String>>(&self, position_in_text: usize, text: S) {
        let plain_text: String = text.into();
        self.text
            .borrow_mut()
            .insert_str(position_in_text, plain_text.as_str())
    }

    pub(crate) fn split(&self, position_in_text: usize) -> Element {
        // create new element
        let element_manager = self.element_manager.upgrade().unwrap();
        let new_text_rc = element_manager
            .insert_new_text(self.uuid(), crate::text_document::InsertMode::After)
            .unwrap();

        let new_element = element_manager.get(new_text_rc.uuid()).unwrap();

        // populate text
        let original_text = self.plain_text();
        let split = original_text.split_at(position_in_text);
        self.set_text(&split.0.to_string());
        new_text_rc.set_text(&split.1.to_string());
        new_text_rc.set_char_format(&self.char_format());

        new_element
    }

    pub(crate) fn remove_text(
        &self,
        left_position_in_text: usize,
        right_position_in_text: usize,
    ) -> Result<(), ModelError> {
        let mut text = self.plain_text();

        if left_position_in_text > text.len() || right_position_in_text > text.len() {
            return Err(ModelError::OutsideElementBounds);
        }

        text.replace_range(left_position_in_text..right_position_in_text, "");
        self.set_text(&text);

        Ok(())
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

impl ElementTrait for Text {
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
impl FormattedElement<CharFormat> for Text {

    fn format(&self)-> CharFormat {
        self.char_format.borrow().clone()
    }

    fn set_format(&self, format: &CharFormat) -> Result<(), ModelError> {       
        self.char_format.replace(format.clone());
        Ok(())
    }

    fn merge_format(&self, format: &CharFormat) -> Result<CharFormat, ModelError> {
        self.char_format.borrow_mut().merge(format)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn list_all_children() {

        let text = Text::new(Weak::new());
        text.set_text("plain_text");
        text.remove_text(0, 10).unwrap();

        assert_eq!(text.plain_text(), "");

        text.set_text("plain_text");
        text.remove_text(1, 9).unwrap();
        assert_eq!(text.plain_text(), "pt");

    }

}
