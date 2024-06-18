pub fn register_interactors(
    repository_provider: Rc<RefCell<dyn RepositoryProviderTrait>>,
) -> InteractorProvider {
    let mut provider = InteractorProvider::new();
    provider.register_interactor(
        "Document".to_string(),
        Interactor::Document(DocumentInteractor::new()),
    );
    provider
}
