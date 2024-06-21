use std::default;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Node {
    Section(Box<Section>),
    Paragraph { paragraph_id: usize },
    List(Vec<ListItem>),
    // ... other types of nodes
}

impl Default for Node {
    fn default() -> Self {
        Node::Paragraph { paragraph_id: 0 }
    }
}

// Define a section with a content
#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct Section {
    pub content: Vec<Node>,
}

#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct ListItem {
    pub paragraph_id: usize,
}

// Define the root of the AST
#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct Document {
    pub nodes: Vec<Node>,
}

impl Document {
    pub fn new() -> Self {
        Document { nodes: Vec::new() }
    }
}
