use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MMBakError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("SQLite error: {0}")]
    SqliteError(#[from] rusqlite::Error),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Entry not found: {0}")]
    EntryNotFound(String),

    #[error("Invalid data format: {0}")]
    InvalidDataFormat(String),

    #[error("Config parse error: {0}")]
    ConfigParseError(String),

    #[error("JSON error: {0}")]
    JsonError(String),

    #[error("Google API error: {0}")]
    GoogleApiError(String),
}

impl From<serde_json::Error> for MMBakError {
    fn from(err: serde_json::Error) -> Self {
        MMBakError::JsonError(err.to_string())
    }
}

impl From<toml::de::Error> for MMBakError {
    fn from(err: toml::de::Error) -> Self {
        MMBakError::ConfigParseError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, MMBakError>;
