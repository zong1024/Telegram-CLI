//! IPC client using `LengthDelimitedCodec` over Unix sockets.
//!
//! Wire format: 4-byte big-endian length prefix + JSON payload.
//! Used by `tg` (CLI) and `tg-tui` to communicate with `tgcd`.

use std::path::Path;

use anyhow::Result;
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value as JsonValue;
use tokio::net::UnixStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::protocol::{IPC_FRAME_MAX, Request, ServerMessage};

type IpcFramed = Framed<UnixStream, LengthDelimitedCodec>;

/// A framed IPC connection to the daemon.
pub struct IpcClient {
    framed: IpcFramed,
}

impl IpcClient {
    /// Connect to the daemon at the given socket path.
    pub async fn connect(socket_path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(socket_path).await?;
        let codec = LengthDelimitedCodec::builder()
            .max_frame_length(IPC_FRAME_MAX)
            .big_endian()
            .new_codec();
        let framed = Framed::new(stream, codec);
        Ok(Self { framed })
    }

    /// Send a request and wait for the matching response.
    pub async fn call(&mut self, method: &str, params: JsonValue) -> Result<JsonValue> {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let req = Request {
            id,
            method: method.to_string(),
            params,
        };
        self.send_request(&req).await?;

        // Read messages until we get the matching response
        loop {
            let msg = self.read_message().await?;
            match msg {
                ServerMessage::Response(resp) if resp.id == id => {
                    if let Some(err) = resp.error {
                        anyhow::bail!("RPC error {}: {}", err.code, err.message);
                    }
                    return Ok(resp.result.unwrap_or(JsonValue::Null));
                }
                _ => continue, // skip events and other responses
            }
        }
    }

    /// Send a request without waiting for a response.
    pub async fn send_request(&mut self, req: &Request) -> Result<()> {
        let payload = serde_json::to_vec(req)?;
        self.framed.send(Bytes::from(payload)).await?;
        Ok(())
    }

    /// Read the next server message (response or event).
    pub async fn read_message(&mut self) -> Result<ServerMessage> {
        let frame = self
            .framed
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("connection closed"))??;
        let msg: ServerMessage = serde_json::from_slice(&frame)?;
        Ok(msg)
    }

    /// Read the next server message as raw JSON.
    pub async fn read_raw(&mut self) -> Result<JsonValue> {
        let frame = self
            .framed
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("connection closed"))??;
        Ok(serde_json::from_slice(&frame)?)
    }
}
