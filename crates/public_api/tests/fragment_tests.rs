use text_document::{
    Alignment, BlockFormat, DocumentFragment, FlowElement, FragmentContent, ListStyle, MoveMode,
    MoveOperation, SelectionType, TextDocument, TextFormat,
};

fn new_doc_with_text(text: &str) -> TextDocument {
    let doc = TextDocument::new();
    doc.set_plain_text(text).unwrap();
    doc
}

/// Insert a fragment into a fresh document and return the document for inspection.
fn insert_into_fresh_doc(frag: &DocumentFragment) -> TextDocument {
    let doc = TextDocument::new();
    let cursor = doc.cursor();
    cursor.insert_fragment(frag).unwrap();
    doc
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Construction basics
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn new_fragment_is_empty() {
    let frag = DocumentFragment::new();
    assert!(frag.is_empty());
    assert_eq!(frag.to_plain_text(), "");
    assert_eq!(frag.to_html(), "<html><head><meta charset=\"utf-8\"></head><body></body></html>");
    assert_eq!(frag.to_markdown(), "");
}

#[test]
fn default_fragment_is_empty() {
    let frag = DocumentFragment::default();
    assert!(frag.is_empty());
    assert_eq!(frag.to_plain_text(), "");
}

#[test]
fn fragment_clone() {
    let frag = DocumentFragment::from_plain_text("Clone me");
    let cloned = frag.clone();
    assert_eq!(cloned.to_plain_text(), "Clone me");
    assert!(!cloned.is_empty());
}

#[test]
fn fragment_debug() {
    let frag = DocumentFragment::from_plain_text("Test");
    let debug = format!("{:?}", frag);
    assert!(debug.contains("DocumentFragment"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// from_plain_text — verified by cursor inspection
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn from_plain_text_single_line() {
    let frag = DocumentFragment::from_plain_text("Hello world");
    assert_eq!(frag.to_plain_text(), "Hello world");

    let doc = insert_into_fresh_doc(&frag);
    let blocks = doc.blocks();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].text(), "Hello world");
    assert!(blocks[0].list().is_none(), "Plain text should have no list");
    assert_eq!(
        blocks[0].block_format().heading_level,
        None,
        "Plain text should have no heading"
    );
}

#[test]
fn from_plain_text_multiline() {
    let frag = DocumentFragment::from_plain_text("Line 1\nLine 2\nLine 3");

    let doc = insert_into_fresh_doc(&frag);
    let blocks = doc.blocks();
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0].text(), "Line 1");
    assert_eq!(blocks[1].text(), "Line 2");
    assert_eq!(blocks[2].text(), "Line 3");
    for b in &blocks {
        assert!(b.list().is_none());
        assert_eq!(b.block_format().heading_level, None);
    }
}

#[test]
fn from_plain_text_empty() {
    let frag = DocumentFragment::from_plain_text("");
    assert!(frag.is_empty());
    assert_eq!(frag.to_plain_text(), "");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// from_document — verified by cursor inspection
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn from_document_captures_content() {
    let doc = new_doc_with_text("Hello world");
    let frag = DocumentFragment::from_document(&doc).unwrap();
    assert!(!frag.is_empty());
    assert_eq!(frag.to_plain_text(), "Hello world");
}

#[test]
fn from_empty_document() {
    let doc = TextDocument::new();
    let frag = DocumentFragment::from_document(&doc).unwrap();
    assert!(frag.is_empty());
}

#[test]
fn from_document_with_heading() {
    let doc = TextDocument::new();
    doc.set_markdown("## My Title").unwrap().wait().unwrap();
    let frag = DocumentFragment::from_document(&doc).unwrap();

    let doc2 = insert_into_fresh_doc(&frag);
    let blocks = doc2.blocks();
    let heading = blocks.iter().find(|b| b.text() == "My Title");
    assert!(heading.is_some(), "Should contain the heading block");
    assert_eq!(heading.unwrap().block_format().heading_level, Some(2));
}

#[test]
fn from_document_with_list() {
    let doc = TextDocument::new();
    doc.set_markdown("- alpha\n- beta\n- gamma").unwrap().wait().unwrap();
    let frag = DocumentFragment::from_document(&doc).unwrap();

    let doc2 = insert_into_fresh_doc(&frag);
    let blocks = doc2.blocks();
    let list_blocks: Vec<_> = blocks.iter().filter(|b| b.list().is_some()).collect();
    assert_eq!(list_blocks.len(), 3, "Expected 3 list items");
    assert_eq!(list_blocks[0].text(), "alpha");
    assert_eq!(list_blocks[1].text(), "beta");
    assert_eq!(list_blocks[2].text(), "gamma");
}

#[test]
fn from_document_with_table() {
    let doc = TextDocument::new();
    doc.set_html("<table><tr><td>A</td><td>B</td></tr><tr><td>C</td><td>D</td></tr></table>")
        .unwrap().wait().unwrap();
    let frag = DocumentFragment::from_document(&doc).unwrap();

    let doc2 = insert_into_fresh_doc(&frag);
    assert!(doc2.stats().table_count >= 1);

    let flow = doc2.flow();
    let table = flow.iter().find_map(|e| match e {
        FlowElement::Table(t) => Some(t.clone()),
        _ => None,
    });
    assert!(table.is_some(), "Flow should contain a table");
    let table = table.unwrap();
    assert_eq!(table.rows(), 2);
    assert_eq!(table.columns(), 2);
    assert_eq!(table.cell(0, 0).unwrap().blocks()[0].text(), "A");
    assert_eq!(table.cell(1, 1).unwrap().blocks()[0].text(), "D");
}

#[test]
fn from_document_with_bold_and_italic() {
    let doc = TextDocument::new();
    doc.set_html("<p><b>bold</b> <em>italic</em></p>").unwrap().wait().unwrap();
    let frag = DocumentFragment::from_document(&doc).unwrap();

    let doc2 = insert_into_fresh_doc(&frag);
    let blocks = doc2.blocks();
    let block = blocks.iter().find(|b| b.text().contains("bold")).expect("Should have bold block");
    let fragments = block.fragments();
    let bold_frag = fragments.iter().find(|f| match f {
        FragmentContent::Text { text, .. } => text == "bold",
        _ => false,
    });
    assert!(bold_frag.is_some());
    if let FragmentContent::Text { format, .. } = bold_frag.unwrap() {
        assert_eq!(format.font_bold, Some(true));
    }
    let italic_frag = fragments.iter().find(|f| match f {
        FragmentContent::Text { text, .. } => text == "italic",
        _ => false,
    });
    assert!(italic_frag.is_some());
    if let FragmentContent::Text { format, .. } = italic_frag.unwrap() {
        assert_eq!(format.font_italic, Some(true));
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// from_html — verified by cursor/block inspection
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn from_html_plain_paragraph() {
    let frag = DocumentFragment::from_html("<p>Simple text</p>");
    let doc = insert_into_fresh_doc(&frag);
    let blocks = doc.blocks();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].text(), "Simple text");
    assert!(blocks[0].list().is_none());
    assert_eq!(blocks[0].block_format().heading_level, None);
}

#[test]
fn from_html_heading_levels() {
    for level in 1u8..=6 {
        let html = format!("<h{0}>Level {0}</h{0}>", level);
        let frag = DocumentFragment::from_html(&html);
        let doc = insert_into_fresh_doc(&frag);
        let blocks = doc.blocks();
        let expected_text = format!("Level {}", level);
        let heading = blocks.iter().find(|b| b.text() == expected_text);
        assert!(heading.is_some(), "h{}: should produce block with text", level);
        assert_eq!(
            heading.unwrap().block_format().heading_level,
            Some(level),
            "h{}: heading_level mismatch",
            level
        );
    }
}

#[test]
fn from_html_bold() {
    let frag = DocumentFragment::from_html("<b>strong text</b>");
    let doc = insert_into_fresh_doc(&frag);
    let frags = doc.blocks()[0].fragments();
    let bold = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "strong text"));
    assert!(bold.is_some());
    if let FragmentContent::Text { format, .. } = bold.unwrap() {
        assert_eq!(format.font_bold, Some(true));
    }
}

