use std::collections::HashMap;

use contracts::persistence::DocumentRepositoryTrait;
use contracts::persistence::RepositoryProviderTrait;

pub enum Repository {
    Document(Box<dyn DocumentRepositoryTrait>),
}

pub struct RepositoryProvider {
    repositories: HashMap<String, Repository>,
}

impl RepositoryProvider {
    pub fn new() -> Self {
        RepositoryProvider {
            repositories: HashMap::new(),
        }
    }
    pub(crate) fn register_repository(&mut self, name: String, repository: Repository) {
        self.repositories.insert(name, repository);
    }
}

impl RepositoryProviderTrait for RepositoryProvider {
    fn get_document_repository(&self) -> &dyn DocumentRepositoryTrait {
        match self.repositories.get("Document") {
            Some(Repository::Document(repository)) => repository.as_ref(),
            _ => panic!("Repository not found"),
        }
    }

    fn get_document_repository_mut(&mut self) -> &mut dyn DocumentRepositoryTrait {
        match self.repositories.get_mut("Document") {
            Some(Repository::Document(repository)) => repository.as_mut(),
            _ => panic!("Repository not found"),
        }
    }
}
