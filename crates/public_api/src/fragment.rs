//! DocumentFragment — format-agnostic rich text interchange type.

use crate::{InlineContent, ListStyle};
use frontend::common::parser_tools::fragment_schema::{
    FragmentBlock, FragmentData, FragmentElement,
};

/// A piece of rich text that can be inserted into a [`TextDocument`](crate::TextDocument).
///
/// `DocumentFragment` is the clipboard/interchange type. It carries
/// blocks, inline elements, and formatting in a format-agnostic
/// internal representation.
#[derive(Debug, Clone)]
pub struct DocumentFragment {
    data: String,
    plain_text: String,
}

impl DocumentFragment {
    /// Create an empty fragment.
    pub fn new() -> Self {
        Self {
            data: String::new(),
            plain_text: String::new(),
        }
    }

    /// Create a fragment from plain text.
    ///
    /// Builds valid fragment data so the fragment can be inserted via
    /// [`TextCursor::insert_fragment`](crate::TextCursor::insert_fragment).
    pub fn from_plain_text(text: &str) -> Self {
        let blocks: Vec<FragmentBlock> = text
            .split('\n')
            .map(|line| FragmentBlock {
                plain_text: line.to_string(),
                elements: vec![FragmentElement {
                    content: InlineContent::Text(line.to_string()),
                    fmt_font_family: None,
                    fmt_font_point_size: None,
                    fmt_font_weight: None,
                    fmt_font_bold: None,
                    fmt_font_italic: None,
                    fmt_font_underline: None,
                    fmt_font_overline: None,
                    fmt_font_strikeout: None,
                    fmt_letter_spacing: None,
                    fmt_word_spacing: None,
                    fmt_anchor_href: None,
                    fmt_anchor_names: vec![],
                    fmt_is_anchor: None,
                    fmt_tooltip: None,
                    fmt_underline_style: None,
                    fmt_vertical_alignment: None,
                }],
                heading_level: None,
                list: None,
                alignment: None,
                indent: None,
                text_indent: None,
                marker: None,
                top_margin: None,
                bottom_margin: None,
                left_margin: None,
                right_margin: None,
                tab_positions: vec![],
                line_height: None,
                non_breakable_lines: None,
                direction: None,
                background_color: None,
            })
            .collect();

        let data = serde_json::to_string(&FragmentData { blocks })
            .expect("fragment serialization should not fail");

        Self {
            data,
            plain_text: text.to_string(),
        }
    }

    /// Create a fragment from HTML.
    pub fn from_html(html: &str) -> Self {
        let parsed = frontend::common::parser_tools::content_parser::parse_html(html);
        parsed_blocks_to_fragment(parsed)
    }

    /// Create a fragment from Markdown.
    pub fn from_markdown(markdown: &str) -> Self {
        let parsed = frontend::common::parser_tools::content_parser::parse_markdown(markdown);
        parsed_blocks_to_fragment(parsed)
    }

    /// Create a fragment from an entire document.
    pub fn from_document(doc: &crate::TextDocument) -> crate::Result<Self> {
        let inner = doc.inner.lock();
        let char_count = {
            let stats =
                frontend::commands::document_inspection_commands::get_document_stats(&inner.ctx)?;
            crate::convert::to_usize(stats.character_count)
        };
        let dto = frontend::document_inspection::ExtractFragmentDto {
            position: 0,
            anchor: crate::convert::to_i64(char_count),
        };
        let result =
            frontend::commands::document_inspection_commands::extract_fragment(&inner.ctx, &dto)?;
        Ok(Self::from_raw(result.fragment_data, result.plain_text))
    }

    /// Create a fragment from the serialized internal format.
    pub(crate) fn from_raw(data: String, plain_text: String) -> Self {
        Self { data, plain_text }
    }

    /// Export the fragment as plain text.
    pub fn to_plain_text(&self) -> &str {
        &self.plain_text
    }

