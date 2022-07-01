use crate::format::{BlockFormat, CharFormat, FormattedElement, IsFormat};
use crate::text::Text;
use crate::text_document::Element::{ImageElement, TextElement};
use crate::text_document::{Element, ElementManager, ElementTrait, ModelError};
use crate::ElementUuid;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

#[derive(Clone, Debug)]
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

    pub fn uuid(&self) -> usize {
        self.uuid.get()
    }

    pub fn iter(&self) -> BlockIter {
        BlockIter::new(self)
    }

    /// Position of the cursor at the start of the block in the context of the document.
    pub fn position(&self) -> usize {
        let mut counter = 0;

        for block in self.element_manager.upgrade().unwrap().block_list() {
            if block.as_ref().eq(self) {
                break;
            }
            counter += block.text_length();
            counter += 1;
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

    /// get this block formatting
    pub fn block_format(&self) -> BlockFormat {
        self.format()
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
                TextElement(text_rc) => position + text_rc.text_length(),
                ImageElement(image_rc) => position + image_rc.text_length(),
                _ => unreachable!(),
            };

            if (position..=child_end_position).contains(&position_in_block) {
                return position_in_block - position;
            }

            position += child_end_position;
        }

        position
    }

    /// Returns the position of child in the context of  this block
    pub(crate) fn position_of_child(&self, uuid: ElementUuid) -> usize {
        let mut position = 0;
        for child in self.list_all_children() {
            if child.uuid() == uuid {
                break;
            }

            let length = match &child {
                TextElement(text_rc) => text_rc.text_length(),
                ImageElement(image_rc) => image_rc.text_length(),
                _ => unreachable!(),
            };

            position += length;
        }

        position
    }

    pub(crate) fn char_format_at(&self, position_in_block: usize) -> Option<CharFormat> {
        if position_in_block == 0 {
            match self.first_child() {
                Some(element) => match element {
                    TextElement(text) => Some(text.char_format()),
                    ImageElement(_) => None,
                    _ => None,
                },
                None => None,
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

    /// Find element inside the block using the cursor position in block
    /// Returns the element
    fn find_element(&self, position_in_block: usize) -> Option<Element> {
        let mut position = 0;

        for child in self.list_all_children() {
            // returns first element if cursor is at first position
            if position_in_block == 0 {
                return Some(child);
            }

            let child_end_position = match &child {
                TextElement(text_rc) => position + text_rc.text_length(),
                ImageElement(image_rc) => position + image_rc.text_length(),
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
                    let new_text_rc = self.insert_new_text_element(position_in_block);
                    new_text_rc.set_text(&plain_text.to_string());
                    new_text_rc.set_format(&self.char_format()).unwrap();
                }
                _ => unreachable!(),
            },
            None => (),
        }
    }

    fn insert_new_text_element(&self, position_in_block: usize) -> Rc<Text> {
        match self.find_element(position_in_block) {
            Some(element) => match element {
                TextElement(text_rc) => {
                    // split if not at the end of the text
                    if position_in_block != text_rc.position_in_block() + text_rc.text_length() {
                        text_rc.split(self.convert_position_from_block_to_child(position_in_block));
                    }
                    // insert new text between splits
                    let element_manager = self.element_manager.upgrade().unwrap();
                    let new_text_rc = element_manager
                        .insert_new_text(text_rc.uuid(), crate::text_document::InsertMode::After);
                    new_text_rc.unwrap()
                }
                ImageElement(_) => {
                    // add text after
                    let element_manager = self.element_manager.upgrade().unwrap();
                    let new_text_rc = element_manager
                        .insert_new_text(element.uuid(), crate::text_document::InsertMode::After);
                    new_text_rc.unwrap()
                }
                _ => unreachable!(),
            },
            None => unreachable!(),
        }
    }

    pub(crate) fn set_plain_text(&self, plain_text: &str) {
        self.clear();
        self.insert_plain_text(plain_text, 0);
    }

    /// helper function to clear all children of this block. Create a new empty text element.
    pub(crate) fn clear(&self) {
        let element_manager = self.element_manager.upgrade().unwrap();
        let children = self
            .list_all_children()
            .iter()
            .map(|element| element.uuid())
            .collect();

        element_manager.remove(children);

        element_manager
            .insert_new_text(self.uuid(), crate::text_document::InsertMode::AsChild)
            .unwrap();
    }

    pub(crate) fn list_all_children(&self) -> Vec<Element> {
        let element_manager = self.element_manager.upgrade().unwrap();
        element_manager.list_all_children(self.uuid())
    }

    /// Describes the block's character format. The block's character format is the char format of the first block.
    pub fn char_format(&self) -> CharFormat {
        match self.first_child().unwrap() {
            TextElement(text_fragment) => text_fragment.char_format(),
            ImageElement(_) => CharFormat::new(),
            _ => unreachable!(),
        }
    }

    /// Apply a new char format onto all text fragments of this block
    pub(crate) fn set_char_format(&self, char_format: &CharFormat) {
        self.list_all_children()
            .iter()
            .filter_map(|element| match element {
                TextElement(text) => Some(text),
                ImageElement(_) => None,
                _ => unreachable!(),
            })
            .for_each(|text_fragment: &Rc<Text>| {
                text_fragment.set_format(char_format).unwrap();
            });
    }

    pub(crate) fn split(&self, position_in_block: usize) -> Result<Rc<Block>, ModelError> {
        let element_manager = self.element_manager.upgrade().unwrap();

        // create block
        let new_block = element_manager
            .insert_new_block(self.uuid(), crate::text_document::InsertMode::After)?;

        // split child element at position

        let sub_element = self
            .find_element(position_in_block)
            .ok_or_else(|| ModelError::ElementNotFound("sub element not found".to_string()))?;

        let new_text_after_text_split = match sub_element {
            TextElement(text) => {
                text.split(self.convert_position_from_block_to_child(position_in_block))
            }
            ImageElement(image) => TextElement(
                element_manager
                    .insert_new_text(image.uuid(), crate::text_document::InsertMode::After)?,
            ),
            _ => unreachable!(),
        };

        // move fragments from one block to another
        let all_children_list = self.list_all_children();
        let mut child_list: Vec<&Element> = all_children_list
            .iter()
            .skip_while(|element| element.uuid() != new_text_after_text_split.uuid())
            .collect();
        child_list.reverse();

        for child in child_list {
            element_manager.move_while_changing_parent(child.uuid(), new_block.uuid())?;
        }

        Ok(new_block)
    }

    fn analyze_for_merges(&self) {
        let children = self.list_all_children();

        'first_loop: for _ in 0..children.len() {
            let children = self.list_all_children();
            for element_window in children.windows(2) {
                let first_text = match &element_window[0] {
                    TextElement(text) => text,
                    _ => continue,
                };
                let second_text = match &element_window[1] {
                    TextElement(text) => text,
                    _ => continue,
                };

                if first_text.char_format() == second_text.char_format() {
                    self.merge_text_elements(first_text, second_text);
                    continue 'first_loop;
                }
            }
        }

        // remove empty text
        //todo!();
    }

    pub(crate) fn merge_with(&self, other_block: Rc<Block>) -> Result<(), ModelError> {
        let element_manager = self.element_manager.upgrade().unwrap();

        let mut own_children = self.list_all_children();
        let mut other_children = other_block.list_all_children();

        own_children.append(&mut other_children);
        own_children.reverse();

        own_children.iter().try_for_each(|element| {
            element_manager.move_while_changing_parent(element.uuid(), self.uuid())
        })?;

        element_manager.remove(vec![other_block.uuid()]);

        Ok(())
    }

    /// merge to texts, adopts the first text's char format
    fn merge_text_elements(&self, first_text_rc: &Rc<Text>, second_text_rc: &Rc<Text>) -> Rc<Text> {
        first_text_rc
            .set_text(&(first_text_rc.plain_text() + second_text_rc.plain_text().as_str()));
        let element_manager = self.element_manager.upgrade().unwrap();
        element_manager.remove(vec![second_text_rc.uuid()]);

        first_text_rc.clone()
    }

    /// returns the plain text of this block
    pub fn plain_text(&self) -> String {
        let texts: Vec<String> = self
            .list_all_children()
            .iter()
            .map(|fragment| match fragment {
                TextElement(text_rc) => text_rc.plain_text(),
                ImageElement(image_rc) => image_rc.plain_text(),
                _ => unreachable!(),
            })
            .collect();
        texts.join("")
    }

    pub(crate) fn plain_text_between_positions(
        &self,
        position_in_block: usize,
        anchor_position_in_block: usize,
    ) -> String {
        let mut position_in_block = position_in_block;
        let mut anchor_position_in_block = anchor_position_in_block;

        let text_length = self.text_length();

        if position_in_block > text_length {
            position_in_block = text_length;
        }
        if anchor_position_in_block > text_length {
            anchor_position_in_block = text_length;
        }

        self.plain_text()[position_in_block..anchor_position_in_block].to_string()
    }

    /// Remove text between two positions. Returns the position in the context of the document and the count of removed characters
    pub(crate) fn remove_between_positions(
        &self,
        position_in_block: usize,
        anchor_position_in_block: usize,
    ) -> Result<(usize, usize), ModelError> {
        let left_position = position_in_block.min(anchor_position_in_block);
        let right_position = anchor_position_in_block.max(position_in_block);

        let left_element = self
            .find_element(left_position)
            .ok_or_else(|| ModelError::ElementNotFound("left_element not found".to_string()))?;
        let right_element = self
            .find_element(right_position)
            .ok_or_else(|| ModelError::ElementNotFound("right_element not found".to_string()))?;

        // if same element targeted
        if left_element == right_element {
            match left_element {
                TextElement(text) => {
                    let left_position_in_child =
                        self.convert_position_from_block_to_child(left_position);
                    let right_position_in_child =
                        self.convert_position_from_block_to_child(right_position);
                    text.remove_text(left_position_in_child, right_position_in_child)?;
                }
                // nothing to remove since image length is 1
                ImageElement(_) => return Ok((0, 0)),
                _ => unreachable!(),
            }
        }
        // if different elements
        else {
            let element_manager = self.element_manager.upgrade().unwrap();

            // remove first part of last element
            match &right_element {
                TextElement(text) => {
                    let left_position_in_child = 0;
                    let right_position_in_child =
                        self.convert_position_from_block_to_child(right_position);
                    text.remove_text(left_position_in_child, right_position_in_child)?;
                }
                // remove completely  since image length is 1
                ImageElement(image) => element_manager.remove(vec![image.uuid()]),
                _ => unreachable!(),
            }

            // remove end part of first element
            match &left_element {
                TextElement(text) => {
                    let left_position_in_child =
                        self.convert_position_from_block_to_child(left_position);
                    let right_position_in_child = text.text_length();
                    text.remove_text(left_position_in_child, right_position_in_child)?;
                }
                // nothing to remove since image length is 1
                ImageElement(_) => (),
                _ => unreachable!(),
            }

            // remove all elements in between

            element_manager.remove(
                self.list_all_children()
                    .iter()
                    .skip_while(|element| element.uuid() != left_element.uuid())
                    .skip(1)
                    .take_while(|element| element.uuid() != right_element.uuid())
                    .map(|element| element.uuid())
                    .collect(),
            )
        }

        self.analyze_for_merges();

        let removed_characters_count = right_position - left_position;

        let new_position_in_document = self.position() + left_position;

        Ok((new_position_in_document, removed_characters_count))
    }

    /// Length of text in the block
    pub fn text_length(&self) -> usize {
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
    /// position of the start of the block in the context of the document
    pub fn start(&self) -> usize {
        self.position()
    }

    /// position of the end of the block in the context of the document
    pub fn end(&self) -> usize {
        self.start() + self.text_length()
    }
}

impl ElementTrait for Block {
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

impl FormattedElement<BlockFormat> for Block {
    fn format(&self) -> BlockFormat {
        self.block_format.borrow().clone()
    }

    fn set_format(&self, format: &BlockFormat) -> Result<(), ModelError> {
        self.block_format.replace(format.clone());
        Ok(())
    }

    fn merge_format(&self, format: &BlockFormat) -> Result<BlockFormat, ModelError> {
        self.block_format.borrow_mut().merge(format)
    }
}

pub struct BlockIter {
    unvisited: Vec<Element>,
}

impl BlockIter {
    fn new(block: &Block) -> Self {
        let ordered_elements = block.list_all_children();

        BlockIter {
            unvisited: ordered_elements,
        }
    }
}

impl Iterator for BlockIter {
    type Item = Element;

    fn next(&mut self) -> Option<Self::Item> {
        let element = self.unvisited.pop()?;

        Some(element)
    }
}

#[cfg(test)]
mod tests {
    use crate::text_document::InsertMode;

    use super::*;

    #[test]
    fn list_all_children() {
        let element_manager_rc = ElementManager::new_rc();
        ElementManager::create_root_frame(element_manager_rc.clone());

        let block = element_manager_rc
            .insert_new_block(0, InsertMode::AsChild)
            .unwrap();
        let text = element_manager_rc
            .insert_new_text(block.uuid(), InsertMode::AsChild)
            .unwrap();
        element_manager_rc.debug_elements();
        assert_eq!(block.list_all_children(), vec![TextElement(text)]);
    }

    #[test]
    fn set_plain_text() {
        let element_manager_rc = ElementManager::new_rc();
        ElementManager::create_root_frame(element_manager_rc.clone());

        let block = element_manager_rc
            .insert_new_block(0, InsertMode::AsChild)
            .unwrap();
        block.set_plain_text("plain_text");
        element_manager_rc.debug_elements();
        assert_eq!(block.plain_text(), "plain_text");
    }

    #[test]
    fn remove_between_positions() {
        let element_manager_rc = ElementManager::new_rc();
        ElementManager::create_root_frame(element_manager_rc.clone());

        let block = element_manager_rc
            .insert_new_block(0, InsertMode::AsChild)
            .unwrap();
        block.set_plain_text("plain_text");

        let (position, removed_count) = block.remove_between_positions(1, 6).unwrap();

        assert_eq!(removed_count, 5);
        assert_eq!(position, 2);
        assert_eq!(block.plain_text(), "ptext");
    }

    #[test]
    fn remove_between_positions_in_2_texts() {
        let element_manager_rc = ElementManager::new_rc();
        ElementManager::create_root_frame(element_manager_rc.clone());

        let block = element_manager_rc
            .insert_new_block(0, InsertMode::AsChild)
            .unwrap();
        block.set_plain_text("plain_text");

        let new_text_rc = block.insert_new_text_element(block.text_length());
        new_text_rc.set_text(" is life");
        element_manager_rc.debug_elements();

        assert_eq!(block.plain_text(), "plain_text is life");

        let (position, removed_count) = block.remove_between_positions(1, 14).unwrap();

        assert_eq!(removed_count, 13);
        assert_eq!(position, 2);
        assert_eq!(block.plain_text(), "plife");
    }

    #[test]
    fn convert_position_from_block_to_child() {
        let element_manager_rc = ElementManager::new_rc();
        ElementManager::create_root_frame(element_manager_rc.clone());

        let block = element_manager_rc
            .insert_new_block(0, InsertMode::AsChild)
            .unwrap();
        block.set_plain_text("plain_text");

        let new_text_rc = block.insert_new_text_element(block.text_length());
        new_text_rc.set_text(" is life");
        element_manager_rc.debug_elements();

        assert_eq!(block.plain_text(), "plain_text is life");

        assert_eq!(3, block.convert_position_from_block_to_child(3));
        assert_eq!(4, block.convert_position_from_block_to_child(14));
        assert_eq!(8, block.convert_position_from_block_to_child(18));
    }
    #[test]
    fn plain_text_between_positions() {
        let element_manager_rc = ElementManager::new_rc();
        ElementManager::create_root_frame(element_manager_rc.clone());

        let block = element_manager_rc
            .insert_new_block(0, InsertMode::AsChild)
            .unwrap();
        block.set_plain_text("plain_text");

        assert_eq!(block.plain_text_between_positions(0, 1), "p");
        assert_eq!(block.plain_text_between_positions(2, 4), "ai");
        assert_eq!(block.plain_text_between_positions(0, 10), "plain_text");
    }

    #[test]
    fn split() {
        let element_manager_rc = ElementManager::new_rc();
        ElementManager::create_root_frame(element_manager_rc.clone());

        let block = element_manager_rc
            .insert_new_block(0, InsertMode::AsChild)
            .unwrap();
        block.set_plain_text("plain_text");

        let new_block = block.split(2).unwrap();
        element_manager_rc.debug_elements();
        assert_eq!(block.plain_text(), "pl");
        assert_eq!(new_block.plain_text(), "ain_text");

        element_manager_rc.clear();
        let block = element_manager_rc
            .insert_new_block(0, InsertMode::AsChild)
            .unwrap();
        block.set_plain_text("plain_text");

        let new_block = block.split(10).unwrap();
        element_manager_rc.debug_elements();
        assert_eq!(block.plain_text(), "plain_text");
        assert_eq!(new_block.plain_text(), "");
    }

    #[test]
    fn merge_text_elements() {
        let element_manager_rc = ElementManager::new_rc();
        ElementManager::create_root_frame(element_manager_rc.clone());

        let block = element_manager_rc.first_block().unwrap();
        block.set_plain_text("plain_text");

        let first_text_rc = block.iter().next().unwrap().get_text().unwrap();

        let new_text_rc = block.insert_new_text_element(block.text_length());
        new_text_rc.set_text(" is life");
        element_manager_rc.debug_elements();

        assert_eq!(block.plain_text(), "plain_text is life");

        //merge

        block.merge_text_elements(&first_text_rc, &new_text_rc);
        assert_eq!(first_text_rc.plain_text(), "plain_text is life");
        assert_eq!(block.plain_text(), "plain_text is life");
        element_manager_rc.debug_elements();

        //let empty_text_rc = block.insert_new_text_element(block.text_length());
    }
    #[test]
    fn analyze_for_merges_of_text_elements() {
        let element_manager_rc = ElementManager::new_rc();
        ElementManager::create_root_frame(element_manager_rc.clone());

        let block = element_manager_rc.first_block().unwrap();
        block.set_plain_text("plain_text");

        let first_text_rc = block.iter().next().unwrap().get_text().unwrap();
        block.insert_new_text_element(block.text_length());

        let new_text_rc = block.insert_new_text_element(block.text_length());
        new_text_rc.set_text(" is life");
        element_manager_rc.debug_elements();

        block.insert_new_text_element(block.text_length());
        element_manager_rc.debug_elements();

        assert_eq!(block.plain_text(), "plain_text is life");
        assert_eq!(block.iter().count(), 4);

        block.analyze_for_merges();
        assert_eq!(block.iter().count(), 1);
        assert_eq!(first_text_rc.plain_text(), "plain_text is life");
        assert_eq!(block.plain_text(), "plain_text is life");

        //let empty_text_rc = block.insert_new_text_element(block.text_length());
    }

    #[test]
    fn insert_new_text_element() {
        let element_manager_rc = ElementManager::new_rc();
        ElementManager::create_root_frame(element_manager_rc.clone());

        let block = element_manager_rc.first_block().unwrap();
        block.set_plain_text("plain_text");

        block.insert_new_text_element(block.text_length());
        element_manager_rc.debug_elements();

        let new_text_rc = block.insert_new_text_element(block.text_length());
        new_text_rc.set_text(" is life");
        element_manager_rc.debug_elements();

        block.insert_new_text_element(block.text_length());
        element_manager_rc.debug_elements();

        assert_eq!(block.plain_text(), "plain_text is life");
        assert_eq!(block.iter().count(), 4);
    }
}