#[test]
fn from_html_italic() {
    let frag = DocumentFragment::from_html("<em>emphasis</em>");
    let doc = insert_into_fresh_doc(&frag);
    let frags = doc.blocks()[0].fragments();
    let it = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "emphasis"));
    assert!(it.is_some());
    if let FragmentContent::Text { format, .. } = it.unwrap() {
        assert_eq!(format.font_italic, Some(true));
    }
}

#[test]
fn from_html_underline() {
    let frag = DocumentFragment::from_html("<u>underlined</u>");
    let doc = insert_into_fresh_doc(&frag);
    let frags = doc.blocks()[0].fragments();
    let u = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "underlined"));
    assert!(u.is_some());
    if let FragmentContent::Text { format, .. } = u.unwrap() {
        assert_eq!(format.font_underline, Some(true));
    }
}

#[test]
fn from_html_strikeout() {
    let frag = DocumentFragment::from_html("<s>deleted</s>");
    let doc = insert_into_fresh_doc(&frag);
    let frags = doc.blocks()[0].fragments();
    let s = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "deleted"));
    assert!(s.is_some());
    if let FragmentContent::Text { format, .. } = s.unwrap() {
        assert_eq!(format.font_strikeout, Some(true));
    }
}

#[test]
fn from_html_code_monospace() {
    let frag = DocumentFragment::from_html("<code>snippet</code>");
    let doc = insert_into_fresh_doc(&frag);
    let frags = doc.blocks()[0].fragments();
    let c = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "snippet"));
    assert!(c.is_some());
    if let FragmentContent::Text { format, .. } = c.unwrap() {
        assert_eq!(format.font_family.as_deref(), Some("monospace"));
    }
}

#[test]
fn from_html_link() {
    let frag = DocumentFragment::from_html("<a href=\"https://example.com\">click</a>");
    let doc = insert_into_fresh_doc(&frag);
    let frags = doc.blocks()[0].fragments();
    let a = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "click"));
    assert!(a.is_some());
    if let FragmentContent::Text { format, .. } = a.unwrap() {
        assert_eq!(format.is_anchor, Some(true));
        assert_eq!(format.anchor_href.as_deref(), Some("https://example.com"));
    }
}

#[test]
fn from_html_nested_bold_italic() {
    let frag = DocumentFragment::from_html("<b><em>bold-italic</em></b>");
    let doc = insert_into_fresh_doc(&frag);
    let frags = doc.blocks()[0].fragments();
    let bi = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "bold-italic"));
    assert!(bi.is_some());
    if let FragmentContent::Text { format, .. } = bi.unwrap() {
        assert_eq!(format.font_bold, Some(true));
        assert_eq!(format.font_italic, Some(true));
    }
}

#[test]
fn from_html_unordered_list() {
    let frag = DocumentFragment::from_html("<ul><li>one</li><li>two</li><li>three</li></ul>");
    let doc = insert_into_fresh_doc(&frag);
    let blocks = doc.blocks();
    let list_blocks: Vec<_> = blocks.iter().filter(|b| b.list().is_some()).collect();
    assert_eq!(list_blocks.len(), 3, "Expected 3 list items, got {} (total blocks: {})", list_blocks.len(), blocks.len());
    for (i, expected) in ["one", "two", "three"].iter().enumerate() {
        assert_eq!(list_blocks[i].text(), *expected);
        let style = list_blocks[i].list().unwrap().style();
        assert!(
            matches!(style, ListStyle::Disc | ListStyle::Circle | ListStyle::Square),
            "Expected unordered style, got: {:?}",
            style
        );
    }
}

#[test]
fn from_html_ordered_list() {
    let frag = DocumentFragment::from_html("<ol><li>first</li><li>second</li></ol>");
    let doc = insert_into_fresh_doc(&frag);
    let blocks = doc.blocks();
    let list_blocks: Vec<_> = blocks.iter().filter(|b| b.list().is_some()).collect();
    assert_eq!(list_blocks.len(), 2, "Expected 2 list items");
    assert_eq!(list_blocks[0].text(), "first");
    assert_eq!(list_blocks[1].text(), "second");
    for b in &list_blocks {
        let list = b.list().unwrap();
        assert!(
            matches!(
                list.style(),
                ListStyle::Decimal
                    | ListStyle::LowerAlpha
                    | ListStyle::UpperAlpha
                    | ListStyle::LowerRoman
                    | ListStyle::UpperRoman
            ),
            "Expected ordered style, got: {:?}",
            list.style()
        );
    }
}

