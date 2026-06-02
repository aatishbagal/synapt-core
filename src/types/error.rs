use thiserror::Error;

/// Shared error type for cross-crate boundaries.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("serialisation error: {0}")]
    Serialise(#[from] serde_json::Error),
    #[error("invalid id: {0}")]
    InvalidId(String),
}
