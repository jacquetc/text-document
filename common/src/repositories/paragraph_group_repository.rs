use std::collections::HashMap;

use crate::contracts::repositories::{
    ParagraphGroupRepositoryTrait, RepositoryError, RepositoryTrait,
};
use crate::entities::paragraph::Paragraph;
use crate::entities::paragraph_group::ParagraphGroup;

#[derive(Debug, Default)]
pub struct ParagraphGroupRepository {
    paragraph_groups: HashMap<usize, ParagraphGroup>,
}

impl ParagraphGroupRepository {
    pub fn new() -> Self {
        ParagraphGroupRepository {
            paragraph_groups: HashMap::new(),
        }
    }
}

impl ParagraphGroupRepositoryTrait for ParagraphGroupRepository {
    // Add a paragraph to a group until a group have 10 paragraphs, else create a new group. Empty groups are removed.
    fn add_paragraph_to_a_group(&mut self, paragraph: &mut Paragraph) {
        // Remove empty groups
        self.paragraph_groups
            .retain(|_, group| group.paragraph_count > 0);

        let char_count: usize = paragraph.get_char_count();

        // Find a group with less than 10 paragraphs
        if let Some(group) = self.paragraph_groups.iter_mut().find_map(|(_, group)| {
            if group.paragraph_count < 10 {
                Some(group)
            } else {
                None
            }
        }) {
            paragraph.paragraph_group_id = group.id;

            group.paragraph_count += 1;
            group
                .char_count_per_paragraph
                .insert(paragraph.id, char_count);
            group.char_count += char_count;
            group.word_count += paragraph.get_word_count();

            return;
        }

        // Create a new group
        let group = ParagraphGroup {
            id: 0,
            paragraph_count: 1,
            char_count_per_paragraph: [(paragraph.id, char_count)]
                .iter()
                .cloned()
                .collect(),
            char_count,
            word_count: paragraph.get_word_count(),
        };
        let id = self.create(group);

        paragraph.paragraph_group_id = id;
    }

    fn remove_paragraph_from_a_group(&mut self, paragraph: &Paragraph) {
        let group = self
            .paragraph_groups
            .get_mut(&paragraph.paragraph_group_id)
            .unwrap();
        group.paragraph_count -= 1;
        group.char_count_per_paragraph.remove(&paragraph.id);
        group.char_count -= paragraph.get_char_count();
        group.word_count -= paragraph.get_word_count();
    }

    fn update_paragraph_group(&mut self, old_paragraph: &Paragraph, new_paragraph: &Paragraph) {
        let group = self
            .paragraph_groups
            .get_mut(&old_paragraph.paragraph_group_id)
            .unwrap();
        let new_char_count: usize = new_paragraph.get_char_count();
        group
            .char_count_per_paragraph
            .insert(new_paragraph.id, new_char_count);
        group.char_count = group
            .char_count
            .saturating_sub(old_paragraph.get_char_count())
            .saturating_add(new_char_count);
        group.word_count = group
            .word_count
            .saturating_sub(old_paragraph.get_word_count())
            .saturating_add(new_paragraph.get_word_count());
    }
}

impl RepositoryTrait<ParagraphGroup> for ParagraphGroupRepository {
    fn create(&mut self, entity: ParagraphGroup) -> usize {
        let id = self.paragraph_groups.len();
        let mut entity = entity;
        entity.id = id;
        self.paragraph_groups.insert(id, entity);
        id
    }

    fn update(&mut self, entity: ParagraphGroup) -> Result<(), RepositoryError> {
        let id = entity.id;
        if !self.paragraph_groups.contains_key(&id) {
            return Err(RepositoryError::IdNotFound);
        }
        self.paragraph_groups.insert(id, entity);
        Ok(())
    }

    fn get(&self, id: usize) -> Option<&ParagraphGroup> {
        self.paragraph_groups.get(&id)
    }

    fn get_slice(&self, ids: Vec<usize>) -> Vec<&ParagraphGroup> {
        ids.iter()
            .filter_map(|id| self.paragraph_groups.get(id))
            .collect()
    }

    fn get_mut(&mut self, id: usize) -> Option<&mut ParagraphGroup> {
        self.paragraph_groups.get_mut(&id)
    }

    fn remove(&mut self, id: usize) -> Option<ParagraphGroup> {
        self.paragraph_groups.remove(&id)
    }

    fn get_all(&self) -> Vec<&ParagraphGroup> {
        self.paragraph_groups.values().collect()
    }

    fn clear(&mut self) {
        self.paragraph_groups.clear();
    }

    fn is_empty(&self) -> bool {
        self.paragraph_groups.is_empty()
    }

    fn len(&self) -> usize {
        self.paragraph_groups.len()
    }
}