#[test]
fn from_html_table_structure() {
    let frag = DocumentFragment::from_html(
        "<table><tr><td>A</td><td>B</td></tr><tr><td>C</td><td>D</td></tr></table>",
    );
    let doc = insert_into_fresh_doc(&frag);
    assert!(doc.stats().table_count >= 1);

    let flow = doc.flow();
    let table = flow
        .iter()
        .find_map(|e| match e {
            FlowElement::Table(t) => Some(t.clone()),
            _ => None,
        })
        .expect("Should contain a table");
    assert_eq!(table.rows(), 2);
    assert_eq!(table.columns(), 2);
    assert_eq!(table.cell(0, 0).unwrap().blocks()[0].text(), "A");
    assert_eq!(table.cell(0, 1).unwrap().blocks()[0].text(), "B");
    assert_eq!(table.cell(1, 0).unwrap().blocks()[0].text(), "C");
    assert_eq!(table.cell(1, 1).unwrap().blocks()[0].text(), "D");
}

#[test]
fn from_html_multi_paragraph() {
    let frag = DocumentFragment::from_html("<p>Para 1</p><p>Para 2</p><p>Para 3</p>");
    let doc = insert_into_fresh_doc(&frag);
    let blocks = doc.blocks();
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0].text(), "Para 1");
    assert_eq!(blocks[1].text(), "Para 2");
    assert_eq!(blocks[2].text(), "Para 3");
}

#[test]
fn from_html_mixed_formatting() {
    let frag = DocumentFragment::from_html(
        "<p><b>B</b> <em>I</em> <u>U</u> <s>S</s> <code>C</code></p>",
    );
    let doc = insert_into_fresh_doc(&frag);
    let frags = doc.blocks()[0].fragments();

    let bold = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "B"));
    assert!(bold.is_some());
    if let FragmentContent::Text { format, .. } = bold.unwrap() {
        assert_eq!(format.font_bold, Some(true));
    }

    let italic = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "I"));
    assert!(italic.is_some());
    if let FragmentContent::Text { format, .. } = italic.unwrap() {
        assert_eq!(format.font_italic, Some(true));
    }

    let underline = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "U"));
    assert!(underline.is_some());
    if let FragmentContent::Text { format, .. } = underline.unwrap() {
        assert_eq!(format.font_underline, Some(true));
    }

    let strike = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "S"));
    assert!(strike.is_some());
    if let FragmentContent::Text { format, .. } = strike.unwrap() {
        assert_eq!(format.font_strikeout, Some(true));
    }

    let code = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "C"));
    assert!(code.is_some());
    if let FragmentContent::Text { format, .. } = code.unwrap() {
        assert_eq!(format.font_family.as_deref(), Some("monospace"));
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// from_markdown — verified by cursor/block inspection
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn from_markdown_plain_text() {
    let frag = DocumentFragment::from_markdown("Hello world");
    let doc = insert_into_fresh_doc(&frag);
    let blocks = doc.blocks();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].text(), "Hello world");
    assert!(blocks[0].list().is_none());
    assert_eq!(blocks[0].block_format().heading_level, None);
}

#[test]
fn from_markdown_heading_levels() {
    for level in 1u8..=6 {
        let md = format!("{} Level {}", "#".repeat(level as usize), level);
        let frag = DocumentFragment::from_markdown(&md);
        let doc = insert_into_fresh_doc(&frag);
        let blocks = doc.blocks();
        let expected_text = format!("Level {}", level);
        let heading = blocks.iter().find(|b| b.text() == expected_text);
        assert!(heading.is_some(), "Level {}: should produce a block", level);
        assert_eq!(
            heading.unwrap().block_format().heading_level,
            Some(level),
            "Level {} heading mismatch",
            level
        );
    }
}

#[test]
fn from_markdown_bold() {
    let frag = DocumentFragment::from_markdown("**bold text**");
    let doc = insert_into_fresh_doc(&frag);
    let frags = doc.blocks()[0].fragments();
    let b = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "bold text"));
    assert!(b.is_some());
    if let FragmentContent::Text { format, .. } = b.unwrap() {
        assert_eq!(format.font_bold, Some(true));
    }
}

#[test]
fn from_markdown_italic() {
    let frag = DocumentFragment::from_markdown("*italic text*");
    let doc = insert_into_fresh_doc(&frag);
    let frags = doc.blocks()[0].fragments();
    let i = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "italic text"));
    assert!(i.is_some());
    if let FragmentContent::Text { format, .. } = i.unwrap() {
        assert_eq!(format.font_italic, Some(true));
    }
}

#[test]
fn from_markdown_bold_italic() {
    let frag = DocumentFragment::from_markdown("***bold-italic***");
    let doc = insert_into_fresh_doc(&frag);
    let frags = doc.blocks()[0].fragments();
    let bi = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "bold-italic"));
    assert!(bi.is_some());
    if let FragmentContent::Text { format, .. } = bi.unwrap() {
        assert_eq!(format.font_bold, Some(true));
        assert_eq!(format.font_italic, Some(true));
    }
}

#[test]
fn from_markdown_strikeout() {
    let frag = DocumentFragment::from_markdown("~~deleted~~");
    let doc = insert_into_fresh_doc(&frag);
    let frags = doc.blocks()[0].fragments();
    let s = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "deleted"));
    assert!(s.is_some());
    if let FragmentContent::Text { format, .. } = s.unwrap() {
        assert_eq!(format.font_strikeout, Some(true));
    }
}

#[test]
fn from_markdown_inline_code() {
    let frag = DocumentFragment::from_markdown("`code`");
    let doc = insert_into_fresh_doc(&frag);
    let frags = doc.blocks()[0].fragments();
    let c = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "code"));
    assert!(c.is_some());
    if let FragmentContent::Text { format, .. } = c.unwrap() {
        assert_eq!(format.font_family.as_deref(), Some("monospace"));
    }
}

#[test]
fn from_markdown_link() {
    let frag = DocumentFragment::from_markdown("[click](https://example.com)");
    let doc = insert_into_fresh_doc(&frag);
    let frags = doc.blocks()[0].fragments();
    let a = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "click"));
    assert!(a.is_some());
    if let FragmentContent::Text { format, .. } = a.unwrap() {
        assert_eq!(format.is_anchor, Some(true));
        assert_eq!(format.anchor_href.as_deref(), Some("https://example.com"));
    }
}

