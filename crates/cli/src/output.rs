//! Pretty-print daemon responses to stdout.

use serde_json::Value;
use crate::Commands;

pub fn print_result(cmd: &Commands, result: &Value) {
    match cmd {
        Commands::Status => print_status(result),
        Commands::Me => print_me(result),
        Commands::Ls { .. } => print_dialogs(result),
        Commands::Messages { .. } => print_messages(result),
        Commands::Search { .. } => print_messages(result),
        Commands::Send { .. } => print_send(result),
        Commands::Download { .. } => print_download(result),
        Commands::Forward { .. } | Commands::Delete { .. } | Commands::Read { .. } => {
            println!("✅  Done.");
        }
        Commands::Logout | Commands::Stop => {
            println!("✅  Done.");
        }
        Commands::Init | Commands::Login => {}
    }
}

fn print_status(result: &Value) {
    let auth = result["authorized"].as_bool().unwrap_or(false);
    let socket = result["socket"].as_str().unwrap_or("?");
    println!("Authorized : {}", if auth { "✅ yes" } else { "❌ no" });
    println!("Socket     : {socket}");
}

fn print_me(result: &Value) {
    let first = result["first_name"].as_str().unwrap_or("");
    let last = result["last_name"].as_str().unwrap_or("");
    let username = result["username"].as_str().unwrap_or("?");
    let phone = result["phone_number"].as_str().unwrap_or("?");
    let id = result["id"].as_i64().unwrap_or(0);
    println!("👤  {first} {last} (@{username})");
    println!("    ID: {id}  Phone: +{phone}");
}

fn print_dialogs(result: &Value) {
    // TDLib GetChats returns { chat_ids: [...] }
    let ids = match result.get("chat_ids").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => {
            println!("(no dialogs)");
            return;
        }
    };
    println!("📋  {} dialogs:\n", ids.len());
    for (i, id) in ids.iter().enumerate() {
        println!("  {:>3}. {}", i + 1, id);
    }
    println!("\n  (Use `tg messages <id>` to view messages)");
}

fn print_messages(result: &Value) {
    let messages = match result.get("messages").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => {
            println!("(no messages)");
            return;
        }
    };

    for msg in messages.iter().rev() {
        let id = msg["id"].as_i64().unwrap_or(0);
        let sender = msg["sender_id"]["user_id"]
            .as_i64()
            .map(|u| format!("user#{u}"))
            .unwrap_or_else(|| "system".to_string());
        let text = msg["content"]["text"]["text"]
            .as_str()
            .or_else(|| msg["content"]["caption"]["text"].as_str())
            .unwrap_or("[media]");
        let timestamp = msg["date"].as_i64().unwrap_or(0);
        let time = chrono_fmt(timestamp);
        println!("[{time}] {sender} #{id}");
        println!("  {text}");
        println!();
    }
}

fn print_send(result: &Value) {
    let id = result["id"].as_i64().unwrap_or(0);
    println!("✅  Sent (message #{id})");
}

fn print_download(result: &Value) {
    let path = result["local"]["path"].as_str().unwrap_or("?");
    println!("✅  Downloaded to {path}");
}

fn chrono_fmt(ts: i64) -> String {
    use std::time::{UNIX_EPOCH, Duration};
    let dt = UNIX_EPOCH + Duration::from_secs(ts as u64);
    let datetime: chrono::DateTime<chrono::Utc> = dt.into();
    datetime.format("%Y-%m-%d %H:%M").to_string()
}
