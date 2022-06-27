use crate::format::{BlockFormat, CharFormat, ImageFormat};
use crate::text::Text;
use crate::text_document::Element::{ImageElement, TextElement};
use crate::text_document::{Element, ElementManager, ElementTrait, ModelError};
use std::borrow::{Borrow, BorrowMut};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::{Rc, Weak};

#[derive(Clone)]
pub struct Block {
    uuid: Cell<usize>,
    element_manager: Weak<ElementManager>,
    /// Describes block-specific properties
    block_format: RefCell<BlockFormat>,
}

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid && self.block_format == other.block_format
    }
}

impl Block {
    pub(crate) fn new(element_manager: Weak<ElementManager>) -> Self {
        Block {
            uuid: Default::default(),
            element_manager,
            block_format: Default::default(),
        }
    }
    /// Position of the cursor at the start of the block in the context of the document.
    pub fn position(&self) -> usize {
        let mut counter = 0;

        for block in self.element_manager.upgrade().unwrap().block_list() {
            if block.as_ref().eq(self) {
                break;
            }
            counter += block.length();
            counter += 1;
        }

        counter
    }

    pub(crate) fn uuid(&self) -> usize {
        self.uuid.get()
    }

    pub fn set_uuid(&self, uuid: usize) {
        self.uuid.set(uuid);
    }

    // position of the end of the block in the context of the document
    pub fn end_position(&self) -> usize {
        self.position() + self.length()
    }

    /// Length of text in the block
    pub fn length(&self) -> usize {
        let element_manager = self.element_manager.upgrade().unwrap();

        let all_children = self.list_all_children();
        let mut counter: usize = 0;

        for element in all_children {
            counter += match element {
                TextElement(text) => text.plain_text().len(),
                ImageElement(_) => 1,
                _ => 0,
            };
        }

        counter
    }

    /// Number of this block in the whole document
    pub fn block_number(&self) -> usize {
        let mut counter = 0;

        for block in self.element_manager.upgrade().unwrap().block_list() {
            if block.as_ref().eq(self) {
                break;
            }

            counter += 1;
        }

        counter
    }

    pub(crate) fn convert_position_from_document(&self, position_in_document: usize) -> usize {
        position_in_document - self.position()
    }
    pub(crate) fn convert_position_from_block_to_child(&self, position_in_block: usize) -> usize {
        let mut position = 0;
        for child in self.list_all_children() {
            if position_in_block == 0 {
                return 0;
            }

            let child_end_position = match &child {
                TextElement(text_rc) => text_rc.len(),
                ImageElement(image_rc) => image_rc.len(),
                _ => unreachable!(),
            };

            if (position..=child_end_position).contains(&position_in_block) {
                return position_in_block - position;
            }

            position += child_end_position;
        }

        position
    }

    pub(crate) fn char_format_at(&self, position_in_block: usize) -> Option<CharFormat> {
        if position_in_block == 0 {
            match self.first_child() {
                Some(element) => match element {
                    TextElement(text) => Some(text.char_format().clone()),
                    ImageElement(_) => None,
                    _ => None,
                },
                None => return None,
            }
        } else {
            None
        }
    }

    fn first_child(&self) -> Option<Element> {
        let element_manager = self.element_manager.upgrade().unwrap();

        let next_element = element_manager.next_element(self.uuid())?;
        match next_element {
            TextElement(_) => Some(next_element),
            ImageElement(_) => Some(next_element),
            _ => None,
        }
    }

    /// Find element inside the blocking using the cursor position in block
    /// Returns the element
    fn find_element(&self, position_in_block: usize) -> Option<Element> {
        let mut position = 0;

        for child in self.list_all_children() {
            // returns first element if cursor is at first postion
            if position_in_block == 0 {
                return Some(child);
            }

            let child_end_position = match &child {
                TextElement(text_rc) => text_rc.len(),
                ImageElement(image_rc) => image_rc.len(),
                _ => unreachable!(),
            };

            if (position..=child_end_position).contains(&position_in_block) {
                return Some(child);
            }

            position += child_end_position;
        }

        None
    }

    pub(crate) fn insert_plain_text(&self, plain_text: &str, position_in_block: usize) {
        match self.find_element(position_in_block) {
            Some(element) => match element {
                TextElement(text_rc) => text_rc.insert_plain_text(
                    self.convert_position_from_block_to_child(position_in_block),
                    &plain_text.to_string(),
                ),
                ImageElement(_) => {
                    let new_text_element = self.insert_text_element(position_in_block);
                }
                _ => unreachable!(),
            },
            None => return,
        }
    }

