use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::TgError;

/// Default socket path
pub fn default_socket_path() -> PathBuf {
    let run = dirs_runtime().unwrap_or_else(|| PathBuf::from("/tmp"));
    run.join("tg-cli.sock")
}

/// Default TDLib database directory
pub fn default_database_dir() -> PathBuf {
    dirs_data().unwrap_or_else(|| PathBuf::from(".")).join("tg-cli")
}

fn dirs_runtime() -> Option<PathBuf> {
    std::env::var("XDG_RUNTIME_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from("/tmp")))
}

fn dirs_data() -> Option<PathBuf> {
    std::env::var("XDG_DATA_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            dirs::home_dir().map(|h| h.join(".local").join("share"))
        })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TgConfig {
    /// Telegram API ID (from https://my.telegram.org)
    pub api_id: i32,
    /// Telegram API hash
    pub api_hash: String,
    /// Phone number for authentication (optional, prompted at login)
    #[serde(default)]
    pub phone: String,
    /// Path to Unix socket
    #[serde(default = "default_socket_path")]
    pub socket_path: PathBuf,
    /// TDLib database directory
    #[serde(default = "default_database_dir")]
    pub database_dir: PathBuf,
    /// Log verbosity (0 = errors only, 1 = warnings, 2 = info, 3+ = debug)
    #[serde(default)]
    pub verbosity: i32,
    /// Use test Telegram server
    #[serde(default)]
    pub test: bool,
}

impl TgConfig {
    pub fn load() -> Result<Self, TgError> {
        let config_path = Self::config_path();
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            Ok(toml::from_str(&content)?)
        } else {
            Err(TgError::Config(format!(
                "Config not found at {}. Run `tg init` to create one.",
                config_path.display()
            )))
        }
    }

    pub fn config_path() -> PathBuf {
        std::env::var("TG_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let cfg = dirs::config_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("tg-cli");
                cfg.join("config.toml")
            })
    }

    pub fn save(&self) -> Result<(), TgError> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}
