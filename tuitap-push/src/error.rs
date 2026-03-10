use thiserror::Error;

/// Errors produced by push-channel adapters.
#[derive(Debug, Error)]
pub enum PushError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Channel error: {0}")]
    Channel(String),
}
