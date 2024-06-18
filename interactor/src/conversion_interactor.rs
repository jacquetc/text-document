use application::conversion_feature::export_to_plain_text_uc::ExportToPlainTextUseCase;
use application::conversion_feature::import_from_plain_text_uc::ImportFromPlainTextUseCase;
use contracts::persistence::RepositoryProviderTrait;

pub struct ConversionInteractor {}

impl ConversionInteractor {
    pub fn get_plain_text(repository_provider: &dyn RepositoryProviderTrait) -> String {
        let document_repository = repository_provider.get_document_repository();
        ExportToPlainTextUseCase::new(document_repository).execute()
    }

    pub fn set_plain_text(repository_provider: &mut dyn RepositoryProviderTrait, text: &str) {
        let document_repository = repository_provider.get_document_repository_mut();
        let _ = ImportFromPlainTextUseCase::new(document_repository).execute(text);
    }

    pub fn get_markdown(repository_provider: &dyn RepositoryProviderTrait) -> String {
        let document_repository = repository_provider.get_document_repository();
        ExportToPlainTextUseCase::new(document_repository).execute()
    }

    pub fn set_markdown(repository_provider: &mut dyn RepositoryProviderTrait, markdown: &str) {
        let document_repository = repository_provider.get_document_repository_mut();
        let _ = ImportFromPlainTextUseCase::new(document_repository).execute(markdown);
    }
}
