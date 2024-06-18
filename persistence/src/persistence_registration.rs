use crate::document_repository::DocumentRepository;
use crate::repository_provider::Repository;
use crate::repository_provider::RepositoryProvider;

pub fn register_repositories() -> RepositoryProvider {
    let mut provider = RepositoryProvider::new();
    provider.register_repository(
        "Document".to_string(),
        Repository::Document(Box::new(DocumentRepository::new())),
    );
    provider
}
