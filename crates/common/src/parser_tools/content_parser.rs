use crate::entities::ListStyle;

/// A parsed inline span with formatting info
#[derive(Debug, Clone, Default)]
pub struct ParsedSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    pub code: bool,
    pub link_href: Option<String>,
}

/// A parsed block (paragraph, heading, list item, code block)
#[derive(Debug, Clone)]
pub struct ParsedBlock {
    pub spans: Vec<ParsedSpan>,
    pub heading_level: Option<i64>,
    pub list_style: Option<ListStyle>,
    pub is_code_block: bool,
}

// ─── Markdown parsing ────────────────────────────────────────────────

pub fn parse_markdown(markdown: &str) -> Vec<ParsedBlock> {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

    let options =
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(markdown, options);

    let mut blocks: Vec<ParsedBlock> = Vec::new();
    let mut current_spans: Vec<ParsedSpan> = Vec::new();
    let mut current_heading: Option<i64> = None;
    let mut current_list_style: Option<ListStyle> = None;
    let mut is_code_block = false;
    let mut in_block = false;

    // Formatting state stack
    let mut bold = false;
    let mut italic = false;
    let mut strikeout = false;
    let mut link_href: Option<String> = None;

    // List style stack for nested lists
    let mut list_stack: Vec<Option<ListStyle>> = Vec::new();

    for event in parser {
        match event {
            Event::Start(Tag::Paragraph) => {
                in_block = true;
                current_heading = None;
                is_code_block = false;
            }
            Event::End(TagEnd::Paragraph) => {
                if !current_spans.is_empty() || in_block {
                    blocks.push(ParsedBlock {
                        spans: std::mem::take(&mut current_spans),
                        heading_level: current_heading.take(),
                        list_style: current_list_style.clone(),
                        is_code_block: false,
                    });
                }
                in_block = false;
                current_list_style = None;
            }
            Event::Start(Tag::Heading { level, .. }) => {
                in_block = true;
                current_heading = Some(heading_level_to_i64(level));
                is_code_block = false;
            }
            Event::End(TagEnd::Heading(_)) => {
                blocks.push(ParsedBlock {
                    spans: std::mem::take(&mut current_spans),
                    heading_level: current_heading.take(),
                    list_style: None,
                    is_code_block: false,
                });
                in_block = false;
            }
            Event::Start(Tag::List(ordered)) => {
                let style = if ordered.is_some() {
                    Some(ListStyle::Decimal)
                } else {
                    Some(ListStyle::Disc)
                };
                list_stack.push(style);
            }
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
            }
            Event::Start(Tag::Item) => {
                in_block = true;
                current_list_style = list_stack.last().cloned().flatten();
            }
            Event::End(TagEnd::Item) => {
                // The paragraph inside the item will have already been flushed,
                // but if there was no inner paragraph (tight list), flush now.
                if !current_spans.is_empty() {
                    blocks.push(ParsedBlock {
                        spans: std::mem::take(&mut current_spans),
                        heading_level: None,
                        list_style: current_list_style.clone(),
                        is_code_block: false,
                    });
                }
                in_block = false;
                current_list_style = None;
            }
            Event::Start(Tag::CodeBlock(_)) => {
                in_block = true;
                is_code_block = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                blocks.push(ParsedBlock {
                    spans: std::mem::take(&mut current_spans),
                    heading_level: None,
                    list_style: None,
                    is_code_block: true,
                });
                in_block = false;
                is_code_block = false;
            }
            Event::Start(Tag::Emphasis) => {
                italic = true;
            }
            Event::End(TagEnd::Emphasis) => {
                italic = false;
            }
            Event::Start(Tag::Strong) => {
                bold = true;
            }
            Event::End(TagEnd::Strong) => {
                bold = false;
            }
            Event::Start(Tag::Strikethrough) => {
                strikeout = true;
            }
            Event::End(TagEnd::Strikethrough) => {
                strikeout = false;
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                link_href = Some(dest_url.to_string());
            }
            Event::End(TagEnd::Link) => {
                link_href = None;
            }
            Event::Text(text) => {
                if !in_block {
                    // Bare text outside any block — create an implicit paragraph
                    in_block = true;
                }
                current_spans.push(ParsedSpan {
                    text: text.to_string(),
                    bold,
                    italic,
                    underline: false,
                    strikeout,
                    code: is_code_block,
                    link_href: link_href.clone(),
                });
            }
            Event::Code(text) => {
                if !in_block {
                    in_block = true;
                }
                current_spans.push(ParsedSpan {
                    text: text.to_string(),
                    bold,
                    italic,
                    underline: false,
                    strikeout,
                    code: true,
                    link_href: link_href.clone(),
                });
            }
            Event::SoftBreak => {
                // Add a space
                current_spans.push(ParsedSpan {
                    text: " ".to_string(),
                    bold,
                    italic,
                    underline: false,
                    strikeout,
                    code: false,
                    link_href: link_href.clone(),
                });
            }
            Event::HardBreak => {
                // Finalize current block
                if !current_spans.is_empty() || in_block {
                    blocks.push(ParsedBlock {
                        spans: std::mem::take(&mut current_spans),
                        heading_level: current_heading.take(),
                        list_style: current_list_style.clone(),
                        is_code_block,
                    });
                }
            }
            _ => {}
        }
    }

    // Flush any remaining content
    if !current_spans.is_empty() {
        blocks.push(ParsedBlock {
            spans: std::mem::take(&mut current_spans),
            heading_level: current_heading,
            list_style: current_list_style,
            is_code_block,
        });
    }

    // If no blocks were parsed, create a single empty paragraph
    if blocks.is_empty() {
        blocks.push(ParsedBlock {
            spans: vec![ParsedSpan {
                text: String::new(),
                ..Default::default()
            }],
            heading_level: None,
            list_style: None,
            is_code_block: false,
        });
    }

    blocks
}

