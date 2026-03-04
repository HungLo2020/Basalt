use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone)]
pub struct CoreError {
    message: String,
}

impl CoreError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for CoreError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for CoreError {}

impl From<String> for CoreError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl From<&str> for CoreError {
    fn from(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl From<CoreError> for String {
    fn from(error: CoreError) -> Self {
        error.message
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
