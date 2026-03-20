//! # text-document
//!
//! A rich text document model for Rust.
//!
//! Provides a [`TextDocument`] as the main entry point and [`TextCursor`] for
//! cursor-based editing, inspired by Qt's QTextDocument/QTextCursor API.
//!
//! ```rust
//! use text_document::{TextDocument, MoveMode, MoveOperation};
//!
//! let doc = TextDocument::new();
//! doc.set_plain_text("Hello world")?;
//!
//! let cursor = doc.cursor();
//! cursor.move_position(MoveOperation::EndOfWord, MoveMode::KeepAnchor, 1);
//! cursor.insert_text("Goodbye")?; // replaces "Hello"
//!
//! // Multiple cursors on the same document
//! let c1 = doc.cursor();
//! let c2 = doc.cursor_at(5);
//! c1.insert_text("A")?;
//! // c2's position is automatically adjusted
//!
//! doc.undo()?;
//! assert_eq!(doc.to_plain_text()?, "Hello world");
//! ```

use std::sync::{Arc, Mutex, Weak};

// TextDocument and TextCursor are Send + Sync (all fields are Arc<Mutex<...>>).
// This is intentional: the ARCHITECTURE.md specifies "Send + Sync throughout".
const _: () = {
    fn assert_send_sync<T: Send + Sync>() {}
    fn assert_all() {
        assert_send_sync::<TextDocument>();
        assert_send_sync::<TextCursor>();
    }
};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Re-exports from generated code (enums that consumers need)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
pub use common::entities::{
    Alignment,
    CharVerticalAlignment,
    FramePosition,
    InlineContent,
    ListStyle,
    MarkerType,
    ResourceType,
    TextDirection,
    UnderlineStyle,
    WrapMode,
};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Error type
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
pub type Result<T> = anyhow::Result<T>;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Public format types
//
// These wrap the Option<T> fields from the entity model into
// ergonomic structs. None = "not set / inherit from default."
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Character/text formatting. All fields are optional: `None` means
/// "not set — inherit from the block's default or the document's default."
///
/// Used both for querying format at a position (`cursor.char_format()`)
/// and for applying format (`cursor.set_char_format()`/`cursor.merge_char_format()`).
/// When applying with `merge`, only `Some(...)` fields overwrite the target.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextFormat {
    pub font_family: Option<String>,
    pub font_point_size: Option<u32>,
    pub font_weight: Option<u32>,
    pub font_bold: Option<bool>,
    pub font_italic: Option<bool>,
    pub font_underline: Option<bool>,
    pub font_overline: Option<bool>,
    pub font_strikeout: Option<bool>,
    pub letter_spacing: Option<i32>,
    pub word_spacing: Option<i32>,
    pub underline_style: Option<UnderlineStyle>,
    pub vertical_alignment: Option<CharVerticalAlignment>,
    pub anchor_href: Option<String>,
    pub anchor_names: Vec<String>,
    pub is_anchor: Option<bool>,
    pub tooltip: Option<String>,
}

/// Block (paragraph) formatting. All fields are optional.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BlockFormat {
    pub alignment: Option<Alignment>,
    pub top_margin: Option<i32>,
    pub bottom_margin: Option<i32>,
    pub left_margin: Option<i32>,
    pub right_margin: Option<i32>,
    pub heading_level: Option<u8>,
    pub indent: Option<u8>,
    pub text_indent: Option<i32>,
    pub marker: Option<MarkerType>,
    pub tab_positions: Vec<i32>,
}

/// Frame formatting. All fields are optional.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FrameFormat {
    pub height: Option<i32>,
    pub width: Option<i32>,
    pub top_margin: Option<i32>,
    pub bottom_margin: Option<i32>,
    pub left_margin: Option<i32>,
    pub right_margin: Option<i32>,
    pub padding: Option<i32>,
    pub border: Option<i32>,
    pub position: Option<FramePosition>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Enums for cursor movement
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Controls whether a movement collapses or extends the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveMode {
    /// Move both position and anchor — collapses selection.
    MoveAnchor,
    /// Move only position, keep anchor — creates or extends selection.
    KeepAnchor,
}

/// Semantic cursor movement operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveOperation {
    NoMove,
    Start,
    End,
    StartOfLine,
    EndOfLine,
    StartOfBlock,
    EndOfBlock,
    StartOfWord,
    EndOfWord,
    PreviousBlock,
    NextBlock,
    PreviousCharacter,
    NextCharacter,
    PreviousWord,
    NextWord,
    Up,
    Down,
    Left,
    Right,
    WordLeft,
    WordRight,
}

