//! IPC client — connects to tgcd over a Unix socket.
//!
//! Uses `LengthDelimitedCodec` for framing.

use std::path::Path;

use anyhow::Result;
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value as JsonValue;
use tokio::net::UnixStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::codec::MAX_FRAME_LEN;
use crate::protocol::{Request, Response, RpcError, ServerMessage};

type IpcFramed = Framed<UnixStream, LengthDelimitedCodec>;

pub struct IpcClient {
    framed: IpcFramed,
}

impl IpcClient {
    pub async fn connect(socket_path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(socket_path).await?;
        let codec = LengthDelimitedCodec::builder()
            .max_frame_length(MAX_FRAME_LEN)
            .big_endian()
            .new_codec();
        Ok(Self {
            framed: Framed::new(stream, codec),
        })
    }

    /// Send a request and wait for the matching response (by UUID).
    pub async fn call(&mut self, method: &str, params: JsonValue) -> Result<JsonValue> {
        let id = uuid::Uuid::new_v4().to_string();
        let req = Request {
            id: id.clone(),
            method: method.to_string(),
            params,
        };
        self.send_request(&req).await?;

        loop {
            let msg = self.read_message().await?;
            match msg {
                ServerMessage::Response(resp) if resp.id == id => {
                    if let Some(err) = resp.error {
                        anyhow::bail!("RPC error {}: {}", err.code, err.message);
                    }
                    return Ok(resp.result.unwrap_or(JsonValue::Null));
                }
                _ => continue,
            }
        }
    }

    /// Send without waiting for a response.
    pub async fn send_request(&mut self, req: &Request) -> Result<()> {
        let payload = serde_json::to_vec(req)?;
        self.framed.send(Bytes::from(payload)).await?;
        Ok(())
    }

    /// Read the next server message.
    pub async fn read_message(&mut self) -> Result<ServerMessage> {
        let frame = self
            .framed
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("connection closed"))??;
        Ok(serde_json::from_slice(&frame)?)
    }
}
