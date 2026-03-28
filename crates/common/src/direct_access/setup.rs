// HashMap backend needs no table initialization.

pub fn initialize_all_tables() -> Result<(), crate::error::RepositoryError> {
    Ok(())
}
