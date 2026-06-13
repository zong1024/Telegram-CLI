//! `tg` — Telegram CLI command-line tool.
//!
//! Thin frontend that sends JSON-RPC requests to `tgcd` over a
//! length-delimited Unix socket connection.

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::json;

use tg_common::config::TgConfig;
use tg_common::ipc::IpcClient;
use tg_common::protocol::methods;

mod init;
mod login;
mod output;

#[derive(Parser)]
#[command(
    name = "tg",
    about = "Telegram CLI — manage chats from your terminal",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to config file
    #[arg(short, long, global = true)]
    config: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a default config file
    Init,

    /// Interactive login (phone number + code + 2FA)
    Login,

    /// Show current account info
    Me,

    /// List recent dialogs (chats/channels/users)
    Ls {
        /// Number of dialogs to show
        #[arg(short, long, default_value = "20")]
        limit: i64,
    },

    /// Show recent messages in a chat
    Messages {
        /// Chat ID or @username
        chat: String,
        /// Number of messages
        #[arg(short, long, default_value = "20")]
        limit: i64,
    },

    /// Send a message to a chat
    Send {
        /// Chat ID or @username
        chat: String,
        /// Message text (all remaining args joined)
        #[arg(required = true, trailing_var_arg = true)]
        text: Vec<String>,
    },

    /// Search messages in a chat
    Search {
        /// Chat ID or @username
        chat: String,
        /// Search query
        query: String,
        /// Max results
        #[arg(short, long, default_value = "20")]
        limit: i64,
    },

    /// Forward a message
    Forward {
        /// Source chat ID
        from: String,
        /// Destination chat ID
        to: String,
        /// Message ID to forward
        msg_id: i64,
    },

    /// Delete a message
    Delete {
        /// Chat ID
        chat: String,
        /// Message ID
        msg_id: i64,
    },

    /// Download file from a message
    Download {
        /// File ID
        file_id: i64,
    },

    /// Mark chat as read
    Read {
        /// Chat ID or @username
        chat: String,
    },

    /// Show daemon status
    Status,

    /// Log out and destroy session
    Logout,

    /// Stop the daemon
    Stop,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("warn")
        .init();

    let cli = Cli::parse();

    // `init` and `login` don't need daemon connection
    match &cli.command {
        Commands::Init => {
            init::run()?;
            return Ok(());
        }
        Commands::Login => {
            login::run().await?;
            return Ok(());
        }
        _ => {}
    }

    // Connect to daemon via IpcClient
    let config = TgConfig::load()?;
    let socket = &config.socket_path;

    if !socket.exists() {
        anyhow::bail!(
            "Daemon not running (socket not found at {}).\n\
             Start it with: tgcd",
            socket.display()
        );
    }

    let mut client = IpcClient::connect(socket).await?;

    // Build request params based on command
    let (method, params) = match &cli.command {
        Commands::Me => (methods::GET_ME, json!({})),
        Commands::Ls { limit } => (methods::LIST_DIALOGS, json!({ "limit": limit })),
        Commands::Messages { chat, limit } => (
            methods::GET_MESSAGES,
            json!({ "chat_id": parse_chat_id(chat), "limit": limit }),
        ),
        Commands::Send { chat, text } => {
            let msg = text.join(" ");
            (
                methods::SEND_MESSAGE,
                json!({ "chat_id": parse_chat_id(chat), "text": msg }),
            )
        }
        Commands::Search { chat, query, limit } => (
            methods::SEARCH,
            json!({ "chat_id": parse_chat_id(chat), "query": query, "limit": limit }),
        ),
        Commands::Forward { from, to, msg_id } => (
            methods::FORWARD_MESSAGE,
            json!({
                "from_chat_id": parse_chat_id(from),
                "to_chat_id": parse_chat_id(to),
                "message_id": msg_id
            }),
        ),
        Commands::Delete { chat, msg_id } => (
            methods::DELETE_MESSAGE,
            json!({ "chat_id": parse_chat_id(chat), "message_id": msg_id }),
        ),
        Commands::Download { file_id } => (
            methods::DOWNLOAD_FILE,
            json!({ "file_id": file_id }),
        ),
        Commands::Read { chat } => (
            methods::MARK_READ,
            json!({ "chat_id": parse_chat_id(chat) }),
        ),
        Commands::Status => (methods::GET_STATUS, json!({})),
        Commands::Logout => (methods::LOGOUT, json!({})),
        Commands::Stop => (methods::SHUTDOWN, json!({})),
        Commands::Init | Commands::Login => unreachable!(),
    };

    let result = client.call(method, params).await;
    match result {
        Ok(val) => output::print_result(&cli.command, &val),
        Err(e) => eprintln!("❌  {e}"),
    }

    Ok(())
}

/// Parse a chat ID. Numeric strings → i64, usernames stay as strings.
fn parse_chat_id(s: &str) -> serde_json::Value {
    if let Ok(id) = s.parse::<i64>() {
        json!(id)
    } else {
        json!(s)
    }
}
