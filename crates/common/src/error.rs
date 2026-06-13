use thiserror::Error;

#[derive(Debug, Error)]
pub enum TgError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("TDLib error code {code}: {message}")]
    Tdlib { code: i32, message: String },

    #[error("Not authenticated. Run `tg login` first.")]
    NotAuthenticated,

    #[error("Connection lost")]
    ConnectionLost,

    #[error("{0}")]
    Other(String),
}
