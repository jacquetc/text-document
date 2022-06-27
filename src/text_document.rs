use crate::block::Block;
use crate::format::CharFormat;
use crate::image::Image;
use crate::text::Text;
use crate::text_cursor::TextCursor;
use crate::text_document::Element::{BlockElement, FrameElement, ImageElement, TextElement};
use crate::{format::Format, frame::Frame};
use std::borrow::BorrowMut;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::mem;
use std::ops::{Add, Index};
use std::rc::{Rc, Weak};
use uuid::Uuid;
use array_tool::vec::Intersect;

#[cfg(test)]
use std::{println as info, println as warn};

use thiserror::Error;


type ElementUuid = usize;

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

#[derive(PartialEq, Clone)]
pub struct TextDocument {
    //formats: Vec<Format>,
    element_manager: Rc<ElementManager>,
    uuid: Uuid,
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

    pub fn character_count(&self) -> usize {
        let mut counter: usize = 0;

        self.element_manager
            .block_list()
            .into_iter()
            .for_each(|block| {
                counter += block.length();
            });

        counter
    }

    pub fn find_block(&self, position: usize) -> Option<Weak<Block>> {
        match self.element_manager.find_block(position) {
            Some(block) => Some(Rc::downgrade(&block)),
            None => None,
        }
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
        self.clear();

        let frame = self.element_manager.create_empty_root_frame();

        Ok(for text in plain_text.into().split("\n") {
            let block = self
                .element_manager
                .insert_new_block(frame.uuid(), InsertMode::AsChild)?;
            let text_rc = self
                .element_manager
                .insert_new_text(block.uuid(), InsertMode::AsChild)?;
            text_rc.set_text(&text.to_string());
        })
    }

    pub fn clear(&mut self) {
        self.element_manager.clear();
    }

    pub fn print_debug_elements(&self) {
        self.element_manager.debug_elements();
    }

    pub fn add_cursor_change_callback(&self, callback: fn(usize, usize, usize)) {
        self.element_manager.add_cursor_change_callback(callback);
    }

    pub fn add_element_change_callback(&self, callback: fn(Element, ChangeReason)) {
        self.element_manager.add_element_change_callback(callback);
    }
}

#[derive(Default, PartialEq, Clone)]
pub struct TextDocumentOption {
    pub tabs: Vec<Tab>,
    pub text_direction: TextDirection,
    pub wrap_mode: WrapMode,
}

#[derive(Default, PartialEq, Clone)]
pub struct Tab {
    pub position: usize,
    pub tab_type: TabType,
    pub delimiter: char,
}

#[derive(PartialEq, Clone, Copy)]
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

#[derive(PartialEq, Clone, Copy)]
pub enum TextDirection {
    LeftToRight,
    RightToLeft,
}

impl Default for TextDirection {
    fn default() -> Self {
        TextDirection::LeftToRight
    }
}

#[derive(PartialEq, Clone, Copy)]
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

