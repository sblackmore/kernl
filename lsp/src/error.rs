use std::fmt;

#[derive(Debug)]
pub enum LspError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Protocol(String),
}

impl fmt::Display for LspError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LspError::Io(e) => write!(f, "IO error: {e}"),
            LspError::Json(e) => write!(f, "JSON error: {e}"),
            LspError::Protocol(msg) => write!(f, "protocol error: {msg}"),
        }
    }
}

impl std::error::Error for LspError {}

impl From<std::io::Error> for LspError {
    fn from(e: std::io::Error) -> Self {
        LspError::Io(e)
    }
}

impl From<serde_json::Error> for LspError {
    fn from(e: serde_json::Error) -> Self {
        LspError::Json(e)
    }
}