#[test]
fn from_markdown_unordered_list() {
    let frag = DocumentFragment::from_markdown("- alpha\n- beta\n- gamma");
    let doc = insert_into_fresh_doc(&frag);
    let blocks = doc.blocks();
    let list_blocks: Vec<_> = blocks.iter().filter(|b| b.list().is_some()).collect();
    assert_eq!(list_blocks.len(), 3, "Expected 3 list items");
    for (i, expected) in ["alpha", "beta", "gamma"].iter().enumerate() {
        assert_eq!(list_blocks[i].text(), *expected);
        let style = list_blocks[i].list().unwrap().style();
        assert!(
            matches!(style, ListStyle::Disc | ListStyle::Circle | ListStyle::Square),
            "Expected unordered style, got: {:?}",
            style
        );
    }
}

#[test]
fn from_markdown_ordered_list() {
    let frag = DocumentFragment::from_markdown("1. first\n2. second\n3. third");
    let doc = insert_into_fresh_doc(&frag);
    let blocks = doc.blocks();
    let list_blocks: Vec<_> = blocks.iter().filter(|b| b.list().is_some()).collect();
    assert_eq!(list_blocks.len(), 3, "Expected 3 list items");
    assert_eq!(list_blocks[0].text(), "first");
    assert_eq!(list_blocks[1].text(), "second");
    assert_eq!(list_blocks[2].text(), "third");
    for b in &list_blocks {
        let list = b.list().unwrap();
        assert!(
            matches!(
                list.style(),
                ListStyle::Decimal
                    | ListStyle::LowerAlpha
                    | ListStyle::UpperAlpha
                    | ListStyle::LowerRoman
                    | ListStyle::UpperRoman
            ),
            "Expected ordered style, got: {:?}",
            list.style()
        );
    }
}

#[test]
fn from_markdown_table() {
    let md = "| H1 | H2 |\n| --- | --- |\n| c1 | c2 |";
    let frag = DocumentFragment::from_markdown(md);
    let doc = insert_into_fresh_doc(&frag);
    assert!(doc.stats().table_count >= 1);

    let flow = doc.flow();
    let table = flow
        .iter()
        .find_map(|e| match e {
            FlowElement::Table(t) => Some(t.clone()),
            _ => None,
        })
        .expect("Should contain a table");
    assert_eq!(table.rows(), 2);
    assert_eq!(table.columns(), 2);
    assert_eq!(table.cell(0, 0).unwrap().blocks()[0].text(), "H1");
    assert_eq!(table.cell(0, 1).unwrap().blocks()[0].text(), "H2");
    assert_eq!(table.cell(1, 0).unwrap().blocks()[0].text(), "c1");
    assert_eq!(table.cell(1, 1).unwrap().blocks()[0].text(), "c2");
}

#[test]
fn from_markdown_mixed_content() {
    let md = "# Title\n\nA paragraph.\n\n- item A\n- item B\n\n| X | Y |\n| --- | --- |\n| 1 | 2 |";
    let frag = DocumentFragment::from_markdown(md);
    let doc = insert_into_fresh_doc(&frag);

    let blocks = doc.blocks();
    // Heading
    let heading = blocks.iter().find(|b| b.text() == "Title");
    assert!(heading.is_some(), "Should have a Title block");
    assert_eq!(heading.unwrap().block_format().heading_level, Some(1));

    // Plain paragraph
    let para = blocks.iter().find(|b| b.text() == "A paragraph.");
    assert!(para.is_some());
    assert!(para.unwrap().list().is_none());

    // List items
    let list_blocks: Vec<_> = blocks.iter().filter(|b| b.list().is_some()).collect();
    assert_eq!(list_blocks.len(), 2);

    // Table
    assert!(doc.stats().table_count >= 1);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Extract via cursor selection
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn extract_selection_as_fragment() {
    let doc = new_doc_with_text("Hello world");
    let cursor = doc.cursor();
    cursor.select(SelectionType::WordUnderCursor);
    let frag = cursor.selection();
    assert_eq!(frag.to_plain_text(), "Hello");
}

#[test]
fn extract_no_selection_returns_empty() {
    let doc = new_doc_with_text("Hello world");
    let cursor = doc.cursor();
    let frag = cursor.selection();
    assert!(frag.is_empty());
}

#[test]
fn extract_full_document_selection() {
    let doc = new_doc_with_text("Hello world");
    let cursor = doc.cursor();
    cursor.select(SelectionType::Document);
    let frag = cursor.selection();
    assert_eq!(frag.to_plain_text(), "Hello world");
}

#[test]
fn extract_preserves_formatting() {
    let doc = TextDocument::new();
    doc.set_html("<p><b>bold</b> plain</p>").unwrap().wait().unwrap();
    let cursor = doc.cursor();
    cursor.select(SelectionType::Document);
    let frag = cursor.selection();

    let doc2 = insert_into_fresh_doc(&frag);
    let block = doc2.blocks().into_iter().find(|b| b.text().contains("bold")).expect("Should have bold block");
    let frags = block.fragments();
    let bold = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "bold"));
    assert!(bold.is_some());
    if let FragmentContent::Text { format, .. } = bold.unwrap() {
        assert_eq!(format.font_bold, Some(true));
    }
}

#[test]
fn extract_preserves_list() {
    let doc = TextDocument::new();
    doc.set_markdown("- a\n- b").unwrap().wait().unwrap();
    let cursor = doc.cursor();
    cursor.select(SelectionType::Document);
    let frag = cursor.selection();

    let doc2 = insert_into_fresh_doc(&frag);
    let blocks = doc2.blocks();
    let list_blocks: Vec<_> = blocks.iter().filter(|b| b.list().is_some()).collect();
    assert_eq!(list_blocks.len(), 2, "Expected 2 list items");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Insert fragment — verified by cursor/block inspection
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn insert_fragment_from_document() {
    let doc1 = new_doc_with_text("Source text");
    let frag = DocumentFragment::from_document(&doc1).unwrap();

    let doc2 = TextDocument::new();
    let cursor = doc2.cursor();
    cursor.insert_fragment(&frag).unwrap();
    assert_eq!(doc2.to_plain_text().unwrap(), "Source text");
}

#[test]
fn insert_fragment_at_position() {
    let doc = new_doc_with_text("Hello world");
    let frag = DocumentFragment::from_plain_text("beautiful ");
    let cursor = doc.cursor_at(6);
    cursor.insert_fragment(&frag).unwrap();
    let text = doc.to_plain_text().unwrap();
    assert!(text.contains("Hello"));
    assert!(text.contains("beautiful"));
    assert!(text.contains("world"));
}

#[test]
fn insert_empty_fragment_no_change() {
    let doc = new_doc_with_text("Hello");
    let frag = DocumentFragment::from_plain_text("");
    let cursor = doc.cursor();
    cursor.insert_fragment(&frag).unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "Hello");
    assert_eq!(doc.stats().block_count, 1);
}