/// Quick-select a region around the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionType {
    WordUnderCursor,
    LineUnderCursor,
    BlockUnderCursor,
    Document,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Read-only info types returned by queries
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Document-level statistics. Always available (O(1), cached).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentStats {
    pub character_count: usize,
    pub word_count: usize,
    pub block_count: usize,
    pub frame_count: usize,
    pub image_count: usize,
    pub list_count: usize,
}

/// Info about a block at a given position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockInfo {
    pub block_id: usize,
    pub block_number: usize,
    pub start: usize,
    pub length: usize,
}

/// A single search match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindMatch {
    pub position: usize,
    pub length: usize,
}

/// Options for find / find_all / replace operations.
#[derive(Debug, Clone, Default)]
pub struct FindOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub use_regex: bool,
    pub search_backward: bool,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Long operation handle
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Shared state for a single long operation, managed by the document.
struct OperationState {
    // TODO: wraps LongOperationManager handle for this operation
}

/// A handle to a running long operation (Markdown/HTML import, DOCX export).
///
/// Provides typed access to progress, cancellation, and the result.
/// Progress events are also emitted via [`DocumentEvent::LongOperationProgress`]
/// and [`DocumentEvent::LongOperationFinished`] for the callback/polling path.
///
/// The result can only be retrieved once (via [`wait()`](Self::wait) or
/// [`try_result()`](Self::try_result)).
///
/// ```rust
/// let op = doc.set_markdown("# Hello\nWorld")?;
///
/// // Non-blocking: check progress
/// if let Some((percent, msg)) = op.progress() {
///     println!("{percent}% — {msg}");
/// }
///
/// // Blocking: wait for the result
/// let result = op.wait()?;
/// println!("Imported {} blocks", result.block_count);
///
/// // Or cancel
/// op.cancel();
/// ```
pub struct Operation<T> {
    id: String,
    state: Arc<Mutex<OperationState>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> Operation<T> {
    /// The operation ID (for matching with [`DocumentEvent`] variants).
    pub fn id(&self) -> &str { &self.id }

    /// Get the current progress, if available.
    /// Returns `(percent, message)` where percent is 0.0–100.0.
    pub fn progress(&self) -> Option<(f64, String)> { todo!() }

    /// Returns `true` if the operation has finished (success or failure).
    pub fn is_done(&self) -> bool { todo!() }

    /// Cancel the operation. No-op if already finished.
    pub fn cancel(&self) { todo!() }

    /// Block the calling thread until the operation completes and return
    /// the typed result. Consumes the handle.
    pub fn wait(self) -> Result<T> { todo!() }

    /// Non-blocking: takes the result if the operation has completed,
    /// returns `None` if still running. The result can only be taken once;
    /// subsequent calls return `None`.
    pub fn try_result(&mut self) -> Option<Result<T>> { todo!() }
}

/// Result of a Markdown import (`set_markdown`).
#[derive(Debug, Clone)]
pub struct MarkdownImportResult {
    pub block_count: usize,
}

/// Result of an HTML import (`set_html`).
#[derive(Debug, Clone)]
pub struct HtmlImportResult {
    pub block_count: usize,
}

/// Result of a DOCX export (`to_docx`).
#[derive(Debug, Clone)]
pub struct DocxExportResult {
    pub file_path: String,
    pub paragraph_count: usize,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// DocumentFragment
//
// A format-agnostic container of rich text, analogous to
// Qt's QTextDocumentFragment. Can be created from any source
// format (plain text, HTML, Markdown, cursor selection) and
// exported to any target format. The internal representation
// is a serialized snapshot of blocks + inline elements + formats.
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A piece of rich text that can be inserted into a [`TextDocument`].
///
/// `DocumentFragment` is the clipboard/interchange type. It carries
/// blocks, inline elements, and formatting in a format-agnostic
/// internal representation.
///
/// # Creating fragments
///
/// ```rust
/// // From various source formats
/// let frag = DocumentFragment::from_plain_text("Hello\nWorld");
/// let frag = DocumentFragment::from_html("<b>Hello</b> World");
/// let frag = DocumentFragment::from_markdown("**Hello** World");
///
/// // From a cursor selection (no format round-trip — internal copy)
/// let frag = cursor.selection();
///
/// // From an entire document
/// let frag = DocumentFragment::from_document(&doc)?;
/// ```
///
/// # Inserting fragments
///
/// ```rust
/// cursor.insert_fragment(&frag)?;
/// ```
///
/// # Exporting fragments
///
/// ```rust
/// let text = frag.to_plain_text();
/// let html = frag.to_html();
/// let md = frag.to_markdown();
/// ```
#[derive(Debug, Clone)]
pub struct DocumentFragment {
    /// Serialized internal representation (JSON of blocks + elements + formats).
    /// This is the format passed to/from the backend via `fragment_data` DTOs.
    data: String,
    /// Cached plain text (always available, avoids re-parsing for simple cases).
    plain_text: String,
}

impl DocumentFragment {
    // ── Constructors ─────────────────────────────────────────

