//! IPC server — accepts client connections using LengthDelimitedCodec.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use tokio::net::UnixListener;
use tokio_util::codec::LengthDelimitedCodec;
use tracing::{info, warn};

use tg_ipc::codec::MAX_FRAME_LEN;
use tg_ipc::protocol::{Request, ServerMessage};

use crate::handler::AppState;

pub async fn run(socket_path: &Path, state: AppState) -> Result<()> {
    let listener = UnixListener::bind(socket_path)?;

    // Restrict socket to owner only (0600)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600));
    }

    info!("IPC listening on {}", socket_path.display());

    let state = Arc::new(state);

    loop {
        let (stream, _) = listener.accept().await?;
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, state).await {
                warn!("client error: {e}");
            }
        });
    }
}

async fn handle_client(
    stream: tokio::net::UnixStream,
    state: Arc<AppState>,
) -> Result<()> {
    let codec = LengthDelimitedCodec::builder()
        .max_frame_length(MAX_FRAME_LEN)
        .big_endian()
        .new_codec();
    let framed = tokio_util::codec::Framed::new(stream, codec);
    let (mut writer, mut reader) = framed.split();

    let mut event_rx = state.updates_tx.subscribe();
    let (resp_tx, mut resp_rx) = tokio::sync::mpsc::channel::<Bytes>(64);

    // Writer task
    let write_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(resp) = resp_rx.recv() => {
                    if writer.send(resp).await.is_err() { break; }
                }
                Ok(ev) = event_rx.recv() => {
                    let ev_type = ev.get("@type").and_then(|v| v.as_str()).unwrap_or("");

                    // Convert auth state events to AuthState message
                    let msg = if ev_type == "updateAuthorizationState" {
                        let auth_state = &ev["authorization_state"];
                        let state_type = auth_state.get("@type").and_then(|v| v.as_str()).unwrap_or("");
                        auth_state_to_msg(state_type)
                    } else if ev_type.starts_with("authorizationState") {
                        // Direct response from getAuthorizationState
                        auth_state_to_msg(ev_type)
                    } else {
                        serde_json::json!({
                            "type": "event",
                            "name": ev_type,
                            "data": ev,
                        })
                    };

                    let payload = serde_json::to_vec(&msg).unwrap_or_default();
                    if writer.send(Bytes::from(payload)).await.is_err() { break; }
                }
            }
        }
    });

    // Reader loop
    while let Some(frame) = reader.next().await {
        let frame = match frame {
            Ok(f) => f,
            Err(e) => { warn!("frame error: {e}"); break; }
        };

        let req: Request = match serde_json::from_slice(&frame) {
            Ok(r) => r,
            Err(e) => {
                warn!("bad request: {e}");
                let err = serde_json::json!({
                    "type": "response", "id": "", "error": {"code": -32700, "message": "Parse error"}
                });
                let _ = resp_tx.send(Bytes::from(serde_json::to_vec(&err).unwrap())).await;
                continue;
            }
        };

        let resp = crate::handler::handle_request(req, &state).await;
        let msg = ServerMessage::Response(resp);
        let payload = serde_json::to_vec(&msg).unwrap_or_default();
        let _ = resp_tx.send(Bytes::from(payload)).await;
    }

    write_handle.abort();
    Ok(())
}

fn auth_state_to_msg(state_type: &str) -> serde_json::Value {
    let state_str = match state_type {
        "authorizationStateWaitPhoneNumber" => "wait_phone",
        "authorizationStateWaitCode" => "wait_code",
        "authorizationStateWaitPassword" => "wait_password",
        "authorizationStateReady" => "ready",
        "authorizationStateClosing" => "closing",
        "authorizationStateClosed" => "closed",
        "authorizationStateLoggingOut" => "logging_out",
        "authorizationStateWaitRegistration" => "wait_registration",
        "authorizationStateWaitOtherDeviceConfirmation" => "wait_other_device",
        _ => "unknown",
    };
    serde_json::json!({
        "type": "auth_state",
        "state": state_str,
    })
}
