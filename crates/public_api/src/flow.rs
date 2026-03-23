//! Flow types for document traversal and layout engine support.
//!
//! The layout engine processes [`FlowElement`]s in order to build its layout
//! tree. Snapshot types capture consistent views for thread-safe reads.

use crate::text_block::TextBlock;
use crate::text_frame::TextFrame;
use crate::text_table::TextTable;
use crate::{Alignment, BlockFormat, FrameFormat, ListStyle, TextFormat};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// FlowElement
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// An element in the document's visual flow.
///
/// The layout engine processes these in order to build its layout tree.
/// Obtained from [`TextDocument::flow()`](crate::TextDocument::flow) or
/// [`TextFrame::flow()`].
#[derive(Clone)]
pub enum FlowElement {
    /// A paragraph or heading. Layout as a text block.
    Block(TextBlock),

    /// A table at this position in the flow. Layout as a grid.
    /// The anchor frame's `table` field identifies the table entity.
    Table(TextTable),

    /// A non-table sub-frame (float, sidebar, blockquote).
    /// Contains its own nested flow, accessible via
    /// [`TextFrame::flow()`].
    Frame(TextFrame),
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// FragmentContent
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A contiguous run of content with uniform formatting within a block.
///
/// Offsets are **block-relative**: `offset` is the character position
/// within the block where this fragment starts (0 = block start).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FragmentContent {
    /// A text run. The layout engine shapes these into glyphs.
    Text {
        text: String,
        format: TextFormat,
        /// Character offset within the block (block-relative).
        offset: usize,
        /// Character count.
        length: usize,
    },
    /// An inline image. The layout engine reserves space for it.
    ///
    /// To retrieve the image pixel data, use the existing
    /// [`TextDocument::resource(name)`](crate::TextDocument::resource) method.
    Image {
        name: String,
        width: u32,
        height: u32,
        quality: u32,
        format: TextFormat,
        /// Character offset within the block (block-relative).
        offset: usize,
    },
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BlockSnapshot
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// All layout-relevant data for one block, captured atomically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSnapshot {
    pub block_id: usize,
    pub position: usize,
    pub length: usize,
    pub text: String,
    pub fragments: Vec<FragmentContent>,
    pub block_format: BlockFormat,
    pub list_info: Option<ListInfo>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ListInfo
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// List membership and marker information for a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListInfo {
    pub list_id: usize,
    /// The list style (Disc, Decimal, LowerAlpha, etc.).
    pub style: ListStyle,
    /// Indentation level.
    pub indent: u8,
    /// Pre-formatted marker text: "•", "3.", "(c)", "IV.", etc.
    pub marker: String,
    /// 0-based index of this item within its list.
    pub item_index: usize,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TableCellRef
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Reference to a table cell that contains a block.
#[derive(Clone)]
pub struct TableCellRef {
    pub table: TextTable,
    pub row: usize,
    pub column: usize,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Table format types
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Table-level formatting.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TableFormat {
    pub border: Option<i32>,
    pub cell_spacing: Option<i32>,
    pub cell_padding: Option<i32>,
    pub width: Option<i32>,
    pub alignment: Option<Alignment>,
}

/// Cell-level formatting.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CellFormat {
    pub padding: Option<i32>,
    pub border: Option<i32>,
    pub vertical_alignment: Option<CellVerticalAlignment>,
    pub background_color: Option<String>,
}

/// Vertical alignment within a table cell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CellVerticalAlignment {
    #[default]
    Top,
    Middle,
    Bottom,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Table and Cell Snapshots
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Consistent snapshot of a table's structure and all cell content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSnapshot {
    pub table_id: usize,
    pub rows: usize,
    pub columns: usize,
    pub column_widths: Vec<i32>,
    pub format: TableFormat,
    pub cells: Vec<CellSnapshot>,
}

/// Snapshot of one table cell including its block content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellSnapshot {
    pub row: usize,
    pub column: usize,
    pub row_span: usize,
    pub column_span: usize,
    pub format: CellFormat,
    pub blocks: Vec<BlockSnapshot>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Flow Snapshots
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Consistent snapshot of the entire document flow, captured in a
/// single lock acquisition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowSnapshot {
    pub elements: Vec<FlowElementSnapshot>,
}

/// Snapshot of one flow element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowElementSnapshot {
    Block(BlockSnapshot),
    Table(TableSnapshot),
    Frame(FrameSnapshot),
}

/// Snapshot of a sub-frame and its contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameSnapshot {
    pub frame_id: usize,
    pub format: FrameFormat,
    pub elements: Vec<FlowElementSnapshot>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// FormatChangeKind
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// What kind of formatting changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatChangeKind {
    /// Block-level: alignment, margins, indent, heading level.
    /// Requires paragraph relayout.
    Block,
    /// Character-level: font, bold, italic, underline, color.
    /// Requires reshaping but not necessarily reflow.
    Character,
}
