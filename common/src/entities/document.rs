use std::default;


#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Node {
    Section(Box<Section>),
    Paragraph { paragraph_id: usize},
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
    pub content: Vec<usize>,
}


#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct ListItem {
    pub paragraph_id: usize,
}


#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct Chunk {
    pub id: usize,
    pub nodes: Vec<Node>,
}


// Define the root of the AST
#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct Document {
    pub chunks: Vec<Chunk>,
}

impl Document {
    pub fn new() -> Self {
        Document { chunks: Vec::new() }
    }
}
