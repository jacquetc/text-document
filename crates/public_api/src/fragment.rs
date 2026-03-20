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
    /// Stores the text for later insertion. The backend's `insert_fragment`
    /// use case handles the conversion to blocks and inline elements.
    pub fn from_plain_text(text: &str) -> Self {
        Self {
            data: String::new(),
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
