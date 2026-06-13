//! `tgcd` — Telegram CLI Daemon.
//!
//! Background daemon that owns the TDLib connection, caches messages in
//! SQLite, and accepts client connections over a Unix socket.

mod auth;
mod cache;
mod dispatcher;
mod handler;
mod ipc;
mod tdlib;

use anyhow::Result;
use clap::Parser;
use tokio::sync::broadcast;
use tracing::info;
use tracing_subscriber::EnvFilter;

use tg_common::config::TgConfig;

#[derive(Parser)]
#[command(
    name = "tgcd",
    about = "Telegram CLI daemon — background service for tg and tg-tui"
)]
struct Args {
    /// Path to config file
    #[arg(short, long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    let config = if let Some(path) = args.config {
        let content = std::fs::read_to_string(&path)?;
        toml::from_str(&content)?
    } else {
        TgConfig::load().unwrap_or_else(|_| {
            eprintln!("No config found. Run: tg init");
            std::process::exit(1);
        })
    };

    // Resolve credentials: keyring first, config file fallback
    let api_id = TgConfig::resolve_api_id().unwrap_or(config.api_id);
    let api_hash = TgConfig::resolve_api_hash().unwrap_or(config.api_hash.clone());

    let mut config = config;
    config.api_id = api_id;
    config.api_hash = api_hash;

    // Ensure directories exist
    std::fs::create_dir_all(&config.tdlib_dir)?;
    if let Some(parent) = config.socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    info!("Starting tgcd…");
    info!("Socket : {}", config.socket_path.display());
    info!("DB     : {}", config.database_path.display());
    info!("TDLib  : {}", config.tdlib_dir.display());

    // Create components
    let (event_tx, _event_rx) = broadcast::channel::<serde_json::Value>(256);
    let td = tdlib::TdClient::new(&config, event_tx.clone())?;
    let cache = cache::Cache::new(&config.database_path).await?;

    // Spawn cache updater (listens to TDLib events, writes to SQLite)
    let cache_for_updater = cache::Cache::new(&config.database_path).await?;
    let td_for_updater = td.clone();
    tokio::spawn(async move {
        dispatcher::run_cache_updater(td_for_updater, cache_for_updater).await;
    });

    let state = handler::AppState {
        config: config.clone(),
        td,
        cache,
        event_tx,
    };

    // Check / drive auth before accepting clients
    auth::ensure_authorized(&state).await?;

    // Remove stale socket
    if config.socket_path.exists() {
        std::fs::remove_file(&config.socket_path)?;
    }

    // Accept IPC clients
    ipc::run(&config.socket_path, state).await
}
