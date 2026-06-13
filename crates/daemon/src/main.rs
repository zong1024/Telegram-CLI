mod auth;
mod dispatcher;
mod handler;
mod server;
mod tdlib_client;

use anyhow::Result;
use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;
use tg_common::config::TgConfig;

#[derive(Parser)]
#[command(name = "tg-daemon", about = "Telegram CLI daemon (TDLib backend)")]
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

    let mut config = if let Some(path) = args.config {
        let content = std::fs::read_to_string(&path)?;
        toml::from_str(&content)?
    } else {
        TgConfig::load().unwrap_or_else(|_| {
            eprintln!("No config found. Run: tg init");
            std::process::exit(1);
        })
    };

    // Ensure database directory exists
    std::fs::create_dir_all(&config.database_dir)?;

    info!("Starting tg-daemon…");
    info!("Socket : {}", config.socket_path.display());
    info!("DB     : {}", config.database_dir.display());

    // Build shared state
    let (event_tx, _event_rx) = tokio::sync::broadcast::channel::<serde_json::Value>(256);
    let td = tdlib_client::TdClient::new(&config, event_tx.clone())?;

    let state = handler::AppState {
        config: config.clone(),
        td,
        event_tx,
    };

    // Check / drive auth before accepting clients
    auth::ensure_authorized(&state).await?;

    // Remove stale socket
    if config.socket_path.exists() {
        std::fs::remove_file(&config.socket_path)?;
    }

    // Accept clients
    server::run(&config.socket_path, state).await
}
