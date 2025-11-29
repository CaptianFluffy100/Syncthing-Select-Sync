use std::fmt;
use rusqlite;
use reqwest;
use std::io;

#[derive(Debug)]
pub enum AppError {
    Database(rusqlite::Error),
    Http(reqwest::Error),
    Io(io::Error),
    NotFound(String),
    InvalidInput(String),
    Configuration(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::Database(e) => write!(f, "Database error: {}", e),
            AppError::Http(e) => write!(f, "HTTP error: {}", e),
            AppError::Io(e) => write!(f, "IO error: {}", e),
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            AppError::Configuration(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::Database(err)
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Http(err)
    }
}

impl From<io::Error> for AppError {
    fn from(err: io::Error) -> Self {
        AppError::Io(err)
    }
}

pub type AppResult<T> = Result<T, AppError>;

