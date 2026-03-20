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
    pub fn from_html(_html: &str) -> Self {
        // TODO: parse HTML into the internal fragment format
        todo!("DocumentFragment::from_html")
    }

    /// Create a fragment from Markdown.
    pub fn from_markdown(_markdown: &str) -> Self {
        // TODO: parse Markdown into the internal fragment format
        todo!("DocumentFragment::from_markdown")
    }

    /// Create a fragment from an entire document.
    pub fn from_document(doc: &crate::TextDocument) -> crate::Result<Self> {
        let inner = doc.inner.lock().unwrap();
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
        // TODO: convert internal format to HTML
        todo!("DocumentFragment::to_html")
    }

    /// Export the fragment as Markdown.
    pub fn to_markdown(&self) -> String {
        // TODO: convert internal format to Markdown
        todo!("DocumentFragment::to_markdown")
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