#[test]
fn insert_fragment_replaces_selection() {
    let doc = new_doc_with_text("Hello world");
    let cursor = doc.cursor();
    cursor.select(SelectionType::Document);
    let frag = DocumentFragment::from_plain_text("Replaced");
    cursor.insert_fragment(&frag).unwrap();
    assert_eq!(doc.to_plain_text().unwrap(), "Replaced");
}

#[test]
fn insert_html_fragment_preserves_formatting() {
    let frag = DocumentFragment::from_html("<b>Bold</b> <em>Italic</em>");
    let doc = insert_into_fresh_doc(&frag);
    let block = doc.blocks().into_iter().find(|b| b.text().contains("Bold")).expect("Should have block with Bold");
    let frags = block.fragments();

    let bold = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "Bold"));
    assert!(bold.is_some());
    if let FragmentContent::Text { format, .. } = bold.unwrap() {
        assert_eq!(format.font_bold, Some(true));
    }

    let italic = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "Italic"));
    assert!(italic.is_some());
    if let FragmentContent::Text { format, .. } = italic.unwrap() {
        assert_eq!(format.font_italic, Some(true));
    }
}

#[test]
fn insert_list_fragment_creates_list_blocks() {
    let frag = DocumentFragment::from_markdown("- item 1\n- item 2\n- item 3");
    let doc = insert_into_fresh_doc(&frag);
    let blocks = doc.blocks();
    let list_blocks: Vec<_> = blocks.iter().filter(|b| b.list().is_some()).collect();
    assert_eq!(list_blocks.len(), 3, "Expected 3 list items");
    for (i, expected) in ["item 1", "item 2", "item 3"].iter().enumerate() {
        assert_eq!(list_blocks[i].text(), *expected);
    }
}

#[test]
fn insert_table_fragment_creates_table() {
    let frag = DocumentFragment::from_html(
        "<table><tr><td>A</td><td>B</td></tr><tr><td>C</td><td>D</td></tr></table>",
    );
    let doc = insert_into_fresh_doc(&frag);
    assert!(doc.stats().table_count >= 1);

    let flow = doc.flow();
    let table = flow
        .iter()
        .find_map(|e| match e {
            FlowElement::Table(t) => Some(t.clone()),
            _ => None,
        })
        .expect("Should contain a table");
    assert_eq!(table.rows(), 2);
    assert_eq!(table.columns(), 2);
}

#[test]
fn cursor_insert_html_merges_inline() {
    let doc = new_doc_with_text("Hello world");
    let cursor = doc.cursor_at(6);
    let block_count_before = doc.stats().block_count;
    cursor.insert_html("<b>beautiful</b>").unwrap();

    let text = doc.to_plain_text().unwrap();
    assert!(text.contains("Hello beautiful"), "got: {}", text);
    assert_eq!(doc.stats().block_count, block_count_before);

    // Verify the bold formatting via block fragments
    let frags = doc.blocks()[0].fragments();
    let bold = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "beautiful"));
    assert!(bold.is_some());
    if let FragmentContent::Text { format, .. } = bold.unwrap() {
        assert_eq!(format.font_bold, Some(true));
    }
}

