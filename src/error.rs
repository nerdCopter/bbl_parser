use std::fmt;

/// Custom error types for BBL parsing
#[derive(Debug)]
pub enum BBLError {
    /// I/O errors
    Io(std::io::Error),
    /// UTF-8 parsing errors
    Utf8(std::str::Utf8Error),
    /// Parse errors with context
    Parse(String),
    /// Invalid header format
    InvalidHeader(String),
    /// Invalid frame data
    InvalidFrame(String),
    /// Unsupported data version
    UnsupportedVersion(u8),
    /// End of file reached unexpectedly
    UnexpectedEof,
    /// Invalid encoding type
    InvalidEncoding(u8),
    /// Invalid predictor type
    InvalidPredictor(u8),
    /// Export format error
    Export(String),
}

impl fmt::Display for BBLError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BBLError::Io(err) => write!(f, "I/O error: {}", err),
            BBLError::Utf8(err) => write!(f, "UTF-8 error: {}", err),
            BBLError::Parse(msg) => write!(f, "Parse error: {}", msg),
            BBLError::InvalidHeader(msg) => write!(f, "Invalid header: {}", msg),
            BBLError::InvalidFrame(msg) => write!(f, "Invalid frame: {}", msg),
            BBLError::UnsupportedVersion(version) => write!(f, "Unsupported data version: {}", version),
            BBLError::UnexpectedEof => write!(f, "Unexpected end of file"),
            BBLError::InvalidEncoding(encoding) => write!(f, "Invalid encoding type: {}", encoding),
            BBLError::InvalidPredictor(predictor) => write!(f, "Invalid predictor type: {}", predictor),
            BBLError::Export(msg) => write!(f, "Export error: {}", msg),
        }
    }
}

impl std::error::Error for BBLError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BBLError::Io(err) => Some(err),
            BBLError::Utf8(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for BBLError {
    fn from(err: std::io::Error) -> Self {
        BBLError::Io(err)
    }
}

impl From<std::str::Utf8Error> for BBLError {
    fn from(err: std::str::Utf8Error) -> Self {
        BBLError::Utf8(err)
    }
}

impl From<anyhow::Error> for BBLError {
    fn from(err: anyhow::Error) -> Self {
        BBLError::Parse(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, BBLError>;
