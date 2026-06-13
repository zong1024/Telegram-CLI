//! Pretty-print daemon responses.

use serde_json::Value;
use crate::Commands;

pub fn print_result(cmd: &Commands, result: &Value) {
    match cmd {
        Commands::Status => {
            println!("Socket: {}", result["socket"].as_str().unwrap_or("?"));
        }
        Commands::Me => {
            let f = result["first_name"].as_str().unwrap_or("");
            let l = result["last_name"].as_str().unwrap_or("");
            let u = result["username"].as_str().unwrap_or("?");
            let id = result["id"].as_i64().unwrap_or(0);
            println!("👤  {f} {l} (@{u})  id={id}");
        }
        Commands::Chats { .. } => print_dialogs(result),
        Commands::History { .. } | Commands::Search { .. } => print_messages(result),
        Commands::Send { .. } => {
            let id = result["id"].as_i64().unwrap_or(0);
            println!("✅  Sent (#{id})");
        }
        Commands::Download { .. } => {
            let path = result["local"]["path"].as_str().unwrap_or("?");
            println!("✅  Downloaded to {path}");
        }
        Commands::Forward { .. } | Commands::Delete { .. } | Commands::Read { .. }
        | Commands::Logout | Commands::Stop => println!("✅  Done."),
        Commands::Init | Commands::Login | Commands::Tui => {}
    }
}

fn print_dialogs(result: &Value) {
    if let Some(arr) = result.as_array() {
        if arr.is_empty() { println!("(no chats)"); return; }
        println!("📋  {} chats:\n", arr.len());
        for (i, item) in arr.iter().enumerate() {
            let title = item["title"].as_str().unwrap_or("?");
            let id = item["id"].as_i64().unwrap_or(0);
            let unread = item["unread_count"].as_i64().unwrap_or(0);
            let prefix = if unread > 0 { format!("({unread}) ") } else { String::new() };
            println!("  {:>3}. {prefix}{title}  [{id}]", i + 1);
        }
    } else if let Some(ids) = result.get("chat_ids").and_then(|v| v.as_array()) {
        println!("📋  {} chats:\n", ids.len());
        for (i, id) in ids.iter().enumerate() {
            println!("  {:>3}. {}", i + 1, id);
        }
    } else {
        println!("(no chats)");
    }
}

fn print_messages(result: &Value) {
    if let Some(arr) = result.as_array() {
        if arr.is_empty() { println!("(no messages)"); return; }
        for m in arr.iter() {
            let id = m["message_id"].as_i64().unwrap_or(0);
            let sender = m["sender_id"].as_i64()
                .or_else(|| m["sender_id"]["user_id"].as_i64())
                .map(|u| format!("user#{u}"))
                .unwrap_or_else(|| "system".into());
            let text = m["text"].as_str()
                .or_else(|| m["content"]["text"]["text"].as_str())
                .unwrap_or("[media]");
            let ts = m["date"].as_i64().unwrap_or(0);
            let time = fmt_time(ts);
            println!("[{time}] {sender} #{id}");
            println!("  {text}");
        }
    } else if let Some(msgs) = result.get("messages").and_then(|v| v.as_array()) {
        for m in msgs.iter().rev() {
            let id = m["id"].as_i64().unwrap_or(0);
            let sender = m["sender_id"]["user_id"].as_i64()
                .map(|u| format!("user#{u}"))
                .unwrap_or_else(|| "system".into());
            let text = m["content"]["text"]["text"].as_str().unwrap_or("[media]");
            let ts = m["date"].as_i64().unwrap_or(0);
            println!("[{}] {sender} #{id}", fmt_time(ts));
            println!("  {text}");
        }
    } else {
        println!("(no messages)");
    }
}

fn fmt_time(ts: i64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let dt = UNIX_EPOCH + Duration::from_secs(ts as u64);
    let dt: chrono::DateTime<chrono::Utc> = dt.into();
    dt.format("%Y-%m-%d %H:%M").to_string()
}
