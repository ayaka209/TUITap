use thiserror::Error;

/// All errors produced by tuitap-core.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("PTY error: {0}")]
    Pty(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Process has already exited")]
    ProcessExited,

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
}