    fn insert_text_element(&self, position_in_block: usize) -> Element {
        match self.find_element(position_in_block) {
            Some(element) => match element {
                TextElement(text_rc) => {
                    // split
                    let second_text_element =
                        text_rc.split(self.convert_position_from_block_to_child(position_in_block));
                    // insert new text between splits
                    let element_manager = self.element_manager.upgrade().unwrap();
                    let new_text_rc = element_manager
                        .insert_new_text(text_rc.uuid(), crate::text_document::InsertMode::After);
                    element_manager.get(new_text_rc.unwrap().uuid()).unwrap()
                }
                ImageElement(_) => {
                    // add text after
                    let element_manager = self.element_manager.upgrade().unwrap();
                    let new_text_rc = element_manager
                        .insert_new_text(element.uuid(), crate::text_document::InsertMode::After);
                    element_manager.get(new_text_rc.unwrap().uuid()).unwrap()
                }
                _ => unreachable!(),
            },
            None => unreachable!(),
        }
    }

    pub(crate) fn set_plain_text(&self, plain_text: &str, char_format: &CharFormat) {
        self.clear();
        self.insert_plain_text(plain_text, 0);
    }

    /// helper function to clear all children of this block
    pub(crate) fn clear(&self) {
        let element_manager = self.element_manager.upgrade().unwrap();
        let children = self
            .list_all_children()
            .iter()
            .map(|element| element.uuid())
            .collect();

        element_manager.remove(children);
    }

    pub(crate) fn list_all_children(&self) -> Vec<Element> {
        let element_manager = self.element_manager.upgrade().unwrap();
        element_manager.list_all_children(self.uuid())
    }

    /// Describes the block's character format. The block's character format is the char format of the first block.
    pub fn char_format(&self) -> CharFormat {
        match self.first_child().unwrap() {
            TextElement(text_fragment) => text_fragment.char_format().clone(),
            ImageElement(_) => CharFormat::new(),
            _ => unreachable!(),
        }
    }

    /// Apply a new char format on all text fragments of this block
    pub(crate) fn set_char_format(&self, char_format: &CharFormat) {
        self.list_all_children()
            .iter()
            .filter_map(|element| match element {
                TextElement(text) => Some(text),
                ImageElement(_) => None,
                _ => unreachable!(),
            })
            .for_each(|text_fragment: &Rc<Text>| {
                text_fragment.set_char_format(&char_format);
            });
    }

    pub(crate) fn split(&self, position_in_block: usize) -> Result<Rc<Block>, ModelError> {
        
        let element_manager = self
            .element_manager
            .upgrade()
            .unwrap();
        
        // create block
        let new_block = element_manager
            .insert_new_block(self.uuid(), crate::text_document::InsertMode::After)?;

        // split child element at position 

        let sub_element = match self.find_element(position_in_block) {
            Some(element) => element,
            None => todo!(),
        };



        let new_text_after_text_split = match sub_element {
            TextElement(text) => text.split(self.convert_position_from_block_to_child(position_in_block)),
            ImageElement(image) => TextElement(element_manager.insert_new_text(image.uuid(), crate::text_document::InsertMode::After)?),
            _ => unreachable!()
        };

        // move fragments from one block to another
        let all_children_list = self.list_all_children();
        let mut child_list: Vec<&Element> = all_children_list.iter()
        .skip_while(|element| element.uuid() != new_text_after_text_split.uuid()).collect();
        child_list.reverse();

        for child in child_list {
             element_manager.move_while_changing_parent(child.uuid(), new_block.uuid())?;
       
        }

        Ok(new_block)

    }

    fn analyse_for_merges(&self) {
        todo!()
    }

    fn merge_text_elements(
        &self,
        first_text_fragment: Rc<Text>,
        second_text_fragment: Rc<Text>,
    ) -> Rc<Text> {
        todo!()
    }

    fn split_text_fragment_at(&self, position_in_block: usize) -> (Rc<Text>, Rc<Text>) {
        todo!()
    }

    /// returns the plain text of this block
    pub fn plain_text(&self) -> String {
        let texts: Vec<String> = self
            .list_all_children()
            .iter()
            .map(|fragment| match fragment {
                TextElement(text_rc) => text_rc.plain_text().to_string(),
                ImageElement(image_rc) => image_rc.text().to_string(),
                _ => unreachable!(),
            })
            .collect();
        texts.join("")
    }

    pub(crate) fn remove_between_positions(
        &self,
        position_in_block: usize,
        anchor_position_in_block: usize,
    ) {
    }
}

impl ElementTrait for Block {
    fn uuid(&self) -> usize {
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
