use crate::block::Block;
use crate::frame::Frame;
use crate::image::Image;
use crate::text::Text;
use crate::text_cursor::TextCursor;
use crate::text_document::Element::{BlockElement, FrameElement, ImageElement, TextElement};
use array_tool::vec::Intersect;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::{Rc, Weak};
use uuid::Uuid;

//#[cfg(test)]
//use std::{println as info, println as warn};

use thiserror::Error;

pub type ElementUuid = usize;

#[derive(PartialEq, Clone)]
pub struct TextDocument {
    //formats: Vec<Format>,
    element_manager: Rc<ElementManager>,
    uuid: Uuid,
}

impl Default for TextDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl TextDocument {
    pub fn new() -> Self {
        let element_manager = ElementManager::new_rc();
        // let mut element_manager = document.element_manager;
        // element_manager.self_weak = document.element_manager

        let document = Self {
            element_manager: element_manager.clone(),
            uuid: Uuid::new_v4(),
        };
        // root frame:
        ElementManager::create_root_frame(element_manager);
        document
    }

    pub fn block_list(&self) -> Vec<Weak<Block>> {
        self.element_manager
            .block_list()
            .into_iter()
            .map(|block| Rc::downgrade(&block))
            .collect()
    }

    pub fn root_frame(&self) -> Weak<Frame> {
        Rc::downgrade(&self.element_manager.root_frame())
    }

    /// Character count, without counting new line character \n
    pub fn character_count(&self) -> usize {
        let mut counter: usize = 0;

        self.element_manager
            .block_list()
            .into_iter()
            .for_each(|block| {
                counter += block.text_length();
            });

        counter
    }

    pub fn find_block(&self, position: usize) -> Option<Weak<Block>> {
        self.element_manager
            .find_block(position)
            .map(|block| Rc::downgrade(&block))
    }

    pub fn first_block(&self) -> Weak<Block> {
        Rc::downgrade(&self.element_manager.first_block().unwrap())
    }

    pub fn last_block(&self) -> Weak<Block> {
        Rc::downgrade(&self.element_manager.last_block().unwrap())
    }

    pub fn block_count(&self) -> usize {
        self.element_manager.block_count()
    }

    pub fn create_cursor(&self) -> TextCursor {
        TextCursor::new(self.element_manager.clone())
    }

    pub fn set_plain_text<S: Into<String>>(&mut self, plain_text: S) -> Result<(), ModelError> {
        let plain_text: String = plain_text.into();

        self.element_manager.clear();

        let frame = self.element_manager.create_empty_root_frame();

        for text in plain_text.split('\n') {
            let block = self
                .element_manager
                .insert_new_block(frame.uuid(), InsertMode::AsChild)?;
            let text_rc = self
                .element_manager
                .insert_new_text(block.uuid(), InsertMode::AsChild)?;
            text_rc.set_text(&text.to_string());
        }

        // signaling changes
        self.element_manager
            .signal_for_text_change(0, 0, plain_text.len());
        self.element_manager.signal_for_element_change(
            self.element_manager.get(0).unwrap(),
            ChangeReason::ChildrenChanged,
        );

        Ok(())
    }

    pub fn to_plain_text(&self) -> String {
        let mut string_list = Vec::new();

        self.element_manager
            .list_all_children(0)
            .iter()
            .filter_map(|element| match element {
                BlockElement(block) => Some(block.plain_text()),
                _ => None,
            })
            .for_each(|string| string_list.push(string));

        string_list.join("\n")
    }

    /// Remove all elements and build a minimal set of element: a Frame, a Block and its empty Text
    pub fn clear(&mut self) -> Result<(), ModelError> {
        self.element_manager.clear();
        let frame = self.element_manager.create_empty_root_frame();
        let block = self
            .element_manager
            .insert_new_block(frame.uuid(), InsertMode::AsChild)?;
        self.element_manager
            .insert_new_text(block.uuid(), InsertMode::AsChild)?;

        Ok(())
    }

    pub fn print_debug_elements(&self) {
        self.element_manager.debug_elements();
    }

    /// Signal the the text change at position, number of removed characters and number of added characters.
    pub fn add_text_change_callback(&self, callback: fn(usize, usize, usize)) {
        self.element_manager.add_text_change_callback(callback);
    }

    ///  Signal the modified element with the reason. If two direct children elements changed at the same time.
    pub fn add_element_change_callback(&self, callback: fn(Element, ChangeReason)) {
        self.element_manager.add_element_change_callback(callback);
    }
}

#[derive(Default, PartialEq, Clone, Debug)]
pub struct TextDocumentOption {
    pub tabs: Vec<Tab>,
    pub text_direction: TextDirection,
    pub wrap_mode: WrapMode,
}

#[derive(Default, PartialEq, Clone, Debug)]
pub struct Tab {
    pub position: usize,
    pub tab_type: TabType,
    pub delimiter: char,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum TabType {
    LeftTab,
    RightTab,
    CenterTab,
    DelimiterTab,
}

impl Default for TabType {
    fn default() -> Self {
        TabType::LeftTab
    }
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum TextDirection {
    LeftToRight,
    RightToLeft,
}

impl Default for TextDirection {
    fn default() -> Self {
        TextDirection::LeftToRight
    }
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum WrapMode {
    NoWrap,
    WordWrap,
    WrapAnywhere,
    WrapAtWordBoundaryOrAnywhere,
}

impl Default for WrapMode {
    fn default() -> Self {
        WrapMode::WordWrap
    }
}

pub(crate) enum InsertMode {
    Before,
    After,
    AsChild,
}

type ElementChangeCallbacks = RefCell<Vec<fn(Element, ChangeReason)>>;
type TextChangeCallbacks = RefCell<Vec<fn(usize, usize, usize)>>;

#[derive(Clone, Debug)]
pub(crate) struct ElementManager {
    self_weak: RefCell<Weak<ElementManager>>,
    text_change_callbacks: TextChangeCallbacks,
    element_change_callbacks: ElementChangeCallbacks,
    tree_model: RefCell<TreeModel>,
}

impl PartialEq for ElementManager {
    fn eq(&self, other: &Self) -> bool {
        self.tree_model == other.tree_model
    }
}

impl ElementManager {
    pub(crate) fn new_rc() -> Rc<Self> {
        let rc = Rc::new(Self {
            tree_model: Default::default(),
            self_weak: RefCell::new(Weak::new()),
            text_change_callbacks: Default::default(),
            element_change_callbacks: Default::default(),
        });
        let new_self_weak = RefCell::new(Rc::downgrade(&rc));
        rc.self_weak.swap(&new_self_weak);
        rc
    }

