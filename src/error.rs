//! The crate-wide error type.

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("PNG: {0}")]
    Png(String),
    /// An invalid annotation spec; `path` locates the offending value, e.g. `annotations[2].size`.
    #[error("{path}: {message}")]
    Spec { path: String, message: String },
    #[error("unsupported: {0}")]
    Unsupported(String),
}

impl From<png::DecodingError> for Error {
    fn from(e: png::DecodingError) -> Self {
        Self::Png(e.to_string())
    }
}

impl From<png::EncodingError> for Error {
    fn from(e: png::EncodingError) -> Self {
        Self::Png(e.to_string())
    }
}
