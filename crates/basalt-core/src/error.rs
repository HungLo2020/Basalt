use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("{0}")]
    InvalidInput(String),

    #[error("A game with name '{0}' already exists")]
    GameNameExists(String),

    #[error("A game with target '{0}' already exists")]
    GameTargetExists(String),

    #[error("Game name '{0}' is blacklisted")]
    Blacklisted(String),

    #[error("No game found with name '{0}'")]
    GameNotFound(String),

    #[error("Playlist '{0}' does not exist")]
    PlaylistNotFound(String),

    #[error("Unsupported emulator system: {0}")]
    UnsupportedSystem(String),

    #[error("Cancelled")]
    Cancelled,

    #[error("{action} {}: {source}", path.display())]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse {}: {source}", path.display())]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    /// Catch-all for failures that callers only ever display, never branch on.
    #[error("{0}")]
    Message(String),
}

impl CoreError {
    pub fn new(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    pub(crate) fn io(
        action: &'static str,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Self::Io {
            action,
            path: path.into(),
            source,
        }
    }

    /// True when the game being added is already in the registry (by name or by target).
    pub fn is_duplicate_game(&self) -> bool {
        matches!(self, Self::GameNameExists(_) | Self::GameTargetExists(_))
    }

    pub fn is_blacklisted(&self) -> bool {
        matches!(self, Self::Blacklisted(_))
    }
}

impl From<String> for CoreError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<&str> for CoreError {
    fn from(message: &str) -> Self {
        Self::Message(message.to_string())
    }
}

impl From<CoreError> for String {
    fn from(error: CoreError) -> Self {
        error.to_string()
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
