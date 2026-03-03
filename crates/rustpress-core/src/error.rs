use thiserror::Error;

#[derive(Error, Debug)]
pub enum RustPressError {
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Hook error: {0}")]
    Hook(String),
}