    /// Create an empty fragment.
    pub fn new() -> Self {
        Self { data: String::new(), plain_text: String::new() }
    }

    /// Create a fragment from plain text.
    ///
    /// Each line becomes a block with a single `InlineContent::Text` element.
    /// No formatting is applied.
    pub fn from_plain_text(text: &str) -> Self { todo!() }

    /// Create a fragment from HTML.
    ///
    /// Formatting is preserved as much as possible: `<b>bold</b>` becomes
    /// a text element with `fmt_font_bold: Some(true)`.
    pub fn from_html(html: &str) -> Self { todo!() }

    /// Create a fragment from Markdown.
    ///
    /// Formatting is preserved: `**bold**` becomes bold, `# Heading`
    /// becomes a block with `fmt_heading_level: Some(1)`, etc.
    pub fn from_markdown(markdown: &str) -> Self { todo!() }

    /// Create a fragment from an entire document.
    pub fn from_document(doc: &TextDocument) -> Result<Self> { todo!() }

    /// Create a fragment from the serialized internal format.
    /// Used when receiving fragment data from the backend.
    pub(crate) fn from_raw(data: String, plain_text: String) -> Self {
        Self { data, plain_text }
    }

    // ── Export ────────────────────────────────────────────────

    /// Export the fragment as plain text.
    pub fn to_plain_text(&self) -> &str { &self.plain_text }

    /// Export the fragment as HTML.
    pub fn to_html(&self) -> String { todo!() }

    /// Export the fragment as Markdown.
    pub fn to_markdown(&self) -> String { todo!() }

    // ── Queries ──────────────────────────────────────────────

    /// Returns true if the fragment contains no text or elements.
    pub fn is_empty(&self) -> bool {
        self.plain_text.is_empty()
    }

    /// Returns the serialized internal representation.
    /// Used when passing fragment data to the backend.
    pub(crate) fn raw_data(&self) -> &str { &self.data }
}

impl Default for DocumentFragment {
    fn default() -> Self { Self::new() }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Events emitted by TextDocument
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Events emitted by a [`TextDocument`].
///
/// Subscribe via [`TextDocument::on_change`] (callback-based, for Slint/GTK)
/// or poll via [`TextDocument::poll_events`] (frame-loop, for egui/iced).
///
/// These events carry enough information for a UI to do incremental updates —
/// repaint only the affected region, not the entire document.
#[derive(Debug, Clone)]
pub enum DocumentEvent {
    /// Text content changed at a specific region.
    ///
    /// The UI should repaint from `position` onward. If `blocks_affected > 1`,
    /// multiple paragraphs need relayout.
    ///
    /// Emitted by: insert_text, delete_text, insert_formatted_text,
    /// insert_block, insert_fragment, insert_html/markdown_at_position,
    /// replace_text.
    ContentsChanged {
        position: usize,
        chars_removed: usize,
        chars_added: usize,
        blocks_affected: usize,
    },

    /// Formatting changed without text content change.
    ///
    /// The UI should repaint the affected range. No text reflow needed,
    /// only style update (bold/italic/color/alignment).
    ///
    /// Emitted by: set_text_format, merge_text_format, set_block_format,
    /// set_frame_format.
    FormatChanged {
        position: usize,
        length: usize,
    },

    /// Block count changed. Carries the new count.
    ///
    /// Useful for UIs that display a block/paragraph count,
    /// or that need to resize scroll regions.
    BlockCountChanged(usize),

    /// The document was completely replaced (import, clear).
    ///
    /// The UI must discard all cached layout and repaint everything.
    /// Do not try to diff — just reload.
    ///
    /// Emitted by: set_plain_text, set_html, set_markdown, clear.
    DocumentReset,

    /// Undo/redo was performed or availability changed.
    ///
    /// The UI should update undo/redo button states and repaint
    /// affected content.
    UndoRedoChanged {
        can_undo: bool,
        can_redo: bool,
    },

    /// The modified flag changed.
    ///
    /// The UI typically updates the window title (add/remove "*").
    ModificationChanged(bool),

    /// A long operation progressed.
    LongOperationProgress {
        operation_id: String,
        percent: f64,
        message: String,
    },

