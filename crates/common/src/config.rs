use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::TgError;

/// XDG-compliant project directories.
fn project_dirs() -> directories::ProjectDirs {
    directories::ProjectDirs::from("com", "telegram-cli", "tg")
        .expect("cannot determine home directory")
}

/// Default socket path: `$XDG_RUNTIME_DIR/tg/tgcd.sock`
pub fn default_socket_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"));
    runtime.join("tg").join("tgcd.sock")
}

/// Default database (SQLite) path: `~/.local/share/tg/tg.db`
pub fn default_database_path() -> PathBuf {
    project_dirs().data_dir().join("tg.db")
}

/// Default config path: `~/.config/tg/config.toml`
pub fn default_config_path() -> PathBuf {
    project_dirs().config_dir().join("config.toml")
}

/// Default TDLib database directory: `~/.local/share/tg/tdlib/`
pub fn default_tdlib_dir() -> PathBuf {
    project_dirs().data_dir().join("tdlib")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TgConfig {
    /// Telegram API ID (from https://my.telegram.org).
    /// Stored in config file; prefer keyring at runtime.
    pub api_id: i32,
    /// Telegram API hash.
    /// Stored in config file; prefer keyring at runtime.
    pub api_hash: String,
    /// Phone number for authentication (optional, prompted at login).
    #[serde(default)]
    pub phone: String,
    /// Path to Unix socket.
    #[serde(default = "default_socket_path")]
    pub socket_path: PathBuf,
    /// Path to SQLite cache database.
    #[serde(default = "default_database_path")]
    pub database_path: PathBuf,
    /// TDLib internal database directory.
    #[serde(default = "default_tdlib_dir")]
    pub tdlib_dir: PathBuf,
    /// Log verbosity (0–3+).
    #[serde(default)]
    pub verbosity: i32,
    /// Use Telegram test server.
    #[serde(default)]
    pub test: bool,
}

impl TgConfig {
    /// Load config from disk.
    pub fn load() -> Result<Self, TgError> {
        let path = Self::config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(toml::from_str(&content)?)
        } else {
            Err(TgError::Config(format!(
                "Config not found at {}. Run `tg init` to create one.",
                path.display()
            )))
        }
    }

    /// Config file path.
    pub fn config_path() -> PathBuf {
        std::env::var("TG_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_config_path())
    }

    /// Save config to disk.
    pub fn save(&self) -> Result<(), TgError> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Resolve `api_id` — keyring first, config file fallback.
    pub fn resolve_api_id() -> Result<i32, TgError> {
        // Try keyring
        if let Ok(entry) = keyring::Entry::new("tg-cli", "api_id") {
            if let Ok(val) = entry.get_password() {
                if let Ok(id) = val.parse::<i32>() {
                    return Ok(id);
                }
            }
        }
        // Fallback to config file
        let config = Self::load()?;
        Ok(config.api_id)
    }

    /// Resolve `api_hash` — keyring first, config file fallback.
    pub fn resolve_api_hash() -> Result<String, TgError> {
        if let Ok(entry) = keyring::Entry::new("tg-cli", "api_hash") {
            if let Ok(val) = entry.get_password() {
                return Ok(val);
            }
        }
        let config = Self::load()?;
        Ok(config.api_hash)
    }

    /// Store credentials in keyring.
    pub fn store_credentials(&self) -> Result<(), TgError> {
        let entry_id = keyring::Entry::new("tg-cli", "api_id")
            .map_err(|e| TgError::Other(format!("keyring: {e}")))?;
        entry_id
            .set_password(&self.api_id.to_string())
            .map_err(|e| TgError::Other(format!("keyring: {e}")))?;

        let entry_hash = keyring::Entry::new("tg-cli", "api_hash")
            .map_err(|e| TgError::Other(format!("keyring: {e}")))?;
        entry_hash
            .set_password(&self.api_hash)
            .map_err(|e| TgError::Other(format!("keyring: {e}")))?;

        Ok(())
    }
}