    /// Export the fragment as HTML.
    pub fn to_html(&self) -> String {
        if self.data.is_empty() {
            return String::from("<html><head><meta charset=\"utf-8\"></head><body></body></html>");
        }

        let fragment_data: FragmentData = match serde_json::from_str(&self.data) {
            Ok(d) => d,
            Err(_) => {
                return String::from(
                    "<html><head><meta charset=\"utf-8\"></head><body></body></html>",
                );
            }
        };

        let mut body = String::new();
        let blocks = &fragment_data.blocks;

        // Single inline-only block: emit inline HTML without block wrapper
        if blocks.len() == 1 && blocks[0].is_inline_only() {
            push_inline_html(&mut body, &blocks[0].elements);
            return format!(
                "<html><head><meta charset=\"utf-8\"></head><body>{}</body></html>",
                body
            );
        }

        let mut i = 0;

        while i < blocks.len() {
            let block = &blocks[i];

            if let Some(ref list) = block.list {
                let is_ordered = is_ordered_list_style(&list.style);
                let list_tag = if is_ordered { "ol" } else { "ul" };
                body.push('<');
                body.push_str(list_tag);
                body.push('>');

                while i < blocks.len() {
                    let b = &blocks[i];
                    match &b.list {
                        Some(l) if is_ordered_list_style(&l.style) == is_ordered => {
                            body.push_str("<li>");
                            push_inline_html(&mut body, &b.elements);
                            body.push_str("</li>");
                            i += 1;
                        }
                        _ => break,
                    }
                }

                body.push_str("</");
                body.push_str(list_tag);
                body.push('>');
            } else if let Some(level) = block.heading_level {
                let n = level.clamp(1, 6);
                body.push_str(&format!("<h{}>", n));
                push_inline_html(&mut body, &block.elements);
                body.push_str(&format!("</h{}>", n));
                i += 1;
            } else {
                // Emit block-level formatting as inline styles (ISSUE-19)
                let style = block_style_attr(block);
                if style.is_empty() {
                    body.push_str("<p>");
                } else {
                    body.push_str(&format!("<p style=\"{}\">", style));
                }
                push_inline_html(&mut body, &block.elements);
                body.push_str("</p>");
                i += 1;
            }
        }

        format!(
            "<html><head><meta charset=\"utf-8\"></head><body>{}</body></html>",
            body
        )
    }

    /// Export the fragment as Markdown.
    pub fn to_markdown(&self) -> String {
        if self.data.is_empty() {
            return String::new();
        }

        let fragment_data: FragmentData = match serde_json::from_str(&self.data) {
            Ok(d) => d,
            Err(_) => return String::new(),
        };

        let mut parts: Vec<String> = Vec::new();
        let mut prev_was_list = false;
        let mut list_counter: u32 = 0;

        for block in &fragment_data.blocks {
            let inline_text = render_inline_markdown(&block.elements);
            let is_list = block.list.is_some();

            // Markdown indent prefix from block indent level (ISSUE-19)
            let indent_prefix = match block.indent {
                Some(n) if n > 0 => "  ".repeat(n as usize),
                _ => String::new(),
            };

            if let Some(level) = block.heading_level {
                let n = level.clamp(1, 6) as usize;
                let prefix = "#".repeat(n);
                parts.push(format!("{} {}", prefix, inline_text));
                prev_was_list = false;
                list_counter = 0;
            } else if let Some(ref list) = block.list {
                let is_ordered = is_ordered_list_style(&list.style);
                if !prev_was_list {
                    list_counter = 0;
                }
                if is_ordered {
                    list_counter += 1;
                    parts.push(format!(
                        "{}{}. {}",
                        indent_prefix, list_counter, inline_text
                    ));
                } else {
                    parts.push(format!("{}- {}", indent_prefix, inline_text));
                }
                prev_was_list = true;
            } else {
                // Prepend blockquote-style indent for indented paragraphs
                if indent_prefix.is_empty() {
                    parts.push(inline_text);
                } else {
                    parts.push(format!("{}{}", indent_prefix, inline_text));
                }
                prev_was_list = false;
                list_counter = 0;
            }

            if !is_list {
                prev_was_list = false;
            }
        }

        // Join: list items with \n, others with \n\n
        let mut result = String::new();
        let blocks = &fragment_data.blocks;
        for (idx, part) in parts.iter().enumerate() {
            if idx > 0 {
                let prev_is_list = blocks[idx - 1].list.is_some();
                let curr_is_list = blocks[idx].list.is_some();
                if prev_is_list && curr_is_list {
                    result.push('\n');
                } else {
                    result.push_str("\n\n");
                }
            }
            result.push_str(part);
        }

        result
    }

