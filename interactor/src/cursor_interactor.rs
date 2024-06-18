use application::cursor_feature::create_cursor_uc::CreateCursorUseCase;
use contracts::persistence::RepositoryProviderTrait;

pub fn create_cursor(repository_provider: &dyn RepositoryProviderTrait) -> usize {
    let cursor_repository = repository_provider.get_cursor_repository();
    CreateCursorUseCase::new(cursor_repository).execute()
}
