#[derive(Debug, Eq, PartialEq, Clone, Copy, Default)]
pub struct Cursor {
    pub id: usize,
    pub position: usize,
    pub anchor_position: Option<usize>,
}

impl Cursor {
    pub fn new() -> Cursor {
        Cursor::default()
    }
}
