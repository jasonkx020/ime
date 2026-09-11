use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EngineError {
    #[error("session invalid")]
    SessionInvalid,
    #[error("operation cancelled")]
    Cancelled,
    #[error("timeout")]
    Timeout,
    #[error("busy")]
    Busy,
    #[error("unsupported")]
    Unsupported,
    #[error("pack invalid")]
    PackInvalid,
    #[error("internal error")]
    Internal,
}

pub type HotResult<T> = Result<T, EngineError>;
