use std::fmt;

/// Persistence layer errors.
#[derive(Debug)]
pub enum PersistenceError {
    /// Wraps a rusqlite error.
    SqliteError { message: String },
    /// Migration failed at a specific version.
    MigrationFailed { version: i32, message: String },
    /// JSON serialization or deserialization error.
    SerializationError { message: String },
}

impl fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PersistenceError::SqliteError { message } => {
                write!(f, "SQLite error: {}", message)
            }
            PersistenceError::MigrationFailed { version, message } => {
                write!(f, "Migration v{} failed: {}", version, message)
            }
            PersistenceError::SerializationError { message } => {
                write!(f, "Serialization error: {}", message)
            }
        }
    }
}

impl std::error::Error for PersistenceError {}

impl From<rusqlite::Error> for PersistenceError {
    fn from(e: rusqlite::Error) -> Self {
        PersistenceError::SqliteError {
            message: e.to_string(),
        }
    }
}

impl From<serde_json::Error> for PersistenceError {
    fn from(e: serde_json::Error) -> Self {
        PersistenceError::SerializationError {
            message: e.to_string(),
        }
    }
}
