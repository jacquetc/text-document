#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub enum MoveOperation {
    /// Keep the cursor where it is.
    NoMove,
    /// Move to the start of the document.
    Start,
    /// Move to the start of the current Paragraph.
    StartOfParagraph,
    /// Move to the start of the current word.
    StartOfWord,
    /// Move to the start of the previous Paragraph.
    PreviousParagraph,
    /// Move to the previous character.
    PreviousCharacter,
    /// Move to the beginning of the previous word.
    PreviousWord,
    /// Move to the end of the document.
    End,
    /// Move to the end of the current word.
    EndOfWord,
    /// Move to the end of the current Paragraph.
    EndOfParagraph,
    /// Move to the beginning of the next Paragraph.
    NextParagraph,
    /// Move to the next character.
    #[default]
    NextCharacter,
    /// Move to the next word.
    NextWord,
    /// Move to the beginning of the next table cell inside the current table. If the current cell is the last cell in the row, the cursor will move to the first cell in the next row.
    NextCell,
    /// Move to the beginning of the previous table cell inside the current table. If the current cell is the first cell in the row, the cursor will move to the last cell in the previous row.
    PreviousCell,
    /// Move to the first new cell of the next row in the current table.
    NextRow,
    /// Move to the last cell of the previous row in the current table.
    PreviousRow,
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub enum MoveMode {
    #[default]
    MoveAnchorWithCursor,
    MoveCursorOnly,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MovePositionDTO {
    pub operation: MoveOperation,
    pub mode: MoveMode,
    pub count: usize,
}

impl Default for MovePositionDTO {
    fn default() -> Self {
        Self {
            operation: Default::default(),
            mode: Default::default(),
            count: 1,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct SetPositionDTO {
    pub position: usize,
    pub mode: MoveMode,
}
