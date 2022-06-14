use crate::format::{BlockFormat, CharFormat, ImageFormat};
use crate::text_document::ElementManager;
use std::borrow::Borrow;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::{Rc, Weak};

#[derive(Clone)]
pub struct Block {
    uuid: usize,
    uuid_counter: Cell<usize>,
    element_manager: Weak<ElementManager>,
    id_with_fragment_hash: RefCell<HashMap<usize, BlockFragment>>,
    order_with_id_map: RefCell<BTreeMap<usize, usize>>,
    /// Describes block-specific properties
    block_format: RefCell<BlockFormat>,
}

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
            && self.id_with_fragment_hash == other.id_with_fragment_hash
            && self.order_with_id_map == other.order_with_id_map
            && self.block_format == other.block_format
            && self.uuid_counter == other.uuid_counter
    }
}

impl Block {
    pub(crate) fn new(uuid: usize, element_manager: Weak<ElementManager>) -> Self {
        
        // create first empty text fragment:
        
        let mut hash = HashMap::new();
        hash.insert(0, Text::create_fragment(0));
        let mut map = BTreeMap::new();
        map.insert(0, 0);
        
        Block {
            uuid,
            element_manager,
            uuid_counter: Cell::new(1),
            id_with_fragment_hash: RefCell::new(hash),
            order_with_id_map: RefCell::new(map),
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
        self.uuid
    }

    // position of the end of the block in the context of the document
    pub fn end_position(&self) -> usize {
        self.position() + self.length()
    }

    /// Length of text in the block
    pub fn length(&self) -> usize {
        let mut counter: usize = 0;

        for fragment in self.id_with_fragment_hash.borrow().values() {
            counter += match fragment {
                BlockFragment::TextFragment(text) => text.text.borrow().len(),
                BlockFragment::ImageFragment(_) => 1,
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

    pub(crate) fn char_format_at(&self, position_in_block: usize) -> Option<CharFormat> {
        if position_in_block == 0 {
            match self.first_fragment() {
                Some(fragment) => match fragment {
                    BlockFragment::TextFragment(text_fragment) => {
                        Some(text_fragment.char_format().clone())
                    }
                    BlockFragment::ImageFragment(_) => return None,
                },
                None => return None,
            }
        } else {
            None
        }
    }

    fn first_fragment(&self) -> Option<BlockFragment> {
        let map = self.order_with_id_map.borrow();
        let first_uuid = match map.values().min() {
            Some(minimum_sort_order) => match map.get(minimum_sort_order) {
                Some(first_uuid) => first_uuid,
                None => return None,
            },
            None => return None,
        };

        let hash = self.id_with_fragment_hash.borrow();
        match hash.get(first_uuid) {
            Some(fragment) => Some(fragment.clone()),
            None => None,
        }

        
    }

    fn find_fragment(&self, position_in_block: usize) -> Option<(BlockFragment, usize)> {
        let mut position = 0;

        for fragment in self.ordered_fragments() {

            let fragment_end_position = match &fragment {
                BlockFragment::TextFragment( text_rc) => text_rc.len(),
                BlockFragment::ImageFragment(image_rc) => image_rc.len(),
            };

            if (position..fragment_end_position).contains(&position_in_block) {
                return Some((fragment, position_in_block - position));
            }

            position += fragment_end_position;
        }

        None
    }

    pub(crate) fn insert_plain_text(&self, plain_text: &str, position_in_block: usize, char_format: &CharFormat) {
        match self.find_fragment(position_in_block) {
            Some((fragment, position_in_fragment)) => match fragment {
                BlockFragment::TextFragment(text_rc) => {
                    text_rc.text.borrow_mut().insert_str(position_in_fragment, plain_text);
                    text_rc.set_char_format(char_format);
                },
                BlockFragment::ImageFragment(_) => todo!(),
            },
            None => return,
        }

    }

    pub(crate) fn set_plain_text(&self, plain_text: &str, char_format: &CharFormat) {
        self.clear();
        self.insert_plain_text(plain_text, 0, char_format);
    }

    fn clear(&self) {
        self.id_with_fragment_hash.replace(HashMap::new());
        self.order_with_id_map.replace(BTreeMap::new());
    }

    fn ordered_fragments(&self) -> Vec<BlockFragment> {
        let map = self.order_with_id_map.borrow_mut();
        let hash = self.id_with_fragment_hash.borrow();
        map.values()
            .filter_map(|uuid| hash.get(uuid)).map(| fragment| fragment.clone())
            .collect()
    }

    /// Describes the block's character format. The block's character format is the char format of the first block.
    pub fn char_format(&self) -> CharFormat {
        match self.first_fragment().unwrap() {
            BlockFragment::TextFragment(text_fragment) => text_fragment.char_format().clone(),
            BlockFragment::ImageFragment(_) => CharFormat::new(),
        }
    }

    /// Apply a new vhar format on all text fragments of this block
    pub(crate) fn set_char_format(&self, char_format: &CharFormat) {
        self.ordered_fragments()
            .iter()
            .filter_map(|fragment| match fragment {
                BlockFragment::TextFragment(text) => Some(text.clone()),
                BlockFragment::ImageFragment(_) => None,
            })
            .for_each(|text_fragment: Rc<Text>| {
                text_fragment.set_char_format(&char_format);
            });
    }

    fn analyse_for_merges(&self) {
        todo!()
    }

    fn merge_text_fragments(
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
    pub fn plain_text(&self) ->String {
        let texts: Vec<String> = self.ordered_fragments()
            .iter()
            .map(|fragment|  { match fragment {
                BlockFragment::TextFragment(text_rc) => text_rc.text.borrow().clone(),
                BlockFragment::ImageFragment(image_rc) => image_rc.text.borrow().clone(),
            }}).collect();
            texts.join("")

    }

}

trait Fragment {
    fn create_fragment(uuid: usize) -> BlockFragment;
    fn len(&self) -> usize;
    fn uuid(&self) -> usize;
}

#[derive(Default, Clone, PartialEq)]
pub(crate) struct Text {
    uuid: usize,
    pub(self) text: RefCell<String>,
    char_format: RefCell<CharFormat>,
}

impl Text {
    pub(crate) fn new(uuid: usize) -> Self {
        Text {
            uuid,
            ..Default::default()
        }
    }

    pub(crate) fn char_format(&self) -> CharFormat {
        self.char_format.borrow().clone()
    }

    pub(crate) fn set_char_format(&self, char_format: &CharFormat) {
        self.char_format.replace(char_format.clone());
    }


}

impl Fragment for Text {
    fn create_fragment(uuid: usize) -> BlockFragment {
        BlockFragment::TextFragment(Rc::new(Text::new(uuid)))
    }

    fn len(&self) -> usize {
        self.text.borrow().len()
    }

    fn uuid(&self) -> usize{
            self.uuid
    }
}

#[derive(Default, Clone, PartialEq)]
pub(crate) struct Image {
    uuid: usize,
    pub(self) text: RefCell<String>,
    image_format: RefCell<ImageFormat>,
}

impl Image {
    pub(crate) fn new(uuid: usize) -> Self {
        Self {
            uuid,
            ..Default::default()
        }
    }
}

impl Fragment for Image {
    fn create_fragment(uuid: usize) -> BlockFragment {
        BlockFragment::ImageFragment(Rc::new(Image::new(uuid)))
    }


    fn len(&self) -> usize {
        1
    }

    fn uuid(&self) -> usize{
            self.uuid
    }

}

#[derive(Clone, PartialEq)]
enum BlockFragment {
    TextFragment(Rc<Text>),
    ImageFragment(Rc<Image>),
}
