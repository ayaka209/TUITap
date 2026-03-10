use thiserror::Error;

/// Errors produced by the gateway.
#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("Core error: {0}")]
    Core(#[from] tuitap_core::CoreError),

    #[error("Push error: {0}")]
    Push(#[from] tuitap_push::PushError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Channel send error: {0}")]
    Send(String),

    #[error("Approval not found: {0}")]
    ApprovalNotFound(String),

    #[error("Permission denied for identity '{0}'")]
    PermissionDenied(String),

    #[error("Unknown identity: '{0}'")]
    UnknownIdentity(String),
}
