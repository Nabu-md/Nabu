use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    Io(String),
    Utf8(String),
    InvalidInput(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Io(msg) => write!(f, "I/O error: {}", msg),
            ParseError::Utf8(msg) => write!(f, "UTF-8 error: {}", msg),
            ParseError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
        }
    }
}

impl std::error::Error for ParseError {}