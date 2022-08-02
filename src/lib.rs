//! A model for text documents.
//!
//! This model is thought as a backend for a text UI. [`TextDocument`] can't be modified directly by the user, only for setting the whole document with `set_plain_text(...)`.
//! The user must use a [`TextCursor`] using `document.text_cursor_mut()` to make any change.
//!  
//! # Document structure
//!
//! ## Elements
//!
//! - [`Frame`]: contains Block elements and other Frame elements, formatable with FrameFormat
//! - [`Block`]: contains Text elements or Image elements, formatable with BlockFormat
//! - [`Text`]: contains the actuel text, formatable with TextFormat
//! - [`Image`]: represent the position of an image, formatable with ImageFormat
//!
//! All these items are encapsulated in its corresponding [`Element`] for ease of storage.
//!
//! ## The simpler plain text
//!
//! ```raw
//! Frame
//! |- Block
//!    |- Text
//! |- Block
//!    |- Text
//! ```
//!
//! ## The more complex rich text
//!
//! ```raw
//! Frame
//! |- Block
//!    |- Text  --> I really lo
//!    |- Text  --> ve (imagine it Formatted in bold)
//!    |- Text  --> Rust
//!    |- Image
//!    |- Text
//! |- Frame
//!    |- Block
//!       |- Text
//!       |- Text
//!       |- Text
//!    |- Block
//!       |- Text
//!       |- Text
//!       |- Text
//! |- Block
//!    |- Image
//! ```
//!
//! # Signaling changes
//!
//! Each modification is signaled using callbacks. [`TextDocument`] offers different ways to make your code aware of any change:
//!- [`TextDocument::add_text_change_callback()`]
//!
//!   Give the  number of removed characters and number of added characters with the reference of a cursor position.
//!
//!- [`TextDocument::add_element_change_callback()`]
//!
//!   Give the modified element with the reason. If two direct children elements changed at the same time.
//!
//! # Examples
//!
//!  ```rust
//!  use text_document::{TextDocument, ChangeReason, MoveMode};
//!
//!  let mut document = TextDocument::new();
//!
//!  document.add_text_change_callback(|position, removed_characters, added_characters|{
//!    println!("position: {}, removed_characters: {}, added_characters: {}", position, removed_characters, added_characters);
//!  } );
//!
//!  document.add_element_change_callback(|element, reason|{
//!    assert_eq!(element.uuid(), 0);
//!    assert_eq!(reason, ChangeReason::ChildrenChanged );
//!  } );
//!  document.set_plain_text("beginningend").unwrap();
//!
//!  let cursor = document.text_cursor_mut();
//!  cursor.set_position(9, MoveMode::MoveAnchor);
//!  cursor.insert_plain_text("new\nplain_text\ntest");
//!
//!  
//!  ```

pub mod block;
pub mod font;
pub mod format;
pub mod frame;
pub mod image;
pub mod text;
pub mod text_cursor;
pub mod text_document;

pub use crate::text_document::*;
pub use block::*;
pub use frame::*;
pub use image::*;
pub use text::*;
pub use text_cursor::*;

// Not public API.
#[doc(hidden)]
pub mod private {}
