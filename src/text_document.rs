use crate::block::Block;
use crate::format::CharFormat;
use crate::frame::Element;
use crate::frame::Element::{BlockElement, FrameElement};
use crate::text_cursor::TextCursor;
use crate::{format::Format, frame::Frame};
use anyhow::Result;
use std::borrow::Borrow;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::{Rc, Weak};
use uuid::Uuid;

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
        Rc::downgrade(&self.element_manager.last_block())
    }

    pub fn block_count(&self) -> usize {
        self.element_manager.block_count()
    }

    pub fn create_cursor(&self) -> TextCursor {
        TextCursor::new(self.element_manager.clone())
    }

    pub fn set_plain_text<S: Into<String>>(&mut self, plain_text: S) {
        self.clear();
        self.element_manager
            .insert_plain_text(plain_text, 0, CharFormat::new());
    }

    pub fn clear(&mut self) {
        self.element_manager.clear();
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

#[derive(Clone)]
pub(crate) struct ElementManager {
    id_with_element_hash: RefCell<HashMap<usize, Element>>,
    order_with_id_map: RefCell<BTreeMap<usize, usize>>,
    child_id_with_parent_id_hash: RefCell<HashMap<usize, usize>>,
    id_counter: Cell<usize>,
    self_weak: RefCell<Weak<ElementManager>>,
}

impl PartialEq for ElementManager {
    fn eq(&self, other: &Self) -> bool {
        self.id_with_element_hash == other.id_with_element_hash
            && self.order_with_id_map == other.order_with_id_map
            && self.child_id_with_parent_id_hash == other.child_id_with_parent_id_hash
            && self.id_counter == other.id_counter
    }
}

impl ElementManager {
    pub(crate) fn new_rc() -> Rc<Self> {
        let rc = Rc::new(Self {
            id_with_element_hash: Default::default(),
            order_with_id_map: Default::default(),
            child_id_with_parent_id_hash: Default::default(),
            id_counter: Default::default(),
            self_weak: RefCell::new(Weak::new()),
        });
        let new_self_weak = RefCell::new(Rc::downgrade(&rc));
        rc.self_weak.swap(&new_self_weak);
        rc
    }

    pub(self) fn create_root_frame(element_manager: Rc<Self>) -> Rc<Frame> {
        ElementManager::create_frame_staticaly(element_manager, 0, 0)
    }

    /// Create and insert a new frame after a block of a frame, as a child of this frame.
    pub(crate) fn insert_frame_using_position(&self, position: usize) -> Rc<Frame> {
        // find reference block
        let block_rc = self.find_block(position).unwrap_or(self.last_block());

        // determine new order
        let new_order = self
            .get_element_order(BlockElement(block_rc.clone()))
            .unwrap_or(1);

        // find reference block's parent id
        let parent_frame = self
            .get_parent_frame(BlockElement(block_rc))
            .unwrap_or(self.root_frame());
        let parent_uuid = parent_frame.uuid();

        // create frame
        let new_frame = self.create_frame(new_order, parent_uuid);

        new_frame
    }

    fn create_frame_staticaly(
        element_manager: Rc<ElementManager>,
        sort_order: usize,
        parent_uuid: usize,
    ) -> Rc<Frame> {
        let new_uuid = element_manager.get_new_uuid();

        let new_frame = Rc::new(Frame::new(new_uuid, Rc::downgrade(&element_manager)));

        let new_element = Element::FrameElement(new_frame.clone());

        element_manager
            .id_with_element_hash
            .borrow_mut()
            .insert(new_uuid, new_element);
        element_manager
            .order_with_id_map
            .borrow_mut()
            .insert(sort_order + 1, new_uuid);
        element_manager
            .child_id_with_parent_id_hash
            .borrow_mut()
            .insert(new_uuid, parent_uuid);

        // create a first empty block
        ElementManager::create_block_staticaly(element_manager.clone(), sort_order + 2, 0);

        new_frame
    }

    fn create_frame(&self, sort_order: usize, parent_uuid: usize) -> Rc<Frame> {
        ElementManager::create_frame_staticaly(
            self.self_weak.borrow().upgrade().unwrap(),
            sort_order,
            parent_uuid,
        )
    }

    fn get_new_uuid(&self) -> usize {
        self.id_counter.set(self.id_counter.get() + 1);
        self.id_counter.get()
    }

    // split block at position, like if a new line is inserted
    pub(crate) fn insert_block_using_position(&self, position: usize) -> Rc<Block> {
        // find reference block
        let old_block_rc = self.find_block(position).unwrap_or(self.last_block());

        // determine new order
        let new_order = self
            .get_element_order(BlockElement(old_block_rc.clone()))
            .unwrap_or(1);

        let parent_frame = self
            .get_parent_frame(BlockElement(old_block_rc))
            .unwrap_or(self.root_frame());
        let parent_uuid = parent_frame.uuid();

        // create block
        let new_block = self.create_block(new_order, parent_uuid);

        // split and move fragments from one block to another

        new_block
    }
    /// returns the new cursor position
    pub(crate) fn insert_plain_text<S: Into<String>>(
        &self,
        plain_text: S,
        position: usize,
        char_format: CharFormat,
    ) -> usize {
        let mut first_loop = true;
        let mut new_position = position;

        let mut block = self.find_block(new_position).unwrap_or(self.last_block());
        for text in plain_text.into().split("\n") {
            if first_loop {
                block.insert_plain_text(
                    text,
                    block.convert_position_from_document(new_position),
                    &char_format,
                );

                first_loop = false;
            } else {
                block = self
                    .self_weak
                    .borrow()
                    .upgrade()
                    .unwrap()
                    .insert_block_using_position(new_position);
                block.set_plain_text(text, &char_format);
                new_position += 1;
            }

            new_position += text.len();
        }

        new_position
    }

    // only needed when TextDocument isn't yet initialized
    fn create_block_staticaly(
        element_manager: Rc<ElementManager>,
        sort_order: usize,
        parent_uuid: usize,
    ) -> Rc<Block> {
        let new_uuid = element_manager.get_new_uuid();
        let new_block = Rc::new(Block::new(new_uuid, Rc::downgrade(&element_manager)));

        let new_element = Element::BlockElement(new_block.clone());

        element_manager
            .id_with_element_hash
            .borrow_mut()
            .insert(new_uuid, new_element);
        element_manager
            .order_with_id_map
            .borrow_mut()
            .insert(sort_order, new_uuid);
        element_manager
            .child_id_with_parent_id_hash
            .borrow_mut()
            .insert(new_uuid, parent_uuid);

        new_block
    }

    fn create_block(&self, sort_order: usize, parent_uuid: usize) -> Rc<Block> {
        ElementManager::create_block_staticaly(
            self.self_weak.borrow().upgrade().unwrap(),
            sort_order,
            parent_uuid,
        )
    }

    pub(crate) fn block_count(&self) -> usize {
        let mut counter = 0;
        self.id_with_element_hash
            .borrow()
            .values()
            .for_each(|element| {
                counter += match element {
                    BlockElement(_) => 1,
                    _ => 0,
                }
            });
        counter
    }

    pub(crate) fn block_list(&self) -> Vec<Rc<Block>> {
        self.id_with_element_hash
            .borrow()
            .values()
            .into_iter()
            .filter_map(|x| match x {
                BlockElement(block) => Some(block.clone()),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn root_frame(&self) -> Rc<Frame> {
        let frame_option;
        {
            let id_with_element_hash = self.id_with_element_hash.borrow();

            frame_option = id_with_element_hash.get(&1);
        }
        let element = match frame_option {
            Some(element) => element.clone(),
            None => {
                let mut mut_hash = self.id_with_element_hash.borrow_mut();
                mut_hash
                    .insert(
                        1,
                        FrameElement(ElementManager::create_root_frame(
                            self.self_weak.borrow().upgrade().unwrap(),
                        )),
                    )
                    .unwrap()
                    .clone()
            }
        };

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

    fn get_parent_frame(&self, element: Element) -> Option<Rc<Frame>> {
        let child_uuid = match element {
            FrameElement(frame_rc) => frame_rc.uuid(),
            BlockElement(block_rc) => block_rc.uuid(),
        };

        let hash = self.child_id_with_parent_id_hash.borrow();
        let parent_uuid = match hash.get(&child_uuid) {
            Some(uuid) => uuid,
            None => return None,
        };

        let hash = self.id_with_element_hash.borrow();
        let parent_element = match hash.get(&parent_uuid) {
            Some(element) => element,
            None => return None,
        };

        match parent_element {
            FrameElement(frame_rc) => Some(frame_rc.clone()),
            BlockElement(_) => None,
        }
    }

    fn get_element_order(&self, element: Element) -> Option<usize> {
        let target_uuid = match element {
            FrameElement(frame_rc) => frame_rc.uuid(),
            BlockElement(block_rc) => block_rc.uuid(),
        };

        match self
            .order_with_id_map
            .borrow()
            .iter()
            .find(|(&order, &uuid)| uuid == target_uuid)
        {
            Some(pair) => Some(*pair.0),
            None => None,
        }
    }

    // to be called after an operation
    fn recalculate_sort_order(element_manager: Rc<ElementManager>) {
        todo!()
    }

    pub(crate) fn find_frame(&self, position: usize) -> Option<Rc<Frame>> {
        let block = self
            .block_list()
            .into_iter()
            .find(|rc_block| (rc_block.position()..rc_block.end_position()).contains(&position));

        match block {
            Some(block_rc) => self.get_parent_frame(BlockElement(block_rc)),
            None => None,
        }
    }

    pub fn last_block(&self) -> Rc<Block> {
        match self.block_list().last() {
            Some(last) => last.clone(),
            None => self.create_block(usize::MAX - 1000, 0),
        }
    }

    pub(crate) fn clear(&self) {
        self.child_id_with_parent_id_hash.borrow_mut().clear();
        self.order_with_id_map.borrow_mut().clear();
        self.id_with_element_hash.borrow_mut().clear();
        self.id_counter.set(0);
        ElementManager::create_root_frame(self.self_weak.borrow().upgrade().unwrap());
    }
}
