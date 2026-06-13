use thiserror::Error;

#[derive(Debug, Error)]
pub enum TgError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML deserialize error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("TDLib error code {code}: {message}")]
    Tdlib { code: i32, message: String },

    #[error("Not authenticated")]
    NotAuthenticated,

    #[error("Connection lost")]
    ConnectionLost,

    #[error("IPC frame too large: {size} bytes (max {max})")]
    FrameTooLarge { size: usize, max: usize },

    #[error("{0}")]
    Other(String),
}