    /// Returns true if the fragment contains no text or elements.
    pub fn is_empty(&self) -> bool {
        self.plain_text.is_empty()
    }

    /// Returns the serialized internal representation.
    pub(crate) fn raw_data(&self) -> &str {
        &self.data
    }
}

impl Default for DocumentFragment {
    fn default() -> Self {
        Self::new()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Shared helpers (used by both to_html and to_markdown)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn is_ordered_list_style(style: &ListStyle) -> bool {
    matches!(
        style,
        ListStyle::Decimal
            | ListStyle::LowerAlpha
            | ListStyle::UpperAlpha
            | ListStyle::LowerRoman
            | ListStyle::UpperRoman
    )
}

// ── HTML helpers ────────────────────────────────────────────────

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

/// Build a CSS `style` attribute value from block-level formatting (ISSUE-19).
fn block_style_attr(block: &FragmentBlock) -> String {
    use crate::Alignment;

    let mut parts = Vec::new();
    if let Some(ref alignment) = block.alignment {
        let value = match alignment {
            Alignment::Left => "left",
            Alignment::Right => "right",
            Alignment::Center => "center",
            Alignment::Justify => "justify",
        };
        parts.push(format!("text-align: {}", value));
    }
    if let Some(n) = block.indent
        && n > 0
    {
        parts.push(format!("margin-left: {}em", n));
    }
    if let Some(px) = block.text_indent
        && px != 0
    {
        parts.push(format!("text-indent: {}px", px));
    }
    if let Some(px) = block.top_margin {
        parts.push(format!("margin-top: {}px", px));
    }
    if let Some(px) = block.bottom_margin {
        parts.push(format!("margin-bottom: {}px", px));
    }
    if let Some(px) = block.left_margin {
        parts.push(format!("margin-left: {}px", px));
    }
    if let Some(px) = block.right_margin {
        parts.push(format!("margin-right: {}px", px));
    }
    parts.join("; ")
}

fn push_inline_html(out: &mut String, elements: &[FragmentElement]) {
    for elem in elements {
        let text = match &elem.content {
            InlineContent::Text(t) => escape_html(t),
            InlineContent::Image {
                name,
                width,
                height,
                ..
            } => {
                format!(
                    "<img src=\"{}\" width=\"{}\" height=\"{}\">",
                    escape_html(name),
                    width,
                    height
                )
            }
            InlineContent::Empty => String::new(),
        };

        let is_monospace = elem
            .fmt_font_family
            .as_deref()
            .is_some_and(|f| f == "monospace");
        let is_bold = elem.fmt_font_bold.unwrap_or(false);
        let is_italic = elem.fmt_font_italic.unwrap_or(false);
        let is_underline = elem.fmt_font_underline.unwrap_or(false);
        let is_strikeout = elem.fmt_font_strikeout.unwrap_or(false);
        let is_anchor = elem.fmt_is_anchor.unwrap_or(false);

        let mut result = text;

        if is_monospace {
            result = format!("<code>{}</code>", result);
        }
        if is_bold {
            result = format!("<strong>{}</strong>", result);
        }
        if is_italic {
            result = format!("<em>{}</em>", result);
        }
        if is_underline {
            result = format!("<u>{}</u>", result);
        }
        if is_strikeout {
            result = format!("<s>{}</s>", result);
        }
        if is_anchor && let Some(ref href) = elem.fmt_anchor_href {
            result = format!("<a href=\"{}\">{}</a>", escape_html(href), result);
        }

        out.push_str(&result);
    }
}

// ── Markdown helpers ────────────────────────────────────────────

fn escape_markdown(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(
            c,
            '\\' | '`'
                | '*'
                | '_'
                | '{'
                | '}'
                | '['
                | ']'
                | '('
                | ')'
                | '#'
                | '+'
                | '-'
                | '.'
                | '!'
                | '|'
                | '~'
                | '<'
                | '>'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn render_inline_markdown(elements: &[FragmentElement]) -> String {
    let mut out = String::new();
    for elem in elements {
        let raw_text = match &elem.content {
            InlineContent::Text(t) => t.clone(),
            InlineContent::Image { name, .. } => format!("![{}]({})", name, name),
            InlineContent::Empty => String::new(),
        };

        let is_monospace = elem
            .fmt_font_family
            .as_deref()
            .is_some_and(|f| f == "monospace");
        let is_bold = elem.fmt_font_bold.unwrap_or(false);
        let is_italic = elem.fmt_font_italic.unwrap_or(false);
        let is_strikeout = elem.fmt_font_strikeout.unwrap_or(false);
        let is_anchor = elem.fmt_is_anchor.unwrap_or(false);

        if is_monospace {
            out.push('`');
            out.push_str(&raw_text);
            out.push('`');
        } else {
            let mut text = escape_markdown(&raw_text);
            if is_bold && is_italic {
                text = format!("***{}***", text);
            } else if is_bold {
                text = format!("**{}**", text);
            } else if is_italic {
                text = format!("*{}*", text);
            }
            if is_strikeout {
                text = format!("~~{}~~", text);
            }
            if is_anchor {
                let href = elem.fmt_anchor_href.as_deref().unwrap_or("");
                out.push_str(&format!("[{}]({})", text, href));
            } else {
                out.push_str(&text);
            }
        }
    }
    out
}

// ── Fragment construction from parsed content ───────────────────

/// Convert parsed blocks (from HTML or Markdown parser) into a `DocumentFragment`.
fn parsed_blocks_to_fragment(
    parsed: Vec<frontend::common::parser_tools::content_parser::ParsedBlock>,
) -> DocumentFragment {
    use frontend::common::parser_tools::fragment_schema::FragmentList;

    let blocks: Vec<FragmentBlock> = parsed
        .into_iter()
        .map(|pb| {
            let elements: Vec<FragmentElement> = pb
                .spans
                .iter()
                .map(|span| {
                    let content = InlineContent::Text(span.text.clone());
                    let fmt_font_family = if span.code {
                        Some("monospace".into())
                    } else {
                        None
                    };
                    let fmt_font_bold = if span.bold { Some(true) } else { None };
                    let fmt_font_italic = if span.italic { Some(true) } else { None };
                    let fmt_font_underline = if span.underline { Some(true) } else { None };
                    let fmt_font_strikeout = if span.strikeout { Some(true) } else { None };
                    let (fmt_anchor_href, fmt_is_anchor) = if let Some(ref href) = span.link_href {
                        (Some(href.clone()), Some(true))
                    } else {
                        (None, None)
                    };

                    FragmentElement {
                        content,
                        fmt_font_family,
                        fmt_font_point_size: None,
                        fmt_font_weight: None,
                        fmt_font_bold,
                        fmt_font_italic,
                        fmt_font_underline,
                        fmt_font_overline: None,
                        fmt_font_strikeout,
                        fmt_letter_spacing: None,
                        fmt_word_spacing: None,
                        fmt_anchor_href,
                        fmt_anchor_names: vec![],
                        fmt_is_anchor,
                        fmt_tooltip: None,
                        fmt_underline_style: None,
                        fmt_vertical_alignment: None,
                    }
                })
                .collect();

            let plain_text: String = pb.spans.iter().map(|s| s.text.as_str()).collect();

            let list = pb.list_style.map(|style| FragmentList {
                style,
                indent: 0,
                prefix: String::new(),
                suffix: String::new(),
            });

            FragmentBlock {
                plain_text,
                elements,
                heading_level: pb.heading_level,
                list,
                alignment: None,
                indent: None,
                text_indent: None,
                marker: None,
                top_margin: None,
                bottom_margin: None,
                left_margin: None,
                right_margin: None,
                tab_positions: vec![],
                line_height: pb.line_height,
                non_breakable_lines: pb.non_breakable_lines,
                direction: pb.direction,
                background_color: pb.background_color,
            }
        })
        .collect();

    let data = serde_json::to_string(&FragmentData { blocks })
        .expect("fragment serialization should not fail");

    let plain_text = parsed_plain_text_from_data(&data);

    DocumentFragment { data, plain_text }
}

/// Extract plain text from serialized fragment data.
fn parsed_plain_text_from_data(data: &str) -> String {
    let fragment_data: FragmentData = match serde_json::from_str(data) {
        Ok(d) => d,
        Err(_) => return String::new(),
    };

    fragment_data
        .blocks
        .iter()
        .map(|b| b.plain_text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