fn heading_level_to_i64(level: pulldown_cmark::HeadingLevel) -> i64 {
    use pulldown_cmark::HeadingLevel;
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

// ─── HTML parsing ────────────────────────────────────────────────────

use scraper::Node;

pub fn parse_html(html: &str) -> Vec<ParsedBlock> {
    use scraper::Html;

    let fragment = Html::parse_fragment(html);
    let mut blocks: Vec<ParsedBlock> = Vec::new();

    // Walk the DOM tree starting from the root
    let root = fragment.root_element();

    #[derive(Clone, Default)]
    struct FmtState {
        bold: bool,
        italic: bool,
        underline: bool,
        strikeout: bool,
        code: bool,
        link_href: Option<String>,
    }

    fn walk_node(
        node: ego_tree::NodeRef<Node>,
        state: &FmtState,
        blocks: &mut Vec<ParsedBlock>,
        current_list_style: &Option<ListStyle>,
    ) {
        match node.value() {
            Node::Element(el) => {
                let tag = el.name();
                let mut new_state = state.clone();
                let mut new_list_style = current_list_style.clone();

                // Determine if this is a block-level element
                let is_block_tag = matches!(
                    tag,
                    "p" | "div"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                        | "li"
                        | "pre"
                        | "br"
                        | "blockquote"
                );

                // Update formatting state
                match tag {
                    "b" | "strong" => new_state.bold = true,
                    "i" | "em" => new_state.italic = true,
                    "u" | "ins" => new_state.underline = true,
                    "s" | "del" | "strike" => new_state.strikeout = true,
                    "code" => new_state.code = true,
                    "a" => {
                        if let Some(href) = el.attr("href") {
                            new_state.link_href = Some(href.to_string());
                        }
                    }
                    "ul" => {
                        new_list_style = Some(ListStyle::Disc);
                    }
                    "ol" => {
                        new_list_style = Some(ListStyle::Decimal);
                    }
                    _ => {}
                }

                // Determine heading level
                let heading_level = match tag {
                    "h1" => Some(1),
                    "h2" => Some(2),
                    "h3" => Some(3),
                    "h4" => Some(4),
                    "h5" => Some(5),
                    "h6" => Some(6),
                    _ => None,
                };

                let is_code_block = tag == "pre";

                if tag == "br" {
                    // <br> creates a new block
                    blocks.push(ParsedBlock {
                        spans: vec![ParsedSpan {
                            text: String::new(),
                            ..Default::default()
                        }],
                        heading_level: None,
                        list_style: None,
                        is_code_block: false,
                    });
                    return;
                }

                if is_block_tag && tag != "br" {
                    // Start collecting spans for a new block
                    let mut spans: Vec<ParsedSpan> = Vec::new();
                    collect_inline_spans(node, &new_state, &mut spans, &new_list_style, blocks);

                    let list_style_for_block = if tag == "li" {
                        new_list_style.clone()
                    } else {
                        None
                    };

                    if !spans.is_empty() || heading_level.is_some() {
                        blocks.push(ParsedBlock {
                            spans,
                            heading_level,
                            list_style: list_style_for_block,
                            is_code_block,
                        });
                    }
                } else if matches!(tag, "ul" | "ol" | "table" | "thead" | "tbody" | "tr") {
                    // Container elements: recurse into children
                    for child in node.children() {
                        walk_node(child, &new_state, blocks, &new_list_style);
                    }
                } else {
                    // Inline element or unknown: recurse
                    for child in node.children() {
                        walk_node(child, &new_state, blocks, current_list_style);
                    }
                }
            }
            Node::Text(text) => {
                let t = text.text.to_string();
                let trimmed = t.trim();
                if !trimmed.is_empty() {
                    // Bare text not in a block — create a paragraph
                    blocks.push(ParsedBlock {
                        spans: vec![ParsedSpan {
                            text: trimmed.to_string(),
                            bold: state.bold,
                            italic: state.italic,
                            underline: state.underline,
                            strikeout: state.strikeout,
                            code: state.code,
                            link_href: state.link_href.clone(),
                        }],
                        heading_level: None,
                        list_style: None,
                        is_code_block: false,
                    });
                }
            }
            _ => {
                // Document, Comment, etc. — recurse children
                for child in node.children() {
                    walk_node(child, state, blocks, current_list_style);
                }
            }
        }
    }

    /// Collect inline spans from a block-level element's children.
    /// If a nested block-level element is encountered, it is flushed as a
    /// separate block.
    fn collect_inline_spans(
        node: ego_tree::NodeRef<Node>,
        state: &FmtState,
        spans: &mut Vec<ParsedSpan>,
        current_list_style: &Option<ListStyle>,
        blocks: &mut Vec<ParsedBlock>,
    ) {
        for child in node.children() {
            match child.value() {
                Node::Text(text) => {
                    let t = text.text.to_string();
                    if !t.is_empty() {
                        spans.push(ParsedSpan {
                            text: t,
                            bold: state.bold,
                            italic: state.italic,
                            underline: state.underline,
                            strikeout: state.strikeout,
                            code: state.code,
                            link_href: state.link_href.clone(),
                        });
                    }
                }
                Node::Element(el) => {
                    let tag = el.name();
                    let mut new_state = state.clone();

                    match tag {
                        "b" | "strong" => new_state.bold = true,
                        "i" | "em" => new_state.italic = true,
                        "u" | "ins" => new_state.underline = true,
                        "s" | "del" | "strike" => new_state.strikeout = true,
                        "code" => new_state.code = true,
                        "a" => {
                            if let Some(href) = el.attr("href") {
                                new_state.link_href = Some(href.to_string());
                            }
                        }
                        _ => {}
                    }

                    // Check for nested block elements
                    let nested_block = matches!(
                        tag,
                        "p" | "div"
                            | "h1"
                            | "h2"
                            | "h3"
                            | "h4"
                            | "h5"
                            | "h6"
                            | "li"
                            | "pre"
                            | "blockquote"
                            | "ul"
                            | "ol"
                    );

                    if tag == "br" {
                        // br within a block: treat as splitting into new block
                        // For simplicity, just add a newline to current span
                        spans.push(ParsedSpan {
                            text: String::new(),
                            ..Default::default()
                        });
                    } else if nested_block {
                        // Flush as separate block
                        walk_node(child, &new_state, blocks, current_list_style);
                    } else {
                        // Inline element: recurse
                        collect_inline_spans(child, &new_state, spans, current_list_style, blocks);
                    }
                }
                _ => {}
            }
        }
    }

    let initial_state = FmtState::default();
    for child in root.children() {
        walk_node(child, &initial_state, &mut blocks, &None);
    }

    // If no blocks were parsed, create a single empty paragraph
    if blocks.is_empty() {
        blocks.push(ParsedBlock {
            spans: vec![ParsedSpan {
                text: String::new(),
                ..Default::default()
            }],
            heading_level: None,
            list_style: None,
            is_code_block: false,
        });
    }

    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_markdown_simple_paragraph() {
        let blocks = parse_markdown("Hello **world**");
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].spans.len() >= 2);
        // "Hello " is plain, "world" is bold
        let plain_span = blocks[0]
            .spans
            .iter()
            .find(|s| s.text.contains("Hello"))
            .unwrap();
        assert!(!plain_span.bold);
        let bold_span = blocks[0].spans.iter().find(|s| s.text == "world").unwrap();
        assert!(bold_span.bold);
    }

    #[test]
    fn test_parse_markdown_heading() {
        let blocks = parse_markdown("# Title");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].heading_level, Some(1));
        assert_eq!(blocks[0].spans[0].text, "Title");
    }

    #[test]
    fn test_parse_markdown_list() {
        let blocks = parse_markdown("- item1\n- item2");
        assert!(blocks.len() >= 2);
        assert_eq!(blocks[0].list_style, Some(ListStyle::Disc));
        assert_eq!(blocks[1].list_style, Some(ListStyle::Disc));
    }

    #[test]
    fn test_parse_html_simple() {
        let blocks = parse_html("<p>Hello <b>world</b></p>");
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].spans.len() >= 2);
        let bold_span = blocks[0].spans.iter().find(|s| s.text == "world").unwrap();
        assert!(bold_span.bold);
    }

    #[test]
    fn test_parse_html_multiple_paragraphs() {
        let blocks = parse_html("<p>A</p><p>B</p>");
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn test_parse_html_heading() {
        let blocks = parse_html("<h2>Subtitle</h2>");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].heading_level, Some(2));
    }

    #[test]
    fn test_parse_html_list() {
        let blocks = parse_html("<ul><li>one</li><li>two</li></ul>");
        assert!(blocks.len() >= 2);
        assert_eq!(blocks[0].list_style, Some(ListStyle::Disc));
    }

    #[test]
    fn test_parse_markdown_code_block() {
        let blocks = parse_markdown("```\nfn main() {}\n```");
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].is_code_block);
        assert!(blocks[0].spans[0].code);
    }

    #[test]
    fn test_parse_markdown_nested_formatting() {
        let blocks = parse_markdown("***bold italic***");
        assert_eq!(blocks.len(), 1);
        let span = &blocks[0].spans[0];
        assert!(span.bold);
        assert!(span.italic);
    }

    #[test]
    fn test_parse_markdown_link() {
        let blocks = parse_markdown("[click](http://example.com)");
        assert_eq!(blocks.len(), 1);
        let span = &blocks[0].spans[0];
        assert_eq!(span.text, "click");
        assert_eq!(span.link_href, Some("http://example.com".to_string()));
    }

    #[test]
    fn test_parse_markdown_empty() {
        let blocks = parse_markdown("");
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].spans[0].text.is_empty());
    }

    #[test]
    fn test_parse_html_empty() {
        let blocks = parse_html("");
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].spans[0].text.is_empty());
    }

    #[test]
    fn test_parse_html_nested_formatting() {
        let blocks = parse_html("<p><b><i>bold italic</i></b></p>");
        assert_eq!(blocks.len(), 1);
        let span = &blocks[0].spans[0];
        assert!(span.bold);
        assert!(span.italic);
    }

    #[test]
    fn test_parse_html_link() {
        let blocks = parse_html("<p><a href=\"http://example.com\">click</a></p>");
        assert_eq!(blocks.len(), 1);
        let span = &blocks[0].spans[0];
        assert_eq!(span.text, "click");
        assert_eq!(span.link_href, Some("http://example.com".to_string()));
    }

    #[test]
    fn test_parse_html_ordered_list() {
        let blocks = parse_html("<ol><li>first</li><li>second</li></ol>");
        assert!(blocks.len() >= 2);
        assert_eq!(blocks[0].list_style, Some(ListStyle::Decimal));
    }

    #[test]
    fn test_parse_markdown_ordered_list() {
        let blocks = parse_markdown("1. first\n2. second");
        assert!(blocks.len() >= 2);
        assert_eq!(blocks[0].list_style, Some(ListStyle::Decimal));
    }

    #[test]
    fn test_parse_html_blockquote_nested() {
        let blocks = parse_html("<p>before</p><blockquote>quoted</blockquote><p>after</p>");
        assert!(blocks.len() >= 3);
    }
}