#[test]
fn cursor_insert_html_multi_paragraph_creates_blocks() {
    let doc = new_doc_with_text("Hello world");
    let block_count_before = doc.stats().block_count;
    let cursor = doc.cursor_at(5);
    cursor.insert_html("<p>A</p><p>B</p>").unwrap();
    assert!(doc.stats().block_count > block_count_before);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Round-trip: extract then insert
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn round_trip_plain_text() {
    let doc1 = new_doc_with_text("The quick brown fox");
    let c1 = doc1.cursor_at(4);
    c1.move_position(MoveOperation::EndOfWord, MoveMode::KeepAnchor, 1);
    let frag = c1.selection();
    assert_eq!(frag.to_plain_text(), "quick");

    let doc2 = new_doc_with_text("The  fox");
    let c2 = doc2.cursor_at(4);
    c2.insert_fragment(&frag).unwrap();
    assert!(doc2.to_plain_text().unwrap().contains("quick"));
}

#[test]
fn round_trip_formatted_document() {
    let doc1 = TextDocument::new();
    doc1.set_html("<p><b>bold</b> <em>italic</em></p>").unwrap().wait().unwrap();
    let frag = DocumentFragment::from_document(&doc1).unwrap();

    let doc2 = insert_into_fresh_doc(&frag);
    let block = doc2.blocks().into_iter().find(|b| b.text().contains("bold")).expect("Should have bold block");
    let frags = block.fragments();
    let bold = frags.iter().find(|f| matches!(f, FragmentContent::Text { text, .. } if text == "bold"));
    assert!(bold.is_some());
    if let FragmentContent::Text { format, .. } = bold.unwrap() {
        assert_eq!(format.font_bold, Some(true));
    }
}

#[test]
fn round_trip_list_document() {
    let doc1 = TextDocument::new();
    doc1.set_markdown("1. first\n2. second").unwrap().wait().unwrap();
    let frag = DocumentFragment::from_document(&doc1).unwrap();

    let doc2 = insert_into_fresh_doc(&frag);
    let blocks = doc2.blocks();
    let list_blocks: Vec<_> = blocks.iter().filter(|b| b.list().is_some()).collect();
    assert_eq!(list_blocks.len(), 2, "Expected 2 list items");
}

#[test]
fn round_trip_table_document() {
    let doc1 = TextDocument::new();
    doc1.set_html("<table><tr><td>X</td><td>Y</td></tr></table>")
        .unwrap().wait().unwrap();
    let frag = DocumentFragment::from_document(&doc1).unwrap();

    let doc2 = insert_into_fresh_doc(&frag);
    assert!(doc2.stats().table_count >= 1);
    let flow = doc2.flow();
    let table = flow
        .iter()
        .find_map(|e| match e {
            FlowElement::Table(t) => Some(t.clone()),
            _ => None,
        })
        .expect("Table should survive round-trip");
    assert_eq!(table.cell(0, 0).unwrap().blocks()[0].text(), "X");
    assert_eq!(table.cell(0, 1).unwrap().blocks()[0].text(), "Y");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// to_html — verified structurally
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn to_html_empty_fragment() {
    let frag = DocumentFragment::new();
    assert_eq!(
        frag.to_html(),
        "<html><head><meta charset=\"utf-8\"></head><body></body></html>"
    );
}

#[test]
fn to_html_single_inline_no_p_wrapper() {
    let frag = DocumentFragment::from_html("<b>bold</b>");
    let html = frag.to_html();
    assert!(html.contains("<strong>bold</strong>"), "got: {}", html);
    assert!(!html.contains("<p>"), "Inline-only should not wrap in <p>, got: {}", html);
}

#[test]
fn to_html_single_plain_block_no_p_wrapper() {
    let doc = new_doc_with_text("Hello world");
    let cursor = doc.cursor();
    cursor.select(SelectionType::Document);
    let frag = cursor.selection();
    let html = frag.to_html();
    assert!(!html.contains("<p>"), "Single plain block → no <p>, got: {}", html);
    assert!(html.contains("Hello world"));
}

#[test]
fn to_html_multi_block_uses_p() {
    let frag = DocumentFragment::from_plain_text("Line 1\nLine 2");
    let html = frag.to_html();
    assert!(html.contains("<p>Line 1</p>"), "got: {}", html);
    assert!(html.contains("<p>Line 2</p>"), "got: {}", html);
}

#[test]
fn to_html_heading_levels() {
    for level in 1..=6 {
        let md = format!("{} Heading", "#".repeat(level));
        let frag = DocumentFragment::from_markdown(&md);
        let html = frag.to_html();
        let open = format!("<h{}>", level);
        let close = format!("</h{}>", level);
        assert!(html.contains(&open) && html.contains(&close), "h{}: got: {}", level, html);
    }
}

#[test]
fn to_html_unordered_list() {
    let frag = DocumentFragment::from_markdown("- X\n- Y\n- Z");
    let html = frag.to_html();
    assert!(html.contains("<ul>"), "got: {}", html);
    assert!(html.contains("</ul>"), "got: {}", html);
    assert_eq!(html.matches("<li>").count(), 3, "got: {}", html);
}

#[test]
fn to_html_ordered_list() {
    let frag = DocumentFragment::from_markdown("1. A\n2. B\n3. C");
    let html = frag.to_html();
    assert!(html.contains("<ol>"), "got: {}", html);
    assert!(html.contains("</ol>"), "got: {}", html);
    assert_eq!(html.matches("<li>").count(), 3, "got: {}", html);
}

#[test]
fn to_html_inline_formatting_all() {
    let frag = DocumentFragment::from_html("<p><b>B</b><em>I</em><u>U</u><s>S</s><code>C</code></p>");
    let html = frag.to_html();
    assert!(html.contains("<strong>B</strong>"), "bold, got: {}", html);
    assert!(html.contains("<em>I</em>"), "italic, got: {}", html);
    assert!(html.contains("<u>U</u>"), "underline, got: {}", html);
    assert!(html.contains("<s>S</s>"), "strikeout, got: {}", html);
    assert!(html.contains("<code>C</code>"), "code, got: {}", html);
}

#[test]
fn to_html_link() {
    let frag = DocumentFragment::from_html("<a href=\"https://x.com\">go</a>");
    let html = frag.to_html();
    assert!(html.contains("<a href=\"https://x.com\">"), "got: {}", html);
    assert!(html.contains("go"));
}

#[test]
fn to_html_escapes_special_chars() {
    let frag = DocumentFragment::from_plain_text("<script>&\"test\"</script>");
    let html = frag.to_html();
    assert!(!html.contains("<script>"), "Should escape, got: {}", html);
    assert!(html.contains("&amp;"), "got: {}", html);
    assert!(html.contains("&lt;"), "got: {}", html);
    assert!(html.contains("&gt;"), "got: {}", html);
    assert!(html.contains("&quot;"), "got: {}", html);
}

#[test]
fn to_html_table() {
    let md = "| H1 | H2 |\n| --- | --- |\n| c1 | c2 |";
    let frag = DocumentFragment::from_markdown(md);
    let html = frag.to_html();
    assert!(html.contains("<table>") && html.contains("</table>"), "got: {}", html);
    assert!(html.contains("<tr>") && html.contains("<td>"), "got: {}", html);
    assert!(html.contains("H1") && html.contains("c2"), "got: {}", html);
}

#[test]
fn to_html_block_with_alignment() {
    let doc = TextDocument::new();
    doc.set_plain_text("centered").unwrap();
    let cursor = doc.cursor();
    cursor
        .set_block_format(&BlockFormat {
            alignment: Some(Alignment::Center),
            ..Default::default()
        })
        .unwrap();
    let frag = DocumentFragment::from_document(&doc).unwrap();
    let html = frag.to_html();
    assert!(html.contains("text-align: center"), "got: {}", html);
}

#[test]
fn to_html_block_with_margins() {
    let doc = TextDocument::new();
    doc.set_plain_text("text").unwrap();
    let cursor = doc.cursor();
    cursor
        .set_block_format(&BlockFormat {
            top_margin: Some(10),
            bottom_margin: Some(20),
            ..Default::default()
        })
        .unwrap();
    let frag = DocumentFragment::from_document(&doc).unwrap();
    let html = frag.to_html();
    assert!(html.contains("margin-top: 10px"), "got: {}", html);
    assert!(html.contains("margin-bottom: 20px"), "got: {}", html);
}

#[test]
fn to_html_mixed_blocks_and_table() {
    let md = "Before\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\nAfter";
    let frag = DocumentFragment::from_markdown(md);
    let html = frag.to_html();
    assert!(html.contains("Before"), "got: {}", html);
    assert!(html.contains("<table>"), "got: {}", html);
    assert!(html.contains("After"), "got: {}", html);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// to_markdown — verified structurally
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn to_markdown_empty_fragment() {
    let frag = DocumentFragment::new();
    assert_eq!(frag.to_markdown(), "");
}

#[test]
fn to_markdown_plain_text() {
    let frag = DocumentFragment::from_plain_text("Hello world");
    assert_eq!(frag.to_markdown(), "Hello world");
}

#[test]
fn to_markdown_multi_paragraph() {
    let frag = DocumentFragment::from_plain_text("Para 1\nPara 2");
    let md = frag.to_markdown();
    assert!(md.contains("Para 1") && md.contains("Para 2"));
    assert!(md.contains("\n\n"), "Paragraphs separated by \\n\\n, got: {:?}", md);
}

#[test]
fn to_markdown_heading_levels() {
    for level in 1..=6 {
        let input = format!("{} H{}", "#".repeat(level), level);
        let frag = DocumentFragment::from_markdown(&input);
        let md = frag.to_markdown();
        let prefix = format!("{} ", "#".repeat(level));
        assert!(md.contains(&prefix), "Level {}, got: {:?}", level, md);
    }
}

#[test]
fn to_markdown_bold() {
    let frag = DocumentFragment::from_html("<b>bold</b>");
    let md = frag.to_markdown();
    assert!(md.contains("**bold**"), "got: {:?}", md);
}

#[test]
fn to_markdown_italic() {
    let frag = DocumentFragment::from_html("<em>italic</em>");
    let md = frag.to_markdown();
    assert!(md.contains("*italic*"), "got: {:?}", md);
}

#[test]
fn to_markdown_bold_italic() {
    let frag = DocumentFragment::from_html("<b><em>both</em></b>");
    let md = frag.to_markdown();
    assert!(md.contains("***both***"), "got: {:?}", md);
}

#[test]
fn to_markdown_strikeout() {
    let frag = DocumentFragment::from_html("<s>deleted</s>");
    let md = frag.to_markdown();
    assert!(md.contains("~~deleted~~"), "got: {:?}", md);
}

#[test]
fn to_markdown_inline_code() {
    let frag = DocumentFragment::from_html("<code>fn main()</code>");
    let md = frag.to_markdown();
    assert!(md.contains("`fn main()`"), "got: {:?}", md);
}

#[test]
fn to_markdown_link() {
    let frag = DocumentFragment::from_html("<a href=\"https://example.com\">click</a>");
    let md = frag.to_markdown();
    assert!(md.contains("[") && md.contains("](https://example.com)"), "got: {:?}", md);
}

#[test]
fn to_markdown_unordered_list() {
    let frag = DocumentFragment::from_markdown("- A\n- B\n- C");
    let md = frag.to_markdown();
    let lines: Vec<&str> = md.lines().collect();
    assert!(lines.len() >= 3, "got: {:?}", md);
    assert!(lines[0].starts_with("- "), "got: {:?}", md);
}

#[test]
fn to_markdown_ordered_list() {
    let frag = DocumentFragment::from_markdown("1. first\n2. second\n3. third");
    let md = frag.to_markdown();
    assert!(md.contains("1. ") && md.contains("2. ") && md.contains("3. "), "got: {:?}", md);
}

#[test]
fn to_markdown_table() {
    let input = "| H1 | H2 |\n| --- | --- |\n| a | b |";
    let frag = DocumentFragment::from_markdown(input);
    let md = frag.to_markdown();
    assert!(md.contains("|") && md.contains("---"), "got: {:?}", md);
    assert!(md.contains("H1") && md.contains("b"), "got: {:?}", md);
}

#[test]
fn to_markdown_escapes_special_chars() {
    let frag = DocumentFragment::from_plain_text("use * and [brackets]");
    let md = frag.to_markdown();
    assert!(md.contains("\\*"), "got: {:?}", md);
    assert!(md.contains("\\["), "got: {:?}", md);
}

#[test]
fn to_markdown_mixed_blocks_and_table() {
    let input = "Before\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\nAfter";
    let frag = DocumentFragment::from_markdown(input);
    let md = frag.to_markdown();
    assert!(md.contains("Before") && md.contains("|") && md.contains("After"), "got: {:?}", md);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Idempotent round-trips: from_x(complex).to_x() == stable
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn html_round_trip_complex() {
    let input = concat!(
        "<h2>Title</h2>",
        "<p><b>bold</b> <em>italic</em> <u>underline</u> <s>strike</s> <code>code</code></p>",
        "<ul><li>item 1</li><li>item 2</li></ul>",
        "<ol><li>first</li><li>second</li></ol>",
        "<table><tr><td>A</td><td>B</td></tr><tr><td>C</td><td>D</td></tr></table>",
        "<p><a href=\"https://example.com\">link</a></p>",
    );
    let frag = DocumentFragment::from_html(input);
    let html = frag.to_html();

    assert!(html.contains("<h2>") && html.contains("Title"), "heading");
    assert!(html.contains("<strong>bold</strong>"), "bold");
    assert!(html.contains("<em>italic</em>"), "italic");
    assert!(html.contains("<u>underline</u>"), "underline");
    assert!(html.contains("<s>strike</s>"), "strike");
    assert!(html.contains("<code>code</code>"), "code");
    assert!(html.contains("<ul>") && html.contains("<li>"), "ul");
    assert!(html.contains("<ol>"), "ol");
    assert!(html.contains("<table>") && html.contains("<td>"), "table");
    assert!(html.contains("<a href=\"https://example.com\">"), "link");

    // Second round-trip should be stable
    let frag2 = DocumentFragment::from_html(&html);
    let html2 = frag2.to_html();
    assert_eq!(html, html2, "HTML round-trip should be stable");
}

#[test]
fn markdown_round_trip_complex() {
    let input = concat!(
        "## Title\n\n",
        "**bold** *italic* ~~strike~~ `code`\n\n",
        "- item 1\n- item 2\n\n",
        "1. first\n2. second\n\n",
        "| H1 | H2 |\n| --- | --- |\n| a | b |",
    );
    let frag = DocumentFragment::from_markdown(input);
    let md = frag.to_markdown();

    assert!(md.contains("## ") && md.contains("Title"), "heading, got: {:?}", md);
    assert!(md.contains("**bold**"), "bold, got: {:?}", md);
    assert!(md.contains("*italic*"), "italic, got: {:?}", md);
    assert!(md.contains("~~strike~~"), "strike, got: {:?}", md);
    assert!(md.contains("`code`"), "code, got: {:?}", md);
    assert!(md.contains("- item 1") && md.contains("- item 2"), "ul, got: {:?}", md);
    assert!(md.contains("1. ") && md.contains("2. "), "ol, got: {:?}", md);
    assert!(md.contains("| H1") && md.contains("| b"), "table, got: {:?}", md);

    // Second round-trip should be stable
    let frag2 = DocumentFragment::from_markdown(&md);
    let md2 = frag2.to_markdown();
    assert_eq!(md, md2, "Markdown round-trip should be stable");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Cross-format: HTML ↔ Markdown via cursor verification
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn html_to_markdown_complex() {
    let frag = DocumentFragment::from_html(concat!(
        "<h2>Title</h2>",
        "<ul><li>x</li><li>y</li></ul>",
        "<table><tr><td>a</td><td>b</td></tr></table>",
    ));
    let md = frag.to_markdown();
    assert!(md.contains("## ") && md.contains("Title"), "heading, got: {:?}", md);
    assert!(md.contains("- "), "list, got: {:?}", md);
    assert!(md.contains("|") && md.contains("a"), "table, got: {:?}", md);
}

#[test]
fn markdown_to_html_complex() {
    let frag = DocumentFragment::from_markdown(concat!(
        "### Sub\n\n",
        "- one\n- two\n\n",
        "**B** *I* `C`\n\n",
        "| A | B |\n| --- | --- |\n| 1 | 2 |",
    ));
    let html = frag.to_html();
    assert!(html.contains("<h3>"), "heading, got: {}", html);
    assert!(html.contains("<ul>"), "list, got: {}", html);
    assert!(html.contains("<strong>"), "bold, got: {}", html);
    assert!(html.contains("<em>"), "italic, got: {}", html);
    assert!(html.contains("<code>"), "code, got: {}", html);
    assert!(html.contains("<table>"), "table, got: {}", html);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Complex document round-trip: build → extract → insert → verify
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn complex_document_fragment_round_trip() {
    // NOTE: uses set_markdown rather than cursor.insert_block() because
    // insert_block() currently inherits heading_level from the previous
    // block, which is a known implementation bug (new blocks after
    // headings should be plain paragraphs).
    let doc = TextDocument::new();
    doc.set_markdown("# Title\n\n**bold text**\n\nA plain paragraph.")
        .unwrap()
        .wait()
        .unwrap();

    // Extract entire document as fragment
    let frag = DocumentFragment::from_document(&doc).unwrap();
    assert!(!frag.is_empty());

    // Insert into fresh document
    let doc2 = insert_into_fresh_doc(&frag);
    let blocks = doc2.blocks();
    assert!(
        blocks.len() >= 3,
        "Expected at least 3 blocks, got {}",
        blocks.len()
    );

    // Verify heading
    let heading = blocks.iter().find(|b| b.text() == "Title");
    assert!(heading.is_some(), "Should have Title block");
    assert_eq!(heading.unwrap().block_format().heading_level, Some(1));

    // Verify bold text
    let bold_block = blocks.iter().find(|b| b.text().contains("bold text"));
    assert!(bold_block.is_some());
    let frags = bold_block.unwrap().fragments();
    let bold = frags.iter().find(|f| {
        matches!(f, FragmentContent::Text { text, .. } if text.contains("bold text"))
    });
    assert!(bold.is_some());
    if let FragmentContent::Text { format, .. } = bold.unwrap() {
        assert_eq!(format.font_bold, Some(true));
    }

    // Verify plain paragraph
    let plain = blocks
        .iter()
        .find(|b| b.text() == "A plain paragraph.");
    assert!(plain.is_some());
    assert_eq!(plain.unwrap().block_format().heading_level, None);
    assert!(plain.unwrap().list().is_none());

    // to_html should contain all content
    let html = frag.to_html();
    assert!(html.contains("<h1>") && html.contains("Title"), "heading in html, got: {}", html);
    assert!(html.contains("<strong>"), "bold in html, got: {}", html);
    assert!(html.contains("A plain paragraph."), "paragraph in html, got: {}", html);

    // to_markdown should contain all content
    let md = frag.to_markdown();
    assert!(md.contains("# ") && md.contains("Title"), "heading in md, got: {:?}", md);
    assert!(md.contains("**bold text**"), "bold in md, got: {:?}", md);
    assert!(md.contains("A plain paragraph"), "paragraph in md, got: {:?}", md);
}

#[test]
fn complex_list_table_round_trip() {
    let doc = TextDocument::new();
    doc.set_markdown(concat!(
        "## Shopping\n\n",
        "- apples\n- bananas\n- cherries\n\n",
        "| Fruit | Price |\n| --- | --- |\n| Apple | 1.50 |\n| Banana | 0.75 |",
    ))
    .unwrap().wait().unwrap();

    let frag = DocumentFragment::from_document(&doc).unwrap();
    let doc2 = insert_into_fresh_doc(&frag);

    // Heading
    let blocks = doc2.blocks();
    let heading = blocks.iter().find(|b| b.text() == "Shopping");
    assert!(heading.is_some(), "Should have Shopping heading");
    assert_eq!(heading.unwrap().block_format().heading_level, Some(2));

    // List
    let list_blocks: Vec<_> = blocks.iter().filter(|b| b.list().is_some()).collect();
    assert_eq!(list_blocks.len(), 3, "3 list items");
    assert_eq!(list_blocks[0].text(), "apples");
    assert_eq!(list_blocks[1].text(), "bananas");
    assert_eq!(list_blocks[2].text(), "cherries");

    // Table
    assert!(doc2.stats().table_count >= 1);
    let flow = doc2.flow();
    let table = flow
        .iter()
        .find_map(|e| match e {
            FlowElement::Table(t) => Some(t.clone()),
            _ => None,
        })
        .expect("Should contain a table");
    assert_eq!(table.rows(), 3);
    assert_eq!(table.columns(), 2);
    assert_eq!(table.cell(0, 0).unwrap().blocks()[0].text(), "Fruit");
    assert_eq!(table.cell(2, 1).unwrap().blocks()[0].text(), "0.75");

    // Verify to_markdown preserves everything
    let md = frag.to_markdown();
    assert!(md.contains("## ") && md.contains("Shopping"), "heading, got: {:?}", md);
    assert!(md.contains("- apples"), "list, got: {:?}", md);
    assert!(md.contains("| Fruit"), "table, got: {:?}", md);

    // Verify to_html preserves everything
    let html = frag.to_html();
    assert!(html.contains("<h2>"), "heading, got: {}", html);
    assert!(html.contains("<ul>"), "list, got: {}", html);
    assert!(html.contains("<table>"), "table, got: {}", html);
}
