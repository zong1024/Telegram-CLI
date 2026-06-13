//! Telegram CLI — thin frontend that talks to tg-daemon over a Unix socket.

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::warn;

use tg_common::config::TgConfig;
use tg_common::protocol::{methods, Request, ServerMessage, AuthState};

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
        /// Chat ID
        chat: String,
        /// Message ID (must contain media)
        msg_id: i64,
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

    match cli.command {
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

    // All other commands: connect to daemon, send request, print result
    let config = TgConfig::load()?;
    let socket = &config.socket_path;

    if !socket.exists() {
        anyhow::bail!(
            "Daemon not running (socket not found at {}).\n\
             Start it with: tg-daemon",
            socket.display()
        );
    }

    let stream = UnixStream::connect(socket).await?;
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);

    let (method, params) = match &cli.command {
        Commands::Me => (methods::GET_ME, json!({})),
        Commands::Ls { limit } => (methods::LIST_DIALOGS, json!({ "limit": limit })),
        Commands::Messages { chat, limit } => (
            methods::GET_MESSAGES,
            json!({ "chat_id": chat_id_from_str(chat), "limit": limit }),
        ),
        Commands::Send { chat, text } => {
            let msg = text.join(" ");
            (
                methods::SEND_MESSAGE,
                json!({ "chat_id": chat_id_from_str(chat), "text": msg }),
            )
        }
        Commands::Search { chat, query, limit } => (
            methods::SEARCH,
            json!({ "chat_id": chat_id_from_str(chat), "query": query, "limit": limit }),
        ),
        Commands::Forward { from, to, msg_id } => (
            methods::FORWARD_MESSAGE,
            json!({
                "from_chat_id": chat_id_from_str(from),
                "to_chat_id": chat_id_from_str(to),
                "message_id": msg_id
            }),
        ),
        Commands::Delete { chat, msg_id } => (
            methods::DELETE_MESSAGE,
            json!({ "chat_id": chat_id_from_str(chat), "message_id": msg_id }),
        ),
        Commands::Download { chat, msg_id } => {
            // Need file_id — first get the message, then extract it.
            // For now, pass msg_id and let daemon handle it.
            (
                methods::DOWNLOAD_FILE,
                json!({ "chat_id": chat_id_from_str(chat), "message_id": msg_id }),
            )
        }
        Commands::Read { chat } => (
            methods::MARK_READ,
            json!({ "chat_id": chat_id_from_str(chat) }),
        ),
        Commands::Status => (methods::GET_STATUS, json!({})),
        Commands::Logout => (methods::LOGOUT, json!({})),
        Commands::Stop => (methods::SHUTDOWN, json!({})),
        Commands::Init | Commands::Login => unreachable!(),
    };

    let req = Request {
        id: 1,
        method: method.to_string(),
        params,
    };

    let line = serde_json::to_string(&req)?;
    writer.write_all(line.as_bytes()).await?;
    writer.write_all(b"\n").await?;

    // Read one response line
    let mut resp_line = String::new();
    reader.read_line(&mut resp_line).await?;
    let resp: serde_json::Value = serde_json::from_str(&resp_line)?;

    // Also check for auth_state events that arrive before the response
    if let Some(tp) = resp.get("type").and_then(|v| v.as_str()) {
        match tp {
            "auth_state" => {
                let state = resp["state"].as_str().unwrap_or("unknown");
                println!("🔐  Auth state: {state}");
                return Ok(());
            }
            "response" => {
                if let Some(err) = resp.get("error") {
                    eprintln!("❌  {}", err["message"]);
                    return Ok(());
                }
                if let Some(result) = resp.get("result") {
                    output::print_result(&cli.command, result);
                }
            }
            _ => {
                println!("{}", serde_json::to_string_pretty(&resp)?);
            }
        }
    } else {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    }

    Ok(())
}

/// Parse a chat ID string. Numeric strings are parsed as i64,
/// usernames are used as-is (handled by TDLib with @ prefix).
fn chat_id_from_str(s: &str) -> serde_json::Value {
    if let Ok(id) = s.parse::<i64>() {
        json!(id)
    } else {
        // Username — the daemon should resolve it via TDLib
        json!(s)
    }
}
