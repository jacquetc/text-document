[![crates.io](https://img.shields.io/crates/v/text-document?style=flat-square&logo=rust)](https://crates.io/crates/text-document)
[![API](https://docs.rs/text-document/badge.svg)](https://docs.rs/text-document)
[![license](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue?style=flat-square)](#license)
[![build status](https://img.shields.io/github/workflow/status/jacquetc/text-document/CI/main?style=flat-square&logo=github)](https://github.com/jacquetc/text-document/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/jacquetc/text-document/branch/main/graph/badge.svg?token=S4M513A2XR)](https://codecov.io/gh/jacquetc/text-document)
![Lines of code](https://img.shields.io/tokei/lines/github.com/jacquetc/text-document)

# text-document
A text document structure and management for Rust

This model is thought as a backend for a text UI. [`TextDocument`] can't be modified directly by the user, only for setting the whole document with `set_plain_text(...)`.
The user must use a [`TextCursor`] using `document.create_cursor()` to make any change.
  
# Document structure

## Elements

- [`Frame`]: contains Block elements and other Frame elements, formatable with FrameFormat
- [`Block`]: contains Text elements or Image elements, formatable with BlockFormat
- [`Text`]: contains the actuel text, formatable with TextFormat
- [`Image`]: represent the position of an image, formatable with ImageFormat

All these items are encapsulated in its corresponding [`Element`] for ease of storage.

## The simpler plain text

```raw
Frame
|- Block
   |- Text
|- Block
   |- Text
```

## The more complex rich text

```raw
Frame
|- Block
   |- Text  --> I really lo
   |- Text  --> ve (imagine it Formatted in bold)
   |- Text  --> Rust
   |- Image
   |- Text
|- Frame
   |- Block
      |- Text
      |- Text
      |- Text
   |- Block
      |- Text
      |- Text
      |- Text
|- Block
   |- Image
```

# Signaling changes

Each modification is signaled using callbacks. [`TextDocument`] offers different ways to make your code aware of any change:
- [`TextDocument::add_text_change_callback()`]

   Give the  number of removed characters and number of added characters with the reference of a cursor position.

- [`TextDocument::add_element_change_callback()`]

   Give the modified element with the reason. If two direct children elements changed at the same time.

# Examples

```rust
use text_document::{TextDocument, ChangeReason, MoveMode};

let mut document = TextDocument::new();

document.add_text_change_callback(|position, removed_characters, added_characters|{
  println!("position: {}, removed_characters: {}, added_characters: {}", position, removed_characters, added_characters);
} );

document.add_element_change_callback(|element, reason|{
  assert_eq!(element.uuid(), 0);
  assert_eq!(reason, ChangeReason::ChildrenChanged );
} );
document.set_plain_text("beginningend").unwrap();

let mut cursor = document.create_cursor();
cursor.set_position(9, MoveMode::MoveAnchor);
cursor.insert_plain_text("new\nplain_text\ntest");

  
```

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.