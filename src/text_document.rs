use interactor::conversion_interactor::ConversionInteractor;
use persistence::persistence_registration::register_repositories;
use persistence::repository_provider::RepositoryProvider;

pub struct TextDocument {
    repository_provider: RepositoryProvider,
}

impl TextDocument {
    pub fn new() -> TextDocument {
        TextDocument {
            repository_provider: register_repositories(),
        }
    }

    pub fn get_plain_text(&self) -> String {
        ConversionInteractor::get_plain_text(&self.repository_provider)
    }

    pub fn set_plain_text<T: AsRef<str>>(&mut self, text: T) {
        ConversionInteractor::set_plain_text(&mut self.repository_provider, text.as_ref());
    }

    pub fn get_markdown(&self) -> String {
        ConversionInteractor::get_markdown(&self.repository_provider)
    }

    pub fn set_markdown<T: AsRef<str>>(&mut self, markdown: T) {
        ConversionInteractor::set_markdown(&mut self.repository_provider, markdown.as_ref());
    }
}
