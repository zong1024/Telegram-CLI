//! Unix socket server — accepts client connections, reads newline-delimited
//! JSON requests, and pushes events.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use serde_json::Value as JsonValue;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;
use tracing::{info, warn};

use tg_common::protocol::{Request, ServerMessage, Event};

use crate::handler::{self, AppState};

pub async fn run(socket_path: &Path, state: AppState) -> Result<()> {
    let listener = UnixListener::bind(socket_path)?;
    info!("Listening on {}", socket_path.display());

    let state = Arc::new(state);

    loop {
        let (stream, _addr) = listener.accept().await?;
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, state).await {
                warn!("client error: {e}");
            }
        });
    }
}

async fn handle_client(mut stream: UnixStream, state: Arc<AppState>) -> Result<()> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // Subscribe to events before splitting
    let mut event_rx = state.event_tx.subscribe();

    // We need to both read requests and push events.
    // Use two tasks: one reads requests, one forwards events.
    // For simplicity we use a single loop with tokio::select!

    // Create a channel to send responses back to the writer task
    let (resp_tx, mut resp_rx) = tokio::sync::mpsc::channel::<String>(64);

    // Spawn writer task: it writes both responses and events
    let write_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(resp) = resp_rx.recv() => {
                    if writer.write_all(resp.as_bytes()).await.is_err() { break; }
                    if writer.write_all(b"\n").await.is_err() { break; }
                }
                Ok(ev) = event_rx.recv() => {
                    let msg = serde_json::json!({
                        "type": "event",
                        "name": ev.get("name").and_then(|v| v.as_str()).unwrap_or("unknown"),
                        "data": ev,
                    });
                    let line = serde_json::to_string(&msg).unwrap_or_default();
                    if writer.write_all(line.as_bytes()).await.is_err() { break; }
                    if writer.write_all(b"\n").await.is_err() { break; }
                }
            }
        }
    });

    // Reader loop
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break; // client disconnected
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match serde_json::from_str::<Request>(trimmed) {
            Ok(req) => {
                let resp = handler::handle_request(req, &state).await;
                let json = serde_json::to_string(&ServerMessage::Response(resp))?;
                let _ = resp_tx.send(json).await;
            }
            Err(e) => {
                warn!("bad request: {e}");
                let err = serde_json::json!({
                    "type": "response",
                    "id": 0,
                    "error": { "code": -32700, "message": "Parse error" }
                });
                let _ = resp_tx.send(err.to_string()).await;
            }
        }
    }

    write_handle.abort();
    Ok(())
}