    // only used while creating a new document
    pub(crate) fn create_root_frame(element_manager: Rc<ElementManager>) -> Rc<Frame> {
        let new_frame = Rc::new(Frame::new(Rc::downgrade(&element_manager)));

        let new_element = Element::FrameElement(new_frame.clone());

        let mut tree_model = element_manager.tree_model.borrow_mut();
        tree_model.set_root_element(new_element);

        // create a first empty block

        let new_block = Rc::new(Block::new(Rc::downgrade(&element_manager)));

        let new_block_element = Element::BlockElement(new_block);

        let block_uuid = tree_model.insert_as_child(0, new_block_element).unwrap();

        // create a first empty text element

        let new_text = Rc::new(Text::new(Rc::downgrade(&element_manager)));

        let new_text_element = Element::TextElement(new_text);

        tree_model
            .insert_as_child(block_uuid, new_text_element)
            .unwrap();

        tree_model.recalculate_sort_order();

        new_frame
    }

    fn create_empty_root_frame(&self) -> Rc<Frame> {
        let new_frame = Rc::new(Frame::new(self.self_weak.borrow().clone()));

        let new_element = Element::FrameElement(new_frame.clone());

        self.tree_model.borrow_mut().set_root_element(new_element);
        self.tree_model.borrow_mut().recalculate_sort_order();

        new_frame
    }

    pub(crate) fn insert_new_frame(
        &self,
        target_uuid: usize,
        insert_mode: InsertMode,
    ) -> Result<Rc<Frame>, ModelError> {
        let new_frame = Rc::new(Frame::new(self.self_weak.borrow().clone()));

        let new_element = Element::FrameElement(new_frame.clone());

        self.insert(new_element.clone(), target_uuid, insert_mode)?;
        // verify:
        let parent_element = match self.get_parent_element(&new_element) {
            Some(element) => element,
            None => return Err(ModelError::ElementNotFound("No parent found".to_string())),
        };
        new_frame.verify_rule_with_parent(&parent_element)?;

        self.tree_model.borrow_mut().recalculate_sort_order();

        Ok(new_frame)
    }

    pub(crate) fn insert_new_block(
        &self,
        target_uuid: usize,
        insert_mode: InsertMode,
    ) -> Result<Rc<Block>, ModelError> {
        let new_block = Rc::new(Block::new(self.self_weak.borrow().clone()));

        let new_element = Element::BlockElement(new_block.clone());

        self.insert(new_element.clone(), target_uuid, insert_mode)?;
        // verify:
        let parent_element = match self.get_parent_element(&new_element) {
            Some(element) => element,
            None => return Err(ModelError::ElementNotFound("No parent found".to_string())),
        };
        new_block.verify_rule_with_parent(&parent_element)?;

        self.tree_model.borrow_mut().recalculate_sort_order();

        Ok(new_block)
    }

    pub(crate) fn insert_new_text(
        &self,
        target_uuid: usize,
        insert_mode: InsertMode,
    ) -> Result<Rc<Text>, ModelError> {
        let new_text = Rc::new(Text::new(self.self_weak.borrow().clone()));

        let new_element = Element::TextElement(new_text.clone());

        self.insert(new_element.clone(), target_uuid, insert_mode)?;
        // verify:
        let parent_element = match self.get_parent_element(&new_element) {
            Some(element) => element,
            None => return Err(ModelError::ElementNotFound("No parent found".to_string())),
        };
        new_text.verify_rule_with_parent(&parent_element)?;
        self.tree_model.borrow_mut().recalculate_sort_order();

        Ok(new_text)
    }

    pub(crate) fn insert_new_image(
        &self,
        target_uuid: usize,
        insert_mode: InsertMode,
    ) -> Result<Rc<Image>, ModelError> {
        let new_image = Rc::new(Image::new(self.self_weak.borrow().clone()));

        let new_element = Element::ImageElement(new_image.clone());

        self.insert(new_element.clone(), target_uuid, insert_mode)?;
        // verify:
        let parent_element = match self.get_parent_element(&new_element) {
            Some(element) => element,
            None => return Err(ModelError::ElementNotFound("No parent found".to_string())),
        };
        new_image.verify_rule_with_parent(&parent_element)?;
        self.tree_model.borrow_mut().recalculate_sort_order();

        Ok(new_image)
    }

    pub(crate) fn insert(
        &self,
        element: Element,
        target_uuid: usize,
        insert_mode: InsertMode,
    ) -> Result<usize, ModelError> {
        let mut tree_model = self.tree_model.borrow_mut();

        match insert_mode {
            InsertMode::Before => tree_model.insert_before(target_uuid, element),
            InsertMode::After => tree_model.insert_after(target_uuid, element),
            InsertMode::AsChild => tree_model.insert_as_child(target_uuid, element),
        }
    }

    // remove a list of element's uuids. Ignore errors.
    pub(crate) fn remove(&self, uuid_list: Vec<usize>) {
        if uuid_list.contains(&0) {
            self.clear();
        } else {
            let mut tree_model = self.tree_model.borrow_mut();
            uuid_list.iter().for_each(|uuid| {
                tree_model.remove_recursively(*uuid).unwrap_or_default();
            });
        }
    }

    /// Give a count of the blocks
    pub(crate) fn block_count(&self) -> usize {
        let mut counter = 0;
        let tree_model = self.tree_model.borrow();
        tree_model.iter().for_each(|element| {
            counter += match element {
                BlockElement(_) => 1,
                _ => 0,
            }
        });
        counter
    }

