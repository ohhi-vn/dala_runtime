/// Error type for BEAM loading operations.
#[derive(Debug, thiserror::Error)]
pub enum BeamError {
    #[error("I/O error: {0}")]
    IoError(String),
    #[error("format error: {0}")]
    FormatError(String),
    #[error("unexpected end of file")]
    UnexpectedEof,
    #[error("unsupported feature: {0}")]
    Unsupported(String),
}

/// Result type for BEAM operations.
pub type Result<T> = std::result::Result<T, BeamError>;