    /// A long operation completed or failed.
    LongOperationFinished {
        operation_id: String,
        success: bool,
        error: Option<String>,
    },
}

/// Handle to a document event subscription.
///
/// Events are delivered as long as this handle is alive.
/// Drop it to unsubscribe. No explicit unsubscribe method needed.
///
/// ```rust
/// let sub = doc.on_change(|event| { /* ... */ });
/// // events flow while `sub` is alive
/// drop(sub); // unsubscribes
/// ```
pub struct Subscription {
    // TODO: real implementation needs a mechanism to signal unsubscription on drop,
    // e.g. Arc<AtomicBool> flipped on Drop, or dropping a channel sender.
    _inner: (),
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Internal shared state
//
// Lock ordering (enforced by convention, never violated):
//   1. TextDocumentInner   (the document lock)
//   2. CursorData          (individual cursor locks)
//
// Always acquire the document lock before any cursor lock.
// Pure cursor-local reads (position, anchor, has_selection) may lock
// only CursorData without the document lock — this is safe because
// they never touch the document lock in the same call.
// Editing methods must lock the document first, then read/update
// cursor data while the document lock is held, and call
// adjust_cursors() before releasing the document lock.
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Cursor position data stored inside the document for automatic adjustment.
struct CursorData {
    position: usize,
    anchor: usize,
}

/// The shared document interior, behind `Arc<Mutex<>>`.
///
/// All document state lives here. Both `TextDocument` and `TextCursor`
/// hold `Arc<Mutex<TextDocumentInner>>` and lock as needed. The document
/// tracks all live cursors via `Weak` references and adjusts their
/// positions after every edit.
struct TextDocumentInner {
    // TODO: wraps frontend::AppContext + stack_id + event dispatch
    // Tracks all live cursors for automatic position adjustment.
    cursors: Vec<Weak<Mutex<CursorData>>>,
}

impl TextDocumentInner {
    /// Remove dead `Weak` refs from the cursor list.
    fn prune_dead_cursors(&mut self) {
        self.cursors.retain(|w| w.strong_count() > 0);
    }

    /// After an edit, adjust all tracked cursor positions.
    ///
    /// `edit_pos` is where the edit happened, `removed` is the number of
    /// characters removed, `added` is the number of characters inserted.
    ///
    /// SAFETY (lock ordering): this method is called while the document lock
    /// is held. It then locks individual CursorData mutexes. This is safe
    /// because CursorData locks are always acquired after the document lock,
    /// never before.
    fn adjust_cursors(&mut self, edit_pos: usize, removed: usize, added: usize) {
        self.prune_dead_cursors();
        for weak in &self.cursors {
            if let Some(cursor) = weak.upgrade() {
                let mut data = cursor.lock().unwrap();
                data.position = adjust_offset(data.position, edit_pos, removed, added);
                data.anchor = adjust_offset(data.anchor, edit_pos, removed, added);
            }
        }
    }

