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
}

pub type Result<T> = std::result::Result<T, MMBakError>;