    pub(crate) fn block_list(&self) -> Vec<Rc<Block>> {
        let tree_model = self.tree_model.borrow();

        tree_model
            .iter()
            .filter_map(|x| match x {
                BlockElement(block) => Some(block.clone()),
                _ => None,
            })
            .collect()
    }

    /// get the common ancestor, typacally a frame. At worst, ancestor is 0, meaning the root frame
    pub(crate) fn find_common_ancestor(
        &self,
        first_element_uuid: usize,
        second_element_uuid: usize,
    ) -> ElementUuid {
        let tree_model = self.tree_model.borrow();

        tree_model.find_common_ancestor(first_element_uuid, second_element_uuid)
    }

    /// get the common ancestor, typacally a frame. At worst, ancestor is 0, meaning the root frame
    pub(crate) fn find_ancestor_of_first_which_is_sibling_of_second(
        &self,
        first_element_uuid: ElementUuid,
        second_element_uuid: ElementUuid,
    ) -> Option<ElementUuid> {
        let tree_model = self.tree_model.borrow();

        tree_model.find_ancestor_of_first_which_is_sibling_of_second(
            first_element_uuid,
            second_element_uuid,
        )
    }

    pub(crate) fn root_frame(&self) -> Rc<Frame> {
        let tree_model = self.tree_model.borrow();
        let element = tree_model.get_root_element().unwrap();

        if let Element::FrameElement(c) = element {
            c.clone()
        } else {
            unreachable!()
        }
    }

    pub(crate) fn find_block(&self, position: usize) -> Option<Rc<Block>> {
        for rc_block in self.block_list() {
            if (rc_block.position()..=rc_block.end()).contains(&position) {
                return Some(rc_block);
            }
        }

        None
    }

    pub(crate) fn get_parent_frame(&self, element: &Element) -> Option<Rc<Frame>> {
        let child_uuid = self.get_element_uuid(element);

        let tree_model = self.tree_model.borrow();
        let parent_uuid = tree_model.get_parent_uuid(child_uuid)?;

        let parent_element = tree_model.get(parent_uuid)?;

        match parent_element {
            FrameElement(frame_rc) => Some(frame_rc.clone()),
            BlockElement(_) => None,
            TextElement(_) => None,
            ImageElement(_) => None,
        }
    }

    pub(crate) fn get_parent_element(&self, element: &Element) -> Option<Element> {
        let child_uuid = self.get_element_uuid(element);

        self.get_parent_element_using_uuid(child_uuid)
    }

    pub(crate) fn get_parent_element_using_uuid(&self, uuid: ElementUuid) -> Option<Element> {
        let tree_model = self.tree_model.borrow();
        let parent_uuid = tree_model.get_parent_uuid(uuid)?;

        tree_model.get(parent_uuid).cloned()
    }

    /// Get uuid of the element
    pub(crate) fn get_element_uuid(&self, element: &Element) -> usize {
        match element {
            FrameElement(frame_rc) => frame_rc.uuid(),
            BlockElement(block_rc) => block_rc.uuid(),
            TextElement(text_rc) => text_rc.uuid(),
            ImageElement(image_rc) => image_rc.uuid(),
        }
    }

    pub(crate) fn get_level(&self, uuid: usize) -> usize {
        let tree_model = self.tree_model.borrow();
        tree_model.get_level(uuid)
    }
    pub(crate) fn recalculate_sort_order(&self) {
        let mut tree_model = self.tree_model.borrow_mut();
        tree_model.recalculate_sort_order();
    }

    pub(crate) fn previous_element(&self, uuid: usize) -> Option<Element> {
        let tree_model = self.tree_model.borrow();
        tree_model
            .iter()
            .take_while(|element| element.uuid() != uuid)
            .last()
            .cloned()
    }

    pub(crate) fn next_element(&self, uuid: usize) -> Option<Element> {
        let tree_model = self.tree_model.borrow();
        tree_model
            .iter()
            .skip_while(|element| element.uuid() != uuid)
            .nth(1)
            .cloned()
    }

    /// Get element sort order
    pub(crate) fn get_element_order(&self, element: Element) -> Option<usize> {
        let tree_model = self.tree_model.borrow();
        let target_uuid = self.get_element_uuid(&element);

        tree_model.get_sort_order(target_uuid)
    }

    // Give element using uuid
    pub(crate) fn get(&self, uuid: usize) -> Option<Element> {
        let tree_model = self.tree_model.borrow();
        tree_model.get(uuid).cloned()
    }

    pub(crate) fn find_frame(&self, position: usize) -> Option<Rc<Frame>> {
        let block = self
            .block_list()
            .into_iter()
            .find(|rc_block| (rc_block.position()..rc_block.end()).contains(&position));

        match block {
            Some(block_rc) => self.get_parent_frame(&BlockElement(block_rc)),
            None => None,
        }
    }

    pub(crate) fn last_block(&self) -> Option<Rc<Block>> {
        self.block_list().last().cloned()
    }

    pub(crate) fn first_block(&self) -> Option<Rc<Block>> {
        self.block_list().first().cloned()
    }

    /// list recursively all elements having uuid as their common ancestor
    pub(crate) fn list_all_children(&self, uuid: usize) -> Vec<Element> {
        let tree_model = self.tree_model.borrow();
        let children = tree_model.list_all_children(uuid);

        children
            .iter()
            .filter_map(|element_uuid| self.get(*element_uuid))
            .collect()
    }

    /// list only the direct children elements having uuid as their common ancestor
    pub(crate) fn list_all_direct_children(&self, uuid: usize) -> Vec<Element> {
        let tree_model = self.tree_model.borrow();
        let children = tree_model.list_all_direct_children(uuid);

        children
            .iter()
            .filter_map(|element_uuid| self.get(*element_uuid))
            .collect()
    }

    /// remove all elements and recreate a combo frame/block/text
    pub(crate) fn clear(&self) {
        {
            let mut tree_model = self.tree_model.borrow_mut();
            tree_model.clear();
        }

        ElementManager::create_root_frame(self.self_weak.borrow().upgrade().unwrap());
    }

