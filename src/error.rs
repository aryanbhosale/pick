use thiserror::Error;

#[derive(Error, Debug)]
pub enum PickError {
    #[error("key not found: {0}")]
    KeyNotFound(String),

    #[error("index out of bounds: {0}")]
    IndexOutOfBounds(i64),

    #[error("expected object for key '{0}', got {1}")]
    NotAnObject(String, String),

    #[error("expected array for index, got {0}")]
    NotAnArray(String),

    #[error("invalid selector: {0}")]
    InvalidSelector(String),

    #[error("failed to parse input as {0}: {1}")]
    ParseError(String, String),

    #[error("no input provided")]
    NoInput,

    #[error("could not detect input format")]
    UnknownFormat,

    #[error("input too large (max {} bytes)", .0)]
    InputTooLarge(u64),

    #[error("too many results (max {0})")]
    TooManyResults(usize),

    #[error("{0}")]
    Io(#[from] std::io::Error),
}
