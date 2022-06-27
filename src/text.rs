use std::{cell::{RefCell, Cell}, rc::Weak};

use crate::{format::CharFormat, text_document::{ElementTrait, ElementManager, Element, ModelError}};


#[derive(Default, Clone)]
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
            char_format: RefCell::new(CharFormat { ..Default::default() }),
            text: RefCell::new(String::new()),
        }
    }

    pub(crate) fn char_format(&self) -> CharFormat {
        self.char_format.borrow().clone()
    }

    pub(crate) fn set_char_format(&self, char_format: &CharFormat) {
        self.char_format.replace(char_format.clone());
    }

    pub fn plain_text(&self) -> String {
        self.text.borrow().clone()
    }

    pub(crate) fn set_text(&self, text: &String) {
        self.text.replace(text.clone());
    }

    pub(crate) fn insert_plain_text(&self, position_in_text: usize, text: &String){
        self.text.borrow_mut().insert_str(position_in_text, text.as_str())
    }

    pub(crate) fn split(&self, position_in_text: usize) -> Element{

        // create new element
        let element_manager = self.element_manager.upgrade().unwrap();
        let new_text_rc = element_manager.insert_new_text(self.uuid(), crate::text_document::InsertMode::After).unwrap();

        let new_element = element_manager.get(new_text_rc.uuid()).unwrap();

        // populate text
        let original_text = self.plain_text();
        let split = original_text.split_at(position_in_text);
        self.set_text(&split.0.to_string());
        new_text_rc.set_text(&split.1.to_string());
        new_text_rc.set_char_format(&self.char_format());

        new_element
    }

    pub(crate) fn remove_text(&self, left_position_in_text: usize, right_position_in_text: usize) -> Result<(), ModelError> {
        let mut text = self.plain_text();

        if left_position_in_text > text.len() || right_position_in_text > text.len() {
            return Err(ModelError::OutsideElementBounds);
        }

        text.replace_range((left_position_in_text .. right_position_in_text ), "");
        self.set_text(&text);

        Ok(())

    }

    pub fn len(&self) -> usize {
        self.text.borrow().len()
    }
}

impl ElementTrait for Text {

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