    /// Register a new cursor and return its shared data.
    fn register_cursor(&mut self, position: usize) -> Arc<Mutex<CursorData>> {
        self.prune_dead_cursors();
        let data = Arc::new(Mutex::new(CursorData { position, anchor: position }));
        self.cursors.push(Arc::downgrade(&data));
        data
    }
}

/// Shift an offset after an edit: offsets before the edit are unchanged,
/// offsets inside the removed range clamp to the edit point, offsets after
/// shift by the delta.
fn adjust_offset(offset: usize, edit_pos: usize, removed: usize, added: usize) -> usize {
    if offset <= edit_pos {
        offset
    } else if offset <= edit_pos + removed {
        edit_pos + added
    } else {
        offset - removed + added
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TextDocument
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A rich text document.
///
/// Owns the backend (database, event hub, undo/redo manager) and provides
/// document-level operations. All cursor-based editing goes through
/// [`TextCursor`], obtained via [`cursor()`](TextDocument::cursor) or
/// [`cursor_at()`](TextDocument::cursor_at).
///
/// Internally uses `Arc<Mutex<...>>` so that multiple [`TextCursor`]s can
/// coexist and edit concurrently. Cloning a `TextDocument` creates a new
/// handle to the **same** underlying document (like Qt's implicit sharing).
#[derive(Clone)]
pub struct TextDocument {
    inner: Arc<Mutex<TextDocumentInner>>,
}

impl TextDocument {
    // ── Construction ──────────────────────────────────────────

    /// Create a new, empty document.
    pub fn new() -> Self { todo!() }

    // ── Whole-document content ────────────────────────────────

    /// Replace the entire document with plain text. Clears undo history.
    pub fn set_plain_text(&self, text: &str) -> Result<()> { todo!() }

    /// Export the entire document as plain text.
    pub fn to_plain_text(&self) -> Result<String> { todo!() }

    /// Replace the entire document with Markdown. Clears undo history.
    ///
    /// This is a **long operation** (parsing can be slow for large documents).
    /// Returns a typed [`Operation`] handle for progress tracking,
    /// cancellation, and retrieving the result.
    pub fn set_markdown(&self, markdown: &str) -> Result<Operation<MarkdownImportResult>> { todo!() }

    /// Export the entire document as Markdown.
    pub fn to_markdown(&self) -> Result<String> { todo!() }

    /// Replace the entire document with HTML. Clears undo history.
    ///
    /// This is a **long operation** (parsing can be slow for large documents).
    /// Returns a typed [`Operation`] handle for progress tracking,
    /// cancellation, and retrieving the result.
    pub fn set_html(&self, html: &str) -> Result<Operation<HtmlImportResult>> { todo!() }

    /// Export the entire document as HTML.
    pub fn to_html(&self) -> Result<String> { todo!() }

    /// Export the entire document as LaTeX.
    pub fn to_latex(&self, document_class: &str, include_preamble: bool) -> Result<String> { todo!() }

    /// Export the entire document as DOCX to a file path.
    ///
    /// This is a **long operation** (serialization can be slow for large documents).
    /// Returns a typed [`Operation`] handle for progress tracking,
    /// cancellation, and retrieving the result.
    pub fn to_docx(&self, output_path: &str) -> Result<Operation<DocxExportResult>> { todo!() }

    /// Clear all document content and reset to an empty state.
    /// Clears undo history. Emits [`DocumentEvent::DocumentReset`].
    pub fn clear(&self) -> Result<()> { todo!() }

    // ── Cursor factory ───────────────────────────────────────

    /// Create a cursor at position 0.
    ///
    /// Multiple cursors can coexist on the same document. When any cursor
    /// edits text, all other cursors' positions are automatically adjusted
    /// (like Qt's `QTextCursor`).
    pub fn cursor(&self) -> TextCursor {
        self.cursor_at(0)
    }

    /// Create a cursor at the given position.
    ///
    /// Multiple cursors can coexist on the same document. When any cursor
    /// edits text, all other cursors' positions are automatically adjusted.
    pub fn cursor_at(&self, position: usize) -> TextCursor {
        let data = {
            let mut inner = self.inner.lock().unwrap();
            inner.register_cursor(position)
        };
        TextCursor { doc: self.inner.clone(), data }
    }

    // ── Document queries ─────────────────────────────────────

    /// Get document statistics (character count, word count, etc.).
    /// O(1) — reads cached values.
    pub fn stats(&self) -> DocumentStats { todo!() }

    /// Get the total character count (excluding block separators).
    /// O(1) — reads cached value.
    pub fn character_count(&self) -> usize { todo!() }

    /// Get the number of blocks (paragraphs).
    /// O(1) — reads cached value.
    pub fn block_count(&self) -> usize { todo!() }

    /// Returns true if the document has no text content.
    pub fn is_empty(&self) -> bool { todo!() }

    /// Get text at a position for a given length.
    pub fn text_at(&self, position: usize, length: usize) -> Result<String> { todo!() }

    /// Get info about the block at a position.
    /// O(log n) — binary search on cached `document_position`.
    pub fn block_at(&self, position: usize) -> Result<BlockInfo> { todo!() }

    /// Get the block format at a position.
    pub fn block_format_at(&self, position: usize) -> Result<BlockFormat> { todo!() }

    // ── Search ───────────────────────────────────────────────

    /// Find the next (or previous) occurrence of a string or regex.
    /// Returns `None` if not found.
    pub fn find(&self, query: &str, from: usize, options: &FindOptions) -> Result<Option<FindMatch>> { todo!() }

    /// Find all occurrences of a string or regex.
    pub fn find_all(&self, query: &str, options: &FindOptions) -> Result<Vec<FindMatch>> { todo!() }

    /// Replace all occurrences (or next occurrence if `replace_all` is false).
    /// Returns the number of replacements made. Undoable.
    pub fn replace_text(
        &self,
        query: &str,
        replacement: &str,
        replace_all: bool,
        options: &FindOptions,
    ) -> Result<usize> { todo!() }

    // ── Resources (images, stylesheets) ──────────────────────

    /// Add a resource (image, stylesheet) to the document.
    pub fn add_resource(
        &self,
        resource_type: ResourceType,
        name: &str,
        mime_type: &str,
        data: &[u8],
    ) -> Result<()> { todo!() }

    /// Get a resource by name. Returns `None` if not found.
    pub fn resource(&self, name: &str) -> Result<Option<Vec<u8>>> { todo!() }

    // ── Undo / Redo ──────────────────────────────────────────

    /// Undo the last operation.
    pub fn undo(&self) -> Result<()> { todo!() }

    /// Redo the last undone operation.
    pub fn redo(&self) -> Result<()> { todo!() }

    /// Returns true if there are operations that can be undone.
    pub fn can_undo(&self) -> bool { todo!() }

    /// Returns true if there are operations that can be redone.
    pub fn can_redo(&self) -> bool { todo!() }

    /// Clear all undo/redo history.
    pub fn clear_undo_redo(&self) { todo!() }

    // ── Modified state ───────────────────────────────────────

    /// Returns true if the document has been modified since the last
    /// call to `set_modified(false)` or since creation.
    pub fn is_modified(&self) -> bool { todo!() }

    /// Set or clear the modified flag.
    pub fn set_modified(&self, modified: bool) { todo!() }

    // ── Document properties ──────────────────────────────────

    /// Get the document title.
    pub fn title(&self) -> String { todo!() }

    /// Set the document title.
    pub fn set_title(&self, title: &str) -> Result<()> { todo!() }

    /// Get the text direction.
    pub fn text_direction(&self) -> TextDirection { todo!() }

    /// Set the text direction.
    pub fn set_text_direction(&self, direction: TextDirection) -> Result<()> { todo!() }

    /// Get the default wrap mode.
    pub fn default_wrap_mode(&self) -> WrapMode { todo!() }

    /// Set the default wrap mode.
    pub fn set_default_wrap_mode(&self, mode: WrapMode) -> Result<()> { todo!() }

    // ── Event subscription ─────────────────────────────────────

    /// Subscribe to document events via callback.
    ///
    /// The callback is invoked on a **background thread** — the UI framework
    /// must dispatch to its main thread (e.g., `invoke_from_event_loop` in Slint,
    /// `glib::idle_add` in GTK).
    ///
    /// Returns a [`Subscription`] handle. Events flow as long as the handle
    /// is alive. Drop it to unsubscribe.
    ///
    /// ```rust
    /// let sub = doc.on_change(|event| {
    ///     match event {
    ///         DocumentEvent::ContentsChanged { position, .. } => {
    ///             // schedule repaint from position
    ///         }
    ///         DocumentEvent::UndoRedoChanged { can_undo, can_redo } => {
    ///             // update toolbar buttons
    ///         }
    ///         _ => {}
    ///     }
    /// });
    /// ```
    pub fn on_change<F>(&self, callback: F) -> Subscription
    where
        F: Fn(DocumentEvent) + Send + 'static,
    { todo!() }

    /// Drain all pending events since the last call.
    ///
    /// Intended for **polling-based UIs** (egui, iced, game loops).
    /// Call once per frame. Returns an empty `Vec` if nothing changed.
    ///
    /// ```rust
    /// // In egui's update():
    /// for event in doc.poll_events() {
    ///     match event {
    ///         DocumentEvent::ContentsChanged { .. } => ctx.request_repaint(),
    ///         DocumentEvent::ModificationChanged(m) => self.dirty = m,
    ///         _ => {}
    ///     }
    /// }
    /// ```
    pub fn poll_events(&self) -> Vec<DocumentEvent> { todo!() }
}

impl Default for TextDocument {
    fn default() -> Self { Self::new() }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TextCursor
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A cursor into a [`TextDocument`].
///
/// Provides position tracking, selection, editing, and formatting.
/// Obtained via [`TextDocument::cursor()`] or [`TextDocument::cursor_at()`].
///
/// Multiple cursors can coexist on the same document (like Qt's `QTextCursor`).
/// When any cursor edits text, all other cursors' positions are automatically
/// adjusted by the document.
///
/// Cloning a cursor creates an **independent** cursor at the same position
/// (like Qt's copy semantics), not a second handle to the same position.
///
/// When the cursor has a selection (`anchor != position`), editing operations
/// replace the selected text.
pub struct TextCursor {
    doc: Arc<Mutex<TextDocumentInner>>,
    data: Arc<Mutex<CursorData>>,
}

impl Clone for TextCursor {
    /// Create an independent cursor at the same position.
    ///
    /// The clone is registered with the document and will have its position
    /// automatically adjusted by edits, just like the original.
    fn clone(&self) -> Self {
        let (position, anchor) = {
            let d = self.data.lock().unwrap();
            (d.position, d.anchor)
        };
        let data = {
            let mut inner = self.doc.lock().unwrap();
            let data = Arc::new(Mutex::new(CursorData { position, anchor }));
            inner.cursors.push(Arc::downgrade(&data));
            data
        };
        TextCursor { doc: self.doc.clone(), data }
    }
}

impl TextCursor {

    // ── Position & selection ─────────────────────────────────

    /// Current cursor position (between characters).
    pub fn position(&self) -> usize {
        self.data.lock().unwrap().position
    }

    /// Anchor position. Equal to `position()` when no selection.
    pub fn anchor(&self) -> usize {
        self.data.lock().unwrap().anchor
    }

    /// Returns true if there is a selection (anchor != position).
    pub fn has_selection(&self) -> bool {
        let d = self.data.lock().unwrap();
        d.position != d.anchor
    }

    /// Start of the selection (min of position and anchor).
    pub fn selection_start(&self) -> usize {
        let d = self.data.lock().unwrap();
        d.position.min(d.anchor)
    }

    /// End of the selection (max of position and anchor).
    pub fn selection_end(&self) -> usize {
        let d = self.data.lock().unwrap();
        d.position.max(d.anchor)
    }

    /// Get the selected text. Returns empty string if no selection.
    pub fn selected_text(&self) -> Result<String> { todo!() }

    /// Collapse the selection by moving anchor to position.
    pub fn clear_selection(&self) {
        let mut d = self.data.lock().unwrap();
        d.anchor = d.position;
    }

    // ── Boundary queries ─────────────────────────────────────

    /// True if the cursor is at the start of a block.
    pub fn at_block_start(&self) -> bool { todo!() }

    /// True if the cursor is at the end of a block.
    pub fn at_block_end(&self) -> bool { todo!() }

    /// True if the cursor is at the very start of the document (position 0).
    pub fn at_start(&self) -> bool {
        self.data.lock().unwrap().position == 0
    }

    /// True if the cursor is at the very end of the document.
    pub fn at_end(&self) -> bool { todo!() }

    /// The block number (0-indexed) containing the cursor.
    pub fn block_number(&self) -> usize { todo!() }

    /// The cursor's column within the current block (0-indexed).
    pub fn position_in_block(&self) -> usize { todo!() }

    // ── Movement ─────────────────────────────────────────────

    /// Set the cursor to an absolute position.
    ///
    /// With `MoveMode::MoveAnchor`, collapses the selection.
    /// With `MoveMode::KeepAnchor`, extends the selection.
    pub fn set_position(&self, position: usize, mode: MoveMode) { todo!() }

    /// Move the cursor by a semantic operation (next word, start of block, etc.).
    ///
    /// `n` is the repeat count (default 1). Returns `true` if the cursor moved.
    ///
    /// With `MoveMode::KeepAnchor`, the anchor stays and a selection is created.
    pub fn move_position(
        &self,
        operation: MoveOperation,
        mode: MoveMode,
        n: usize,
    ) -> bool { todo!() }

    /// Select a region relative to the cursor position.
    ///
    /// - `WordUnderCursor`: selects the word at the cursor
    /// - `BlockUnderCursor`: selects the entire current block
    /// - `LineUnderCursor`: selects the current line (same as block for now)
    /// - `Document`: selects the entire document
    pub fn select(&self, selection: SelectionType) { todo!() }

    // ── Text editing ─────────────────────────────────────────
    // All editing operations:
    // - Lock the document first, then read/update cursor data (lock ordering)
    // - Replace the selection if one exists
    // - Update the cursor position from the result DTO
    // - Call adjust_cursors() to shift all other cursors
    // - Are undoable (pushed to the undo stack)
    // - Update cached fields (text_length, document_position, plain_text)

    /// Insert plain text at the cursor. Replaces selection if any.
    pub fn insert_text(&self, text: &str) -> Result<()> { todo!() }

    /// Insert text with a specific character format. Replaces selection if any.
    pub fn insert_formatted_text(&self, text: &str, format: &TextFormat) -> Result<()> { todo!() }

    /// Insert a block break (new paragraph) at the cursor. Like pressing Enter.
    /// Replaces selection if any.
    pub fn insert_block(&self) -> Result<()> { todo!() }

    /// Insert an HTML fragment at the cursor position. Replaces selection if any.
    pub fn insert_html(&self, html: &str) -> Result<()> { todo!() }

    /// Insert a Markdown fragment at the cursor position. Replaces selection if any.
    pub fn insert_markdown(&self, markdown: &str) -> Result<()> { todo!() }

    /// Insert a document fragment at the cursor. Replaces selection if any.
    ///
    /// This is the format-agnostic insertion method. The fragment carries
    /// its own structure and formatting — no format conversion happens.
    /// Use this for clipboard paste and internal copy/paste.
    pub fn insert_fragment(&self, fragment: &DocumentFragment) -> Result<()> { todo!() }

    /// Extract the current selection as a [`DocumentFragment`].
    ///
    /// If there is no selection, returns an empty fragment.
    /// The fragment carries the full structure: blocks, inline elements,
    /// and all formatting. No format conversion — it's an internal copy.
    ///
    /// ```rust
    /// cursor.select(SelectionType::WordUnderCursor);
    /// let frag = cursor.selection();
    /// // frag can be inserted elsewhere, exported to HTML/Markdown, etc.
    /// ```
    pub fn selection(&self) -> DocumentFragment { todo!() }

    /// Insert an image at the cursor. The `name` must match a resource
    /// added via [`TextDocument::add_resource`]. Replaces selection if any.
    pub fn insert_image(&self, name: &str, width: u32, height: u32) -> Result<()> { todo!() }

    /// Insert a new frame at the cursor. Replaces selection if any.
    pub fn insert_frame(&self) -> Result<()> { todo!() }

    /// Delete the character after the cursor (Delete key).
    /// If there is a selection, deletes the selection instead.
    pub fn delete_char(&self) -> Result<()> { todo!() }

    /// Delete the character before the cursor (Backspace key).
    /// If there is a selection, deletes the selection instead.
    pub fn delete_previous_char(&self) -> Result<()> { todo!() }

    /// Delete the selected text. Returns the deleted text.
    /// No-op if no selection.
    pub fn remove_selected_text(&self) -> Result<String> { todo!() }

    // ── List operations ──────────────────────────────────────

    /// Turn the block(s) in the selection into a list with the given style.
    /// If no selection, applies to the current block.
    pub fn create_list(&self, style: ListStyle) -> Result<()> { todo!() }

    /// Insert a new list item at the cursor position.
    pub fn insert_list(&self, style: ListStyle) -> Result<()> { todo!() }

    // ── Format queries ───────────────────────────────────────
    // These read the format of the InlineElement / Block at the cursor.
    // Returns Option<T> fields: None means "not set" (inherit).

    /// Get the character format at the cursor position.
    ///
    /// If there is a selection, returns the format of the first character
    /// in the selection. Fields that differ across the selection are `None`.
    pub fn char_format(&self) -> Result<TextFormat> { todo!() }

    /// Get the block format of the block containing the cursor.
    pub fn block_format(&self) -> Result<BlockFormat> { todo!() }

    // ── Format application ───────────────────────────────────

    /// Set the character format for the selection (or for future inserts
    /// if no selection). Replaces all format fields.
    pub fn set_char_format(&self, format: &TextFormat) -> Result<()> { todo!() }

    /// Merge a character format into the selection. Only `Some(...)` fields
    /// in `format` overwrite; `None` fields are left unchanged.
    pub fn merge_char_format(&self, format: &TextFormat) -> Result<()> { todo!() }

    /// Set the block format for the current block (or all blocks in the
    /// selection). Replaces all format fields.
    pub fn set_block_format(&self, format: &BlockFormat) -> Result<()> { todo!() }

    /// Set the frame format. Uses `frame_id` if nonzero, otherwise
    /// targets the frame at the cursor position.
    pub fn set_frame_format(&self, frame_id: usize, format: &FrameFormat) -> Result<()> { todo!() }

    // ── Edit blocks (composite undo) ─────────────────────────
    // These map to UndoRedoManager::begin_composite/end_composite,
    // which is document-level state. The cursor routes these calls
    // to the document's undo manager. Nesting works correctly across
    // cursors — only the outermost begin/end pair defines the scope.

    /// Begin a group of operations that will be undone as a single unit.
    ///
    /// Calls can be nested — only the outermost pair defines the undo scope.
    ///
    /// ```rust
    /// cursor.begin_edit_block();
    /// cursor.insert_text("Hello ")?;
    /// cursor.insert_text("World")?;
    /// cursor.end_edit_block();
    /// // One undo() reverses both inserts
    /// ```
    pub fn begin_edit_block(&self) { todo!() }

    /// End the current edit block.
    pub fn end_edit_block(&self) { todo!() }

    /// Join with the previous edit block. Makes subsequent operations
    /// part of the last completed edit block. Used for continuous typing
    /// (each keystroke extends the same undo unit).
    pub fn join_previous_edit_block(&self) { todo!() }
}
