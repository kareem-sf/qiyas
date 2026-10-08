use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("This drawing can't be opened: {0}")]
    Unreadable(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("No sheet named {0}")]
    NoSuchSheet(String),
    #[error("No entity with handle {0:X}")]
    NoSuchEntity(u64),
    #[error("{0}")]
    InvalidCommand(String),
    /// The project file is not valid JSON for a project; the parser's own words.
    #[error("{0}")]
    Project(String),
}