    pub(crate) fn fill_empty_frames(&self) {
        // find empty frames
        let tree_model = self.tree_model.borrow();
        let empty_frames: Vec<ElementUuid> = tree_model
            .iter()
            .filter_map(|element| match element {
                FrameElement(frame) => {
                    if !tree_model.has_children(frame.uuid()) {
                        Some(frame.uuid())
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect();

        // fill these frames
        for frame_uuid in empty_frames {
            let block = self
                .insert_new_block(frame_uuid, InsertMode::AsChild)
                .unwrap();
            self.insert_new_text(block.uuid(), InsertMode::AsChild)
                .unwrap();
        }
    }

    pub(crate) fn debug_elements(&self) {
        let mut indent_with_string = vec![(0, 0, 0, "------------\n".to_string())];

        println!("debug_elements");
        let tree_model = self.tree_model.borrow();

        tree_model.iter().for_each(|element| {
            match element {
                FrameElement(frame) => indent_with_string.push((
                    tree_model.get_level(frame.uuid()),
                    frame.uuid(),
                    tree_model.get_sort_order(frame.uuid()).unwrap(),
                    "frame".to_string(),
                )),
                BlockElement(block) => indent_with_string.push((
                    tree_model.get_level(block.uuid()),
                    block.uuid(),
                    tree_model.get_sort_order(block.uuid()).unwrap(),
                    "block".to_string(),
                )),
                TextElement(text) => indent_with_string.push((
                    tree_model.get_level(text.uuid()),
                    text.uuid(),
                    tree_model.get_sort_order(text.uuid()).unwrap(),
                    "text".to_string(),
                )),
                ImageElement(image) => indent_with_string.push((
                    tree_model.get_level(image.uuid()),
                    image.uuid(),
                    tree_model.get_sort_order(image.uuid()).unwrap(),
                    "[image]".to_string(),
                )),
            };
        });

        indent_with_string
            .iter()
            .for_each(|(indent, uuid, sort_order, string)| {
                println!(
                    "{}{} {} \'{}\'",
                    " ".repeat(*indent),
                    *uuid,
                    *sort_order,
                    string.as_str()
                )
            });
        indent_with_string.clear();

        tree_model.iter().for_each(|element| {
            match element {
                FrameElement(frame) => indent_with_string.push((
                    tree_model.get_level(frame.uuid()),
                    frame.uuid(),
                    tree_model.get_sort_order(frame.uuid()).unwrap(),
                    "frame".to_string(),
                )),
                BlockElement(block) => indent_with_string.push((
                    tree_model.get_level(block.uuid()),
                    block.uuid(),
                    tree_model.get_sort_order(block.uuid()).unwrap(),
                    "block ".to_string() + &block.plain_text(),
                )),
                TextElement(text) => indent_with_string.push((
                    tree_model.get_level(text.uuid()),
                    text.uuid(),
                    tree_model.get_sort_order(text.uuid()).unwrap(),
                    "text ".to_string() + &text.plain_text(),
                )),
                ImageElement(image) => indent_with_string.push((
                    tree_model.get_level(image.uuid()),
                    image.uuid(),
                    tree_model.get_sort_order(image.uuid()).unwrap(),
                    "[image] ".to_string() + &image.plain_text(),
                )),
            };
        });

        indent_with_string
            .iter()
            .for_each(|(indent, uuid, sort_order, string)| {
                println!(
                    "{}{} {} \'{}\'",
                    " ".repeat(*indent),
                    *uuid,
                    *sort_order,
                    string.as_str()
                )
            });
    }

    /// Signal the number of removed characters and number of added characters with the reference of a cursor position.
    pub(crate) fn signal_for_text_change(
        &self,
        position: usize,
        removed_characters: usize,
        added_character: usize,
    ) {
        self.text_change_callbacks
            .borrow()
            .iter()
            .for_each(|callback| callback(position, removed_characters, added_character));
    }

    /// Add callback for when there is a change, giving the number of removed characters and number of added characters with the reference of a cursor position.
    pub(self) fn add_text_change_callback(&self, callback: fn(usize, usize, usize)) {
        self.text_change_callbacks.borrow_mut().push(callback);
    }

    /// Signal for when an element (and/or more than one child Blocks) is modified. If only one Block is modified, only an element Block is sent.
    pub(crate) fn signal_for_element_change(&self, changed_element: Element, reason: ChangeReason) {
        self.element_change_callbacks
            .borrow()
            .iter()
            .for_each(|callback| callback(changed_element.clone(), reason));
    }

    /// Add callback for when an element (and/or more than one child Blocks) is modified. If only one Block is modified, only an element Block is sent.
    pub(self) fn add_element_change_callback(&self, callback: fn(Element, ChangeReason)) {
        self.element_change_callbacks.borrow_mut().push(callback);
    }

    pub(crate) fn move_while_changing_parent(
        &self,
        uuid_to_move: usize,
        new_parent_uuid: usize,
    ) -> Result<(), ModelError> {
        let mut tree_model = self.tree_model.borrow_mut();
        tree_model.move_while_changing_parent(uuid_to_move, new_parent_uuid)
    }
}

#[derive(Default, PartialEq, Clone, Debug)]
struct TreeModel {
    id_with_element_hash: HashMap<usize, Element>,
    order_with_id_map: BTreeMap<usize, usize>,
    child_id_with_parent_id_hash: HashMap<usize, usize>,
    id_counter: usize,
}

impl TreeModel {
    const STEP: usize = 1000;

    pub(crate) fn new() -> Self {
        Self {
            id_with_element_hash: Default::default(),
            order_with_id_map: Default::default(),
            child_id_with_parent_id_hash: Default::default(),
            id_counter: Default::default(),
        }
    }
    // to be called after an operation
    pub(crate) fn recalculate_sort_order(&mut self) {
        let mut new_order = 0;

        let mut new_map: BTreeMap<usize, usize> = BTreeMap::new();

        for (_order, id) in self.order_with_id_map.iter() {
            new_map.insert(new_order, *id);
            new_order += Self::STEP;
        }

        self.order_with_id_map = new_map;
    }

    fn iter(&self) -> TreeIter {
        TreeIter::new(self)
    }

    fn get_new_uuid(&mut self) -> usize {
        self.id_counter += 1;
        self.id_counter
    }

    fn number_of_ancestors(&self, child_id: &usize) -> usize {
        let mut number_of_ancestors: usize = 0;
        let mut loop_child_id = child_id;

        loop {
            match self.child_id_with_parent_id_hash.get(loop_child_id) {
                Some(parent) => match parent {
                    0 => {
                        number_of_ancestors += 1;
                        break;
                    }

                    _ => {
                        number_of_ancestors += 1;
                        loop_child_id = parent;
                    }
                },
                None => unreachable!(),
            }
        }
        number_of_ancestors
    }

    pub(self) fn set_root_element(&mut self, element: Element) {
        self.clear();

        self.id_with_element_hash.insert(0, element);
        self.order_with_id_map.insert(0, 0);
        self.child_id_with_parent_id_hash.insert(0, 0);

        self.recalculate_sort_order();
    }

    pub(self) fn insert_after(
        &mut self,
        sibling_uuid: usize,
        mut element: Element,
    ) -> Result<usize, ModelError> {
        if sibling_uuid == self.get_root_element().unwrap().uuid() {
            return Err(ModelError::ForbiddenOperation(
                "can't add by root element".to_string(),
            ));
        }
        if self.get_parent_uuid(sibling_uuid).is_none() {
            return Err(ModelError::ElementNotFound("no parent element".to_string()));
        }
        let parent_uuid = match self.get_parent_uuid(sibling_uuid) {
            Some(parent_uuid) => parent_uuid,
            None => unreachable!(),
        };

        // determine safe sort order

        let safe_sort_order = match self.get_next_sibling(sibling_uuid) {
            Some(next_sibling_id) => match self.get_sort_order(next_sibling_id) {
                Some(sort_order) => sort_order - 1,
                None => unreachable!(),
            },
            // get next parent element or one of the grand parent
            None => {
                let parent_level = self.get_level(parent_uuid);
                let next_items: Vec<(&usize, &usize)> = self
                    .order_with_id_map
                    .iter()
                    // dismiss previous items
                    .skip_while(|(&_order, &id)| parent_uuid != id)
                    .skip(1)
                    .skip_while(|(&_order, &id)| self.get_level(id) > parent_level)
                    .collect();
                match next_items.first() {
                    Some(item) => {
                        if *item.0 == 0 {
                            usize::MAX - Self::STEP
                        } else {
                            item.0 - 1
                        }
                    }
                    // extreme bottom of the tree
                    None => usize::MAX - Self::STEP,
                }
            }
        };

        let new_uuid = self.get_new_uuid();
        element.set_uuid(new_uuid);

        self.id_with_element_hash.insert(new_uuid, element);
        self.order_with_id_map.insert(safe_sort_order, new_uuid);
        self.child_id_with_parent_id_hash
            .insert(new_uuid, parent_uuid);

        self.recalculate_sort_order();
        Ok(new_uuid)
    }

    pub(self) fn insert_before(
        &mut self,
        sibling_uuid: usize,
        mut element: Element,
    ) -> Result<usize, ModelError> {
        if sibling_uuid == self.get_root_element().unwrap().uuid() {
            return Err(ModelError::ForbiddenOperation(
                "can't add by root element".to_string(),
            ));
        }

        if self.get_parent_uuid(sibling_uuid).is_none() {
            return Err(ModelError::ElementNotFound("no parent element".to_string()));
        }

        let parent_uuid = match self.get_parent_uuid(sibling_uuid) {
            Some(parent_uuid) => parent_uuid,
            None => unreachable!(),
        };

        let safe_sort_order = match self.get_sort_order(sibling_uuid) {
            Some(sort_order) => sort_order - 1,
            None => unreachable!(),
        };

        let new_uuid = self.get_new_uuid();
        element.set_uuid(new_uuid);

        self.id_with_element_hash.insert(new_uuid, element);
        self.order_with_id_map.insert(safe_sort_order, new_uuid);
        self.child_id_with_parent_id_hash
            .insert(new_uuid, parent_uuid);

        self.recalculate_sort_order();
        Ok(new_uuid)
    }

    /// insert add child of parent uuid, returns uuid of new element
    pub(self) fn insert_as_child(
        &mut self,
        parent_uuid: usize,
        mut element: Element,
    ) -> Result<usize, ModelError> {
        // determine safe sort order

        let safe_sort_order = match self.get_next_sibling(parent_uuid) {
            Some(next_sibling_id) => match self.get_sort_order(next_sibling_id) {
                Some(sort_order) => sort_order - 1,
                None => unreachable!(),
            },
            // get next element
            None => {
                let parent_level = self.get_level(parent_uuid);
                let next_items: Vec<(&usize, &usize)> = self
                    .order_with_id_map
                    .iter()
                    .skip_while(|(_order, id)| parent_uuid != **id)
                    .skip_while(|(_order, id)| self.get_level(**id) >= parent_level)
                    .collect();
                match next_items.first() {
                    Some(item) => item.0 - 1,
                    // extreme bottom of the tree
                    None => usize::MAX - Self::STEP,
                }
            }
        };

        let new_uuid = self.get_new_uuid();
        element.set_uuid(new_uuid);

        self.id_with_element_hash.insert(new_uuid, element);
        self.order_with_id_map.insert(safe_sort_order, new_uuid);
        self.child_id_with_parent_id_hash
            .insert(new_uuid, parent_uuid);

        self.recalculate_sort_order();
        Ok(new_uuid)
    }

    fn get_next_sibling(&self, uuid: usize) -> Option<usize> {
        let parent_uuid = self.get_parent_uuid(uuid)?;

        let siblings: Vec<&usize> = self
            .child_id_with_parent_id_hash
            .iter()
            .filter_map(|(child_id, parent_id)| {
                if *parent_id == parent_uuid && uuid != *child_id && *child_id != 0 {
                    Some(child_id)
                } else {
                    None
                }
            })
            .collect();

        if siblings.is_empty() {
            return None;
        }

        let next_sibling = self
            .order_with_id_map
            .iter()
            .skip_while(|(_order, id)| *id != &uuid)
            .skip(1)
            .find(|(_order, id)| siblings.contains(&id))?;

        Some(next_sibling.1.to_owned())
    }

    // pub(self) fn swap(&mut self, uuid: ElementUuid, mut element: Element) {
    //     unimplemented!()
    // }

    pub(self) fn remove_recursively(
        &mut self,
        uuid: ElementUuid,
    ) -> Result<Vec<ElementUuid>, ModelError> {
        let mut uuids_to_remove = self.list_all_children(uuid);
        uuids_to_remove.push(uuid);

        for element_uuid in &uuids_to_remove {
            self.remove(*element_uuid)?;
        }

        Ok(uuids_to_remove)
    }

    fn remove(&mut self, uuid: ElementUuid) -> Result<ElementUuid, ModelError> {
        let id = self
            .order_with_id_map
            .remove_entry(
                &self
                    .get_sort_order(uuid)
                    .ok_or_else(|| ModelError::ElementNotFound(uuid.to_string()))?,
            )
            .ok_or_else(|| ModelError::ElementNotFound(uuid.to_string()))?
            .1;

        self.child_id_with_parent_id_hash
            .remove_entry(&uuid)
            .ok_or_else(|| ModelError::ElementNotFound(uuid.to_string()))?;

        self.id_with_element_hash
            .remove_entry(&uuid)
            .ok_or_else(|| ModelError::ElementNotFound(uuid.to_string()))?;

        Ok(id)
    }

    pub(self) fn list_all_children(&self, uuid: usize) -> Vec<usize> {
        let element_level = self.get_level(uuid);

        self.iter()
            .skip_while(|element| element.uuid() != uuid)
            // skip current element
            .skip(1)
            // keep only children
            .take_while(|element| element_level < self.get_level(element.uuid()))
            .map(|element| element.uuid())
            .collect()
    }

    pub(self) fn list_all_direct_children(&self, uuid: usize) -> Vec<usize> {
        let unordered_child_list: Vec<usize> = self
            .child_id_with_parent_id_hash
            .iter()
            .filter_map(|(child_id, parent_id)| {
                if *parent_id == uuid && *child_id != 0 {
                    Some(*child_id)
                } else {
                    None
                }
            })
            .collect();

        let ordered_child_list: Vec<usize> = self
            .order_with_id_map
            .iter()
            .filter_map(|(_order, id)| {
                if unordered_child_list.contains(id) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        ordered_child_list
    }

    fn get_parent_uuid(&self, uuid: usize) -> Option<usize> {
        // exception for root
        if uuid == 0 {
            return None;
        }

        self.child_id_with_parent_id_hash.get(&uuid).copied()
    }

    fn get_level(&self, uuid: usize) -> usize {
        let mut child_id = uuid;
        let mut level = 0;

        while let Some(&parent_id) = self.child_id_with_parent_id_hash.get(&child_id) {
            if child_id == 0 {
                break;
            }

            child_id = parent_id;
            level += 1;
        }

        level
    }

    /// get the common ancestor, typacally a frame. At worst, ancestor is 0, meaning the root frame
    pub(self) fn find_ancestor_of_first_which_is_sibling_of_second(
        &self,
        first_element_uuid: ElementUuid,
        second_element_uuid: ElementUuid,
    ) -> Option<ElementUuid> {
        let mut ancestors_of_first_element: Vec<usize> = Vec::new();

        let mut child_id = first_element_uuid;

        // find ancestors for first
        while let Some(&parent_id) = self.child_id_with_parent_id_hash.get(&child_id) {
            if child_id == 0 {
                break;
            }
            ancestors_of_first_element.push(parent_id);

            child_id = parent_id;
        }

        // compare and get the ancestor

        let second_element_all_siblings = self.get_all_siblings(second_element_uuid);

        let sibling = ancestors_of_first_element.intersect(second_element_all_siblings);

        sibling.first().copied()
    }

    pub(self) fn get_all_siblings(&self, uuid: ElementUuid) -> Vec<ElementUuid> {
        let parent_uuid = match self.get_parent_uuid(uuid) {
            Some(parent_uuid) => parent_uuid,
            None => return Vec::new(),
        };

        self.child_id_with_parent_id_hash
            .iter()
            .filter_map(|(child_id, parent_id)| {
                match *parent_id == parent_uuid && *child_id != uuid {
                    true => Some(*child_id),
                    false => None,
                }
            })
            .collect()
    }

    /// get the common ancestor, typacally a frame. At worst, ancestor is 0, meaning the root frame
    pub(self) fn find_common_ancestor(
        &self,
        first_element_uuid: ElementUuid,
        second_element_uuid: ElementUuid,
    ) -> ElementUuid {
        let mut ancestors_of_first_element: Vec<usize> = Vec::new();

        let mut child_id = first_element_uuid;

        // find ancestors for first
        while let Some(&parent_id) = self.child_id_with_parent_id_hash.get(&child_id) {
            if child_id == 0 {
                break;
            }
            ancestors_of_first_element.push(parent_id);

            child_id = parent_id;
        }

        // find ancestors for second
        let mut ancestors_of_second_element: Vec<usize> = Vec::new();
        child_id = second_element_uuid;

        while let Some(&parent_id) = self.child_id_with_parent_id_hash.get(&child_id) {
            if child_id == 0 {
                break;
            }
            ancestors_of_second_element.push(parent_id);

            child_id = parent_id;
        }

        // compare and get the ancestor

        let common_ancestors = ancestors_of_first_element.intersect(ancestors_of_second_element);

        *common_ancestors.first().unwrap()
    }

    /// set a new parent and change order so the element is directly under the new parent. Careful, the new child isn't moved at the end of the list of children !
    pub(self) fn move_while_changing_parent(
        &mut self,
        uuid_to_move: usize,
        new_parent_uuid: usize,
    ) -> Result<(), ModelError> {
        // change parent
        self.child_id_with_parent_id_hash
            .iter_mut()
            .find_map(|(child_id, parent_id)| {
                if *child_id == uuid_to_move {
                    *parent_id = new_parent_uuid;
                    Some(parent_id)
                } else {
                    None
                }
            });

        // change order

        let old_order = *self
            .order_with_id_map
            .iter()
            .find(|(&_order, &iter_uuid)| iter_uuid == uuid_to_move)
            .ok_or_else(|| ModelError::ElementNotFound("parent not found".to_string()))?
            .0;

        let new_order = self
            .order_with_id_map
            .iter()
            .find(|(&_order, &iter_uuid)| iter_uuid == new_parent_uuid)
            .ok_or_else(|| ModelError::ElementNotFound("parent not found".to_string()))?
            .0
            + 1;

        self.order_with_id_map.remove(&old_order);

        self.order_with_id_map.insert(new_order, uuid_to_move);

        self.recalculate_sort_order();

        Ok(())
    }

    pub(self) fn get(&self, uuid: usize) -> Option<&Element> {
        self.id_with_element_hash.get(&uuid)
    }

    pub(self) fn get_root_element(&self) -> Option<&Element> {
        self.id_with_element_hash.get(&0)
    }

    fn get_sort_order(&self, uuid: usize) -> Option<usize> {
        self.order_with_id_map
            .iter()
            .find(|(&_order, &iter_uuid)| iter_uuid == uuid)
            .map(|pair| *pair.0)
    }

    pub(crate) fn clear(&mut self) {
        self.child_id_with_parent_id_hash.clear();
        self.order_with_id_map.clear();
        self.id_with_element_hash.clear();
        self.id_counter = 0;
    }

    pub(crate) fn has_children(&self, uuid: ElementUuid) -> bool {
        let level = self.get_level(uuid);

        match self
            .order_with_id_map
            .iter()
            .skip_while(|(&_order, &iter_uuid)| iter_uuid != uuid)
            .nth(1)
        {
            Some((_order, id)) => level < self.get_level(*id),
            None => false,
        }
    }
}

struct TreeIter<'a> {
    unvisited: Vec<&'a Element>,
}

impl<'a> TreeIter<'a> {
    fn new(tree: &'a TreeModel) -> Self {
        let ordered_elements = tree
            .order_with_id_map
            .iter()
            .rev()
            .map(|(_order, id)| -> &Element { tree.id_with_element_hash.get(id).unwrap() })
            .collect();

        TreeIter {
            unvisited: ordered_elements,
        }
    }
}

impl<'a> Iterator for TreeIter<'a> {
    type Item = &'a Element;

    fn next(&mut self) -> Option<Self::Item> {
        let element = self.unvisited.pop()?;

        Some(element)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Element {
    FrameElement(Rc<Frame>),
    BlockElement(Rc<Block>),
    TextElement(Rc<Text>),
    ImageElement(Rc<Image>),
}

impl Element {
    pub(crate) fn set_uuid(&mut self, uuid: usize) {
        match self {
            Element::FrameElement(rc_frame) => rc_frame.set_uuid(uuid),
            Element::BlockElement(rc_block) => rc_block.set_uuid(uuid),
            Element::TextElement(rc_text) => rc_text.set_uuid(uuid),
            Element::ImageElement(rc_image) => rc_image.set_uuid(uuid),
        }
    }
    pub fn uuid(&self) -> usize {
        match self {
            Element::FrameElement(rc_frame) => rc_frame.uuid(),
            Element::BlockElement(rc_block) => rc_block.uuid(),
            Element::TextElement(rc_text) => rc_text.uuid(),
            Element::ImageElement(rc_image) => rc_image.uuid(),
        }
    }
    pub fn text_length(&self) -> usize {
        match self {
            Element::FrameElement(rc_frame) => rc_frame.text_length(),
            Element::BlockElement(rc_block) => rc_block.text_length(),
            Element::TextElement(rc_text) => rc_text.text_length(),
            Element::ImageElement(rc_image) => rc_image.text_length(),
        }
    }
    pub fn end_of_element(&self) -> usize {
        match self {
            Element::FrameElement(rc_frame) => rc_frame.end(),
            Element::BlockElement(rc_block) => rc_block.end(),
            Element::TextElement(rc_text) => rc_text.end(),
            Element::ImageElement(rc_image) => rc_image.end(),
        }
    }

    pub fn start_of_element(&self) -> usize {
        match self {
            Element::FrameElement(rc_frame) => rc_frame.start(),
            Element::BlockElement(rc_block) => rc_block.start(),
            Element::TextElement(rc_text) => rc_text.start(),
            Element::ImageElement(rc_image) => rc_image.start(),
        }
    }

    pub fn is_block(&self) -> bool {
        matches!(self, Element::BlockElement(_))
    }
    pub fn get_block(&self) -> Option<Rc<Block>> {
        match self {
            Element::BlockElement(block) => Some(block.clone()),
            _ => None,
        }
    }

    pub fn is_frame(&self) -> bool {
        matches!(self, Element::FrameElement(_))
    }
    pub fn get_frame(&self) -> Option<Rc<Frame>> {
        match self {
            Element::FrameElement(frame) => Some(frame.clone()),
            _ => None,
        }
    }

    pub fn is_text(&self) -> bool {
        matches!(self, Element::TextElement(_))
    }
    pub fn get_text(&self) -> Option<Rc<Text>> {
        match self {
            Element::TextElement(text) => Some(text.clone()),
            _ => None,
        }
    }

    pub fn is_image(&self) -> bool {
        matches!(self, Element::ImageElement(_))
    }
    pub fn get_image(&self) -> Option<Rc<Image>> {
        match self {
            Element::ImageElement(image) => Some(image.clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests_element {
    use std::rc::{Rc, Weak};

    use crate::Block;
    use crate::Element::BlockElement;

    #[test]
    fn get() {
        let element = BlockElement(Rc::new(Block::new(Weak::new())));
        assert!(element.is_block());
        assert!(!element.is_image());
        assert!(!element.is_frame());
        assert!(!element.is_text());
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChangeReason {
    // when format of a element change
    FormatChanged,
    // when content like text change
    ContentChanged,
    // internal value change, like the element UUID
    InternalStructureChanged,
    // when more than one child element change, is removed or is added, a sort of reset is asked because of too many changes in the children
    ChildrenChanged,
}

pub(crate) trait ElementTrait {
    fn set_uuid(&self, uuid: usize);
    fn verify_rule_with_parent(&self, parent_element: &Element) -> Result<(), ModelError>;
}

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("no element found with this id: `{0}`")]
    ElementNotFound(String),
    #[error("forbidden operation: `{0}`")]
    ForbiddenOperation(String),
    #[error("Outside text limits in an element")]
    OutsideElementBounds,
    #[error("wrong parent")]
    WrongParent,
    #[error("unknown error")]
    Unknown,
}

#[cfg(test)]
mod tree_model_tests {
    use super::*;

    #[test]
    fn ancestors() {
        let element_manager_rc = ElementManager::new_rc();
        ElementManager::create_root_frame(element_manager_rc.clone());

        let block = element_manager_rc
            .insert_new_block(0, InsertMode::AsChild)
            .unwrap();

        let tree_model = element_manager_rc.tree_model.borrow();

        assert_eq!(tree_model.number_of_ancestors(&block.uuid()), 1);
    }
}

#[cfg(test)]
mod document_tests {
    use super::*;

    #[test]
    fn list_all_children() {
        let document = TextDocument::new();
        document.print_debug_elements();

        let children = document.element_manager.list_all_children(0);

        assert_eq!(children.len(), 2);

        let children = document.element_manager.list_all_children(1);
        assert_eq!(children.len(), 1);
    }

    #[test]
    fn list_all_direct_children() {
        let document = TextDocument::new();
        document.print_debug_elements();

        let children = document.element_manager.list_all_direct_children(0);

        assert_eq!(children.len(), 1);

        let children = document.element_manager.list_all_children(1);
        assert_eq!(children.len(), 1);

        document
            .element_manager
            .insert_new_block(0, InsertMode::AsChild)
            .expect("Insertion failed");

        document
            .element_manager
            .insert_new_block(0, InsertMode::AsChild)
            .expect("Insertion failed");

        document
            .element_manager
            .insert_new_block(0, InsertMode::AsChild)
            .expect("Insertion failed");

        document.print_debug_elements();

        let children = document.element_manager.list_all_direct_children(0);

        assert_eq!(children.len(), 4);
    }

    #[test]
    fn insert_new_block_as_child() {
        let document = TextDocument::new();
        document.print_debug_elements();

        // insert at the end of the tree
        document
            .element_manager
            .insert_new_block(0, InsertMode::AsChild)
            .expect("Insertion failed");
        document.print_debug_elements();
        assert_eq!(document.last_block().upgrade().unwrap().uuid(), 3);

        let children = document.element_manager.list_all_children(0);
        assert_eq!(children.len(), 3);
    }

    #[test]
    fn find_block() {
        let mut document = TextDocument::new();
        document
            .set_plain_text("plain_text\nsecond\nthird")
            .unwrap();
        document.print_debug_elements();

        let block = document.find_block(10).unwrap();
        assert_eq!(block.upgrade().unwrap().uuid(), 1);
        let block = document.find_block(11).unwrap();
        assert_eq!(block.upgrade().unwrap().uuid(), 3);
        let block = document.find_block(23).unwrap();
        assert_eq!(block.upgrade().unwrap().uuid(), 5);
    }

    #[test]
    fn insert_new_block_before() {
        let document = TextDocument::new();
        document.print_debug_elements();

        // insert at the end of the tree
        document
            .element_manager
            .insert_new_block(1, InsertMode::Before)
            .expect("Insertion failed");
        document.print_debug_elements();
        assert_eq!(document.last_block().upgrade().unwrap().uuid(), 1);

        let children = document.element_manager.list_all_children(0);
        assert_eq!(children.len(), 3);
    }

    #[test]
    fn insert_new_block_after() {
        let document = TextDocument::new();
        document.print_debug_elements();

        // insert at the end of the tree
        document
            .element_manager
            .insert_new_block(1, InsertMode::After)
            .expect("Insertion failed");
        document.print_debug_elements();
        assert_eq!(document.last_block().upgrade().unwrap().uuid(), 3);

        let children = document.element_manager.list_all_children(0);
        assert_eq!(children.len(), 3);

        // insert when one next sibling already exists
        document
            .element_manager
            .insert_new_block(1, InsertMode::After)
            .expect("Insertion failed");
        document.print_debug_elements();
        assert_eq!(document.last_block().upgrade().unwrap().uuid(), 3);

        let children = document.element_manager.list_all_children(0);
        assert_eq!(children.len(), 4);
    }

    #[test]
    fn insert_new_block_after_in_frame() {
        let document = TextDocument::new();
        document.print_debug_elements();

        // insert at the end of the tree
        document
            .element_manager
            .insert_new_block(1, InsertMode::After)
            .expect("Insertion failed");
        document.print_debug_elements();
        assert_eq!(document.last_block().upgrade().unwrap().uuid(), 3);

        let frame = document
            .element_manager
            .insert_new_frame(0, InsertMode::AsChild)
            .unwrap();

        document
            .element_manager
            .insert_new_block(frame.uuid(), InsertMode::After)
            .unwrap();

        // insert in frame

        let block_in_frame = document
            .element_manager
            .insert_new_block(frame.uuid(), InsertMode::AsChild)
            .unwrap();

        // insert in frame
        document.print_debug_elements();

        let _second_block_in_frame = document
            .element_manager
            .insert_new_block(block_in_frame.uuid(), InsertMode::After)
            .unwrap();

        document.print_debug_elements();

        let children = document.element_manager.list_all_children(0);
        assert_eq!(children.len(), 7);
    }
}
