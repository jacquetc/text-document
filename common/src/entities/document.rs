use im_rc::Vector;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Node {
    Section(Box<Section>),
    Paragraph { paragraph_id: usize },
    List(Vector<ListItem>),
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
    pub nodes: Vector<Node>,
}

impl Section {
    pub fn new() -> Self {
        Section {
            nodes: Vector::new(),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct ListItem {
    pub paragraph_id: usize,
    pub indent_level: usize,
}

// Define the root of the AST
#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct Document {
    pub nodes: Vector<Node>,
}

impl Document {
    pub fn new() -> Self {
        Document {
            nodes: Vector::new(),
        }
    }
}
