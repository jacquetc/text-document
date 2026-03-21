//! DocumentFragment — format-agnostic rich text interchange type.

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
        use frontend::common::entities::InlineContent;
        use frontend::common::parser_tools::fragment_schema::{
            FragmentBlock, FragmentData, FragmentElement,
        };

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
        use frontend::common::entities::InlineContent;
        use frontend::common::parser_tools::fragment_schema::FragmentData;

        if self.data.is_empty() {
            return String::from("<html><head><meta charset=\"utf-8\"></head><body></body></html>");
        }

        let fragment_data: FragmentData = match serde_json::from_str(&self.data) {
            Ok(d) => d,
            Err(_) => {
                return String::from(
                    "<html><head><meta charset=\"utf-8\"></head><body></body></html>",
                )
            }
        };

        let mut body = String::new();
        let blocks = &fragment_data.blocks;
        let mut i = 0;

        while i < blocks.len() {
            let block = &blocks[i];

            if let Some(ref list) = block.list {
                // Start a list group
                let is_ordered = is_ordered_style(&list.style);
                let list_tag = if is_ordered { "ol" } else { "ul" };
                body.push('<');
                body.push_str(list_tag);
                body.push('>');

                // Collect consecutive blocks with the same list type
                while i < blocks.len() {
                    let b = &blocks[i];
                    match &b.list {
                        Some(l) if is_ordered_style(&l.style) == is_ordered => {
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
                body.push_str("<p>");
                push_inline_html(&mut body, &block.elements);
                body.push_str("</p>");
                i += 1;
            }
        }

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

        fn push_inline_html(
            out: &mut String,
            elements: &[frontend::common::parser_tools::fragment_schema::FragmentElement],
        ) {
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

                // Determine wrapping tags (innermost first)
                let is_monospace = elem
                    .fmt_font_family
                    .as_deref()
                    .map_or(false, |f| f == "monospace");
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
                if is_anchor {
                    if let Some(ref href) = elem.fmt_anchor_href {
                        result = format!("<a href=\"{}\">{}</a>", escape_html(href), result);
                    }
                }

                out.push_str(&result);
            }
        }

        fn is_ordered_style(
            style: &frontend::common::entities::ListStyle,
        ) -> bool {
            use frontend::common::entities::ListStyle;
            matches!(
                style,
                ListStyle::Decimal
                    | ListStyle::LowerAlpha
                    | ListStyle::UpperAlpha
                    | ListStyle::LowerRoman
                    | ListStyle::UpperRoman
            )
        }

        format!(
            "<html><head><meta charset=\"utf-8\"></head><body>{}</body></html>",
            body
        )
    }

    /// Export the fragment as Markdown.
    pub fn to_markdown(&self) -> String {
        use frontend::common::entities::InlineContent;
        use frontend::common::parser_tools::fragment_schema::FragmentData;

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
                    parts.push(format!("{}. {}", list_counter, inline_text));
                } else {
                    parts.push(format!("- {}", inline_text));
                }
                prev_was_list = true;
            } else {
                parts.push(inline_text);
                prev_was_list = false;
                list_counter = 0;
            }

            // Track whether we need \n or \n\n separator
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

        fn escape_markdown(s: &str) -> String {
            let mut out = String::with_capacity(s.len());
            for c in s.chars() {
                if matches!(c, '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '(' | ')' | '#' | '+' | '-' | '.' | '!' | '|' | '~' | '<' | '>') {
                    out.push('\\');
                }
                out.push(c);
            }
            out
        }

        fn render_inline_markdown(
            elements: &[frontend::common::parser_tools::fragment_schema::FragmentElement],
        ) -> String {
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
                    .map_or(false, |f| f == "monospace");
                let is_bold = elem.fmt_font_bold.unwrap_or(false);
                let is_italic = elem.fmt_font_italic.unwrap_or(false);
                let is_strikeout = elem.fmt_font_strikeout.unwrap_or(false);
                let is_anchor = elem.fmt_is_anchor.unwrap_or(false);

                if is_monospace {
                    // Code spans: no escaping inside backticks
                    out.push('`');
                    out.push_str(&raw_text);
                    out.push('`');
                } else if is_anchor {
                    let href = elem.fmt_anchor_href.as_deref().unwrap_or("");
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
                    out.push_str(&format!("[{}]({})", text, href));
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
                    out.push_str(&text);
                }
            }
            out
        }

        fn is_ordered_list_style(
            style: &frontend::common::entities::ListStyle,
        ) -> bool {
            use frontend::common::entities::ListStyle;
            matches!(
                style,
                ListStyle::Decimal
                    | ListStyle::LowerAlpha
                    | ListStyle::UpperAlpha
                    | ListStyle::LowerRoman
                    | ListStyle::UpperRoman
            )
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

/// Convert parsed blocks (from HTML or Markdown parser) into a `DocumentFragment`.
fn parsed_blocks_to_fragment(
    parsed: Vec<frontend::common::parser_tools::content_parser::ParsedBlock>,
) -> DocumentFragment {
    use frontend::common::entities::InlineContent;
    use frontend::common::parser_tools::fragment_schema::{
        FragmentBlock, FragmentData, FragmentElement, FragmentList,
    };

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
            }
        })
        .collect();

    let data = serde_json::to_string(&FragmentData { blocks })
        .expect("fragment serialization should not fail");

    let plain_text: String = parsed_plain_text_from_data(&data);

    DocumentFragment {
        data,
        plain_text,
    }
}

/// Extract plain text from serialized fragment data.
fn parsed_plain_text_from_data(data: &str) -> String {
    use frontend::common::parser_tools::fragment_schema::FragmentData;

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

impl Default for DocumentFragment {
    fn default() -> Self {
        Self::new()
    }
}
