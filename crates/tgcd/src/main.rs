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

use tg_core::config::TgConfig;

#[derive(Parser)]
#[command(name = "tgcd", about = "Telegram CLI daemon", version)]
struct Args {
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

    let config: TgConfig = if let Some(path) = args.config {
        let content = std::fs::read_to_string(&path)?;
        toml::from_str(&content)?
    } else {
        TgConfig::load().unwrap_or_else(|_| {
            eprintln!("No config found. Run: tg init");
            std::process::exit(1);
        })
    };

    let api_hash = config.load_api_hash();

    // Ensure directories
    std::fs::create_dir_all(&config.tdlib.database_directory)?;
    std::fs::create_dir_all(&config.tdlib.files_directory)?;
    if let Some(parent) = config.ipc.socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    info!("Starting tgcd…");
    info!("Socket : {}", config.ipc.socket_path.display());
    info!("DB     : {}", config.database_path().display());

    // Initialize TDLib with global receive loop
    let (updates_tx, _updates_rx) = broadcast::channel::<serde_json::Value>(256);
    tg_tdjson::set_log_verbosity(config.tdlib.verbosity);
    tg_tdjson::init(updates_tx.clone());

    // Create TDLib client
    let td = tg_tdjson::TdClient::new();

    // Configure TDLib parameters
    td.send(serde_json::json!({
        "@type": "setTdlibParameters",
        "database_directory": config.tdlib.database_directory.to_string_lossy(),
        "files_directory": config.tdlib.files_directory.to_string_lossy(),
        "use_message_database": config.tdlib.use_message_database,
        "use_secret_chats": config.tdlib.use_secret_chats,
        "use_test_dc": config.tdlib.test,
        "api_id": config.telegram.api_id,
        "api_hash": api_hash,
        "system_language_code": config.tdlib.system_language_code,
        "device_model": config.tdlib.device_model,
        "system_version": std::env::consts::OS,
        "application_version": config.tdlib.application_version,
        "enable_storage_optimizer": true
    }))
    .await?;

    // Set database encryption key
    td.send(serde_json::json!({
        "@type": "setDatabaseEncryptionKey",
        "new_encryption_key": ""
    }))
    .await?;

    // Configure proxy if enabled
    if config.proxy.enabled {
        let proxy_type = match config.proxy.kind.as_str() {
            "socks5" => "proxyTypeSocks5",
            "http" => "proxyTypeHttp",
            "mtproto" => "proxyTypeMtproto",
            _ => "proxyTypeSocks5",
        };
        td.send(serde_json::json!({
            "@type": "addProxy",
            "server": config.proxy.host,
            "port": config.proxy.port as i64,
            "enable": true,
            "type": {
                "@type": proxy_type,
                "username": config.proxy.username,
                "password": config.proxy.password,
            }
        }))
        .await?;
        info!("Proxy configured: {}://{}:{}", config.proxy.kind, config.proxy.host, config.proxy.port);
    }

    // Open SQLite cache
    let cache = cache::Cache::new(&config.database_path()).await?;

    // Spawn cache updater
    let td_clone = td.clone();
    let cache_clone = cache.clone();
    tokio::spawn(async move {
        dispatcher::run_cache_updater(td_clone, cache_clone).await;
    });

    // Trigger auth state machine (non-blocking)
    // TDLib will send updateAuthorizationState events that flow to IPC clients
    td.send_no_wait(serde_json::json!({"@type": "getAuthorizationState"}));

    let state = handler::AppState {
        config: config.clone(),
        td,
        cache,
        updates_tx,
    };

    // Remove stale socket
    if config.ipc.socket_path.exists() {
        std::fs::remove_file(&config.ipc.socket_path)?;
    }

    // Write PID file (for `tg stop`)
    let pid_path = config.ipc.socket_path.with_extension("pid");
    std::fs::write(&pid_path, std::process::id().to_string())?;

    info!("IPC server starting — use `tg login` to authenticate");

    // Start IPC server (auth happens via IPC commands, not blocking here)
    let result = ipc::run(&config.ipc.socket_path, state).await;

    // Cleanup PID file on exit
    let _ = std::fs::remove_file(&pid_path);
    result
}
