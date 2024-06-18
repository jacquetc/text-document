use interactor::cursor_interactor::create_cursor;
use persistence::repository_provider::RepositoryProvider;

use crate::TextDocument;

pub struct TextCursor<'a> {
    id: usize,
    repository_provider: &'a RepositoryProvider,
}

impl TextCursor<'_> {
    pub fn new(text_document: &TextDocument) -> TextCursor {
        let provider = text_document.get_repository_provider();

        let id = create_cursor(provider);

        TextCursor {
            id,
            repository_provider: provider,
        }
    }
}