#[derive(Clone)]
pub(crate) struct ElementManager {
    self_weak: RefCell<Weak<ElementManager>>,
    cursor_change_callbacks: RefCell<Vec<fn(usize, usize, usize)>>,
    element_change_callbacks: RefCell<Vec<fn(Element, ChangeReason)>>,
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
            cursor_change_callbacks: Default::default(),
            element_change_callbacks: Default::default(),
        });
        let new_self_weak = RefCell::new(Rc::downgrade(&rc));
        rc.self_weak.swap(&new_self_weak);
        rc
    }

    /// Create and insert a new frame after a block of a frame, as a child of this frame.
    pub(crate) fn insert_frame_using_position(
        &self,
        position: usize,
    ) -> Result<Rc<Frame>, ModelError> {
        // find reference block
        let block_rc = self
            .find_block(position)
            .unwrap_or(self.last_block().unwrap());

        let block_uuid = block_rc.uuid();

        // determine new order
        let new_order = self
            .get_element_order(BlockElement(block_rc.clone()))
            .unwrap_or(1);

        // find reference block's parent id
        let parent_frame = self
            .get_parent_frame(&BlockElement(block_rc))
            .unwrap_or(self.root_frame());
        let parent_uuid = parent_frame.uuid();

        // create frame
        let new_frame = self.insert_new_frame(block_uuid, InsertMode::After);

        todo!("wrong");
        new_frame
    }

    // only used while creating a new document
    fn create_root_frame(element_manager: Rc<ElementManager>) -> Rc<Frame> {
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

        tree_model.insert_as_child(block_uuid, new_text_element);

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

    pub(crate) fn insert(
        &self,
        element: Element,
        target_uuid: usize,
        insert_mode: InsertMode,
    ) -> Result<usize, ModelError> {
        let mut tree_model = self.tree_model.borrow_mut();

        let new_uuid = match insert_mode {
            InsertMode::Before => tree_model.insert_before(target_uuid, element),
            InsertMode::After => tree_model.insert_after(target_uuid, element),
            InsertMode::AsChild => tree_model.insert_as_child(target_uuid, element),
        };

        new_uuid
    }

    // remove a list of element's uuids. Ignore errors.
    pub(crate) fn remove(&self, uuid_list: Vec<usize>) {

            if uuid_list.contains(&0) {
                self.clear();
            }
            else {

        let mut tree_model = self.tree_model.borrow_mut();
        uuid_list
            .iter()
            .for_each(|uuid| -> () {tree_model.remove_recursively(*uuid).unwrap_or_default();} );

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
    pub(crate) fn find_common_ancestor(&self, first_element_uuid: usize, second_element_uuid: usize) -> ElementUuid {
        let tree_model = self.tree_model.borrow();
        
        tree_model.find_common_ancestor( first_element_uuid, second_element_uuid) 
 
    }
    
    /// get the common ancestor, typacally a frame. At worst, ancestor is 0, meaning the root frame
    pub(crate) fn find_ancestor_of_first_which_is_sibling_of_second(&self, first_element_uuid: ElementUuid, second_element_uuid: ElementUuid) -> Option<ElementUuid>{
        let tree_model = self.tree_model.borrow();
        
        tree_model.find_ancestor_of_first_which_is_sibling_of_second( first_element_uuid, second_element_uuid) 
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
            if (rc_block.position()..rc_block.end_position()).contains(&position) {
                return Some(rc_block);
            }
        }

        None
    }

    pub(crate) fn get_parent_frame(&self, element: &Element) -> Option<Rc<Frame>> {
        let child_uuid = self.get_element_uuid(&element);

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
        let child_uuid = self.get_element_uuid(&element);

        let tree_model = self.tree_model.borrow();
        let parent_uuid = tree_model.get_parent_uuid(child_uuid)?;

        match tree_model.get(parent_uuid) {
            Some(element) => Some(element.clone()),
            None => None,
        }
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

    pub(crate) fn next_element(&self, uuid: usize) -> Option<Element> {
        let tree_model = self.tree_model.borrow();
        match tree_model
            .iter()
            .skip_while(|element| element.uuid() != uuid)
            .skip(1)
            .next()
        {
            Some(element) => Some(element.clone()),
            None => None,
        }
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
        match tree_model.get(uuid) {
            Some(element) => Some(element.clone()),
            None => None,
        }
    }

    pub(crate) fn find_frame(&self, position: usize) -> Option<Rc<Frame>> {
        let block = self
            .block_list()
            .into_iter()
            .find(|rc_block| (rc_block.position()..rc_block.end_position()).contains(&position));

        match block {
            Some(block_rc) => self.get_parent_frame(&BlockElement(block_rc)),
            None => None,
        }
    }

    pub(crate) fn last_block(&self) -> Option<Rc<Block>> {
        match self.block_list().last() {
            Some(last) => Some(last.clone()),
            None => None,
        }
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

    /// remove all elements and recreate a combo frame/block/text
    pub(crate) fn clear(&self) {
        {
            let mut tree_model = self.tree_model.borrow_mut();
            tree_model.clear();
        }

        ElementManager::create_root_frame(self.self_weak.borrow().upgrade().unwrap());
    }

    pub(crate) fn fill_empty_frames(&self) {

        // find empy frames
        let mut tree_model = self.tree_model.borrow();
        let empty_frames: Vec<ElementUuid> = tree_model.iter()
        .filter_map(| element | match element {
            FrameElement(frame) => if !tree_model.has_children(frame.uuid()) {
                Some(frame.uuid())
            }
            else{
                None
            } ,
            _ => None,
        } ).collect();

        // fill these frames
        for frame_uuid in empty_frames {

            let block = self.insert_new_block(frame_uuid, InsertMode::AsChild).unwrap();
            self.insert_new_text(block.uuid(), InsertMode::AsChild);
        }

    }

    pub(self) fn debug_elements(&self) {
        let mut indent_with_string = vec![(0, "------------\n".to_string())];

        println!("debug_elements");
        let tree_model = self.tree_model.borrow();

        tree_model.iter().for_each(|element| {
            match element {
                FrameElement(frame) => indent_with_string
                    .push((tree_model.get_level(frame.uuid()), "frame".to_string())),
                BlockElement(block) => indent_with_string.push((
                    tree_model.get_level(block.uuid()),
                    "block ".to_string() + &block.plain_text(),
                )),
                Element::TextElement(text) => indent_with_string.push((
                    tree_model.get_level(text.uuid()),
                    "text ".to_string() + &text.plain_text().to_string(),
                )),
                Element::ImageElement(image) => indent_with_string
                    .push((tree_model.get_level(image.uuid()), "[image]".to_string())),
            };
        });

        indent_with_string
            .iter()
            .for_each(|(indent, string)| println!("{}{}", " ".repeat(*indent), string.as_str()));
    }

    pub(self) fn signal_for_cursor_change(
        &self,
        position: usize,
        removed_characters: usize,
        added_character: usize,
    ) {
        self.cursor_change_callbacks
            .borrow()
            .iter()
            .for_each(|callback| callback(position, removed_characters, added_character));
    }

    pub(self) fn add_cursor_change_callback(&self, callback: fn(usize, usize, usize)) {
        self.cursor_change_callbacks.borrow_mut().push(callback);
    }

    /// Signal for when a Frame (and/or more than one child Blocks) is modified. If only one Block is modified, only an element Block is sent.
    pub(self) fn signal_for_element_change(&self, changed_element: Element, reason: ChangeReason) {
        self.element_change_callbacks
            .borrow()
            .iter()
            .for_each(|callback| callback(changed_element.clone(), reason));
    }

    /// Add callback for when a Frame (and/or more than one child Blocks) is modified. If only one Block is modified, only an element Block is sent.
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

#[derive(Default, PartialEq, Clone)]
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
            .find(|(_order, id)| siblings.contains(&id))?;

        Some(next_sibling.1.to_owned())
    }

    pub(self) fn swap(&mut self, uuid: usize, mut element: Element) {}

    pub(self) fn remove_recursively(&mut self, uuid: usize) -> Result<Vec<usize>, ModelError> {
        todo!()
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
    pub(self) fn find_ancestor_of_first_which_is_sibling_of_second(&self, first_element_uuid: ElementUuid, second_element_uuid: ElementUuid) -> Option<ElementUuid>{

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

        match sibling.first() {
            Some(sib) => Some(sib.clone()),
            None => None,
        }
    }


    pub(self) fn get_all_siblings(&self, uuid: ElementUuid) -> Vec<ElementUuid> {


        let parent_uuid = match self.get_parent_uuid(uuid) {
            Some(parent_uuid) => parent_uuid,
            None => return Vec::new(),
        };

        self.child_id_with_parent_id_hash.iter().filter_map(| (child_id, parent_id) | match *parent_id == parent_uuid && *child_id != uuid {
            true => Some(*child_id),
            false => None,
        }
    ).collect()
    }

    /// get the common ancestor, typacally a frame. At worst, ancestor is 0, meaning the root frame
    pub(self) fn find_common_ancestor(&self, first_element_uuid: ElementUuid, second_element_uuid: ElementUuid) -> ElementUuid{

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

        common_ancestors.first().unwrap().clone()
        
    }


    /// set a new parent and change order so the element is under the new parent
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
            .ok_or(ModelError::ElementNotFound("parent not found".to_string()))?
            .0;

        let new_order = self
            .order_with_id_map
            .iter()
            .find(|(&_order, &iter_uuid)| iter_uuid == new_parent_uuid)
            .ok_or(ModelError::ElementNotFound("parent not found".to_string()))?
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
        match self
            .order_with_id_map
            .iter()
            .find(|(&order, &iter_uuid)| iter_uuid == uuid)
        {
            Some(pair) => Some(*pair.0),
            None => None,
        }
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
            .skip_while(|(&order, &iter_uuid)| iter_uuid != uuid).skip(1).next() {
                Some((order, id)) => level < self.get_level(*id),
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
            .map(|(order, id)| -> &Element { tree.id_with_element_hash.get(id).unwrap() })
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

        Some(&element)
    }
}

#[derive(Clone, PartialEq)]
pub enum Element {
    FrameElement(Rc<Frame>),
    BlockElement(Rc<Block>),
    TextElement(Rc<Text>),
    ImageElement(Rc<Image>),
}

impl Element {
    pub fn set_uuid(&mut self, uuid: usize) {
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
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChangeReason {
    // when format of a element change
    FormatChanged,
    // when content like text change
    ContentChanged,
    // when more than one child element change, is removed or is added
    StructureChanged,
}

pub(crate) trait ElementTrait {
    fn uuid(&self) -> usize;
    fn set_uuid(&self, uuid: usize);
    fn verify_rule_with_parent(&self, parent_element: &Element) -> Result<(), ModelError>;
}

#[cfg(test)]
mod tests {
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
}
