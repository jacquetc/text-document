use domain::document::Document;

pub trait RepositoryProviderTrait {
    fn get_document_repository(&self) -> &dyn DocumentRepositoryTrait;
    fn get_document_repository_mut(&mut self) -> &mut dyn DocumentRepositoryTrait;
}

pub trait DocumentRepositoryTrait {
    fn get(&self) -> &Document;
    fn get_mut(&mut self) -> &mut Document;
    fn update(&mut self, document: Document);
}
