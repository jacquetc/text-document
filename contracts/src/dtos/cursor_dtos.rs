#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MoveCursorDTO {
    pub cursor_id: usize,
    pub position: usize,
    pub anchor_position: Option<usize>,
}
