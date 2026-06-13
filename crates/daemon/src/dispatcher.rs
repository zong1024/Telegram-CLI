//! Event dispatcher — receives TDLib updates and updates the SQLite cache.

use serde_json::Value as JsonValue;
use tracing::{debug, warn};

use crate::cache::Cache;
use crate::tdlib::TdClient;

/// Run the cache updater: listen for TDLib events and upsert into SQLite.
pub async fn run_cache_updater(td: TdClient, cache: Cache) {
    let mut rx = td.subscribe();

    loop {
        match rx.recv().await {
            Ok(ev) => {
                if let Err(e) = process_event(&ev, &cache).await {
                    debug!("cache update error: {e}");
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                warn!("cache updater lagged by {n} events");
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                break;
            }
        }
    }
}

async fn process_event(ev: &JsonValue, cache: &Cache) -> anyhow::Result<()> {
    let ev_type = ev.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let data = ev.get("data");

    match ev_type {
        "new_message" => {
            if let Some(data) = data {
                let msg = &data["message"];
                let msg_id = msg["id"].as_i64().unwrap_or(0);
                let chat_id = msg["chat_id"].as_i64().unwrap_or(0);
                let sender_id = msg["sender_id"]["user_id"].as_i64();
                let text = msg["content"]["text"]["text"]
                    .as_str()
                    .or_else(|| msg["content"]["caption"]["text"].as_str());
                let date = msg["date"].as_i64().unwrap_or(0);

                if chat_id != 0 && msg_id != 0 {
                    cache
                        .upsert_message(msg_id, chat_id, sender_id, text, date, msg)
                        .await?;
                }
            }
        }
        _ => {}
    }

    Ok(())
}
