#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct ParagraphGroup {
    pub id: usize,
    pub paragraph_count: usize, 
    pub char_count: usize,
    pub word_count: usize,
}
