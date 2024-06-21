use std::collections::HashMap;

// Only used for counts, so no need to store the actual paragraphs. No notion of order, so no need to store that either.
#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct ParagraphGroup {
    pub id: usize,
    pub paragraph_count: usize,
    pub char_count_per_paragraph: HashMap<usize, usize>,
    pub char_count: usize,
    pub word_count: usize,
}
