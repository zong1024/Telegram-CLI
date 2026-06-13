//! Request handler — maps IPC requests to TDLib queries.

use anyhow::Result;
use serde_json::Value as JsonValue;
use tokio::sync::broadcast;

use tg_core::config::TgConfig;
use tg_ipc::protocol::{methods, Request, Response, RpcError};

use crate::cache::Cache;
use crate::tdlib;

pub struct AppState {
    pub config: TgConfig,
    pub td: tg_tdjson::TdClient,
    pub cache: Cache,
    pub updates_tx: broadcast::Sender<JsonValue>,
}

pub async fn handle_request(req: Request, state: &AppState) -> Response {
    let result = dispatch(&req.method, &req.params, state).await;
    match result {
        Ok(val) => Response {
            id: req.id,
            result: Some(val),
            error: None,
        },
        Err(e) => Response {
            id: req.id,
            result: None,
            error: Some(RpcError {
                code: -1,
                message: e.to_string(),
            }),
        },
    }
}

async fn dispatch(method: &str, params: &JsonValue, state: &AppState) -> Result<JsonValue> {
    match method {
        methods::GET_STATUS => {
            Ok(serde_json::json!({
                "socket": state.config.ipc.socket_path,
            }))
        }

        methods::GET_ME => {
            let resp = tdlib::query(&state.td, serde_json::json!({
                "@type": "getMe"
            })).await?;
            Ok(resp)
        }

        methods::LIST_DIALOGS => {
            let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(20);

            // Try cache first
            match state.cache.get_chats(limit).await {
                Ok(chats) if !chats.is_empty() => {
                    return Ok(serde_json::to_value(chats)?);
                }
                _ => {}
            }

            let resp = tdlib::query(&state.td, serde_json::json!({
                "@type": "getChats",
                "chat_list": {"@type": "chatListMain"},
                "limit": limit
            })).await?;
            Ok(resp)
        }

        methods::GET_MESSAGES => {
            let chat_id = params["chat_id"].as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing chat_id"))?;
            let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(50);

            // Try cache first
            match state.cache.get_messages(chat_id, limit).await {
                Ok(msgs) if !msgs.is_empty() => {
                    return Ok(serde_json::to_value(msgs)?);
                }
                _ => {}
            }

            let resp = tdlib::query(&state.td, serde_json::json!({
                "@type": "getChatHistory",
                "chat_id": chat_id,
                "from_message_id": 0,
                "offset": 0,
                "limit": limit,
                "only_local": false
            })).await?;
            Ok(resp)
        }

        methods::SEND_MESSAGE => {
            let chat_id = params["chat_id"].as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing chat_id"))?;
            let text = params["text"].as_str()
                .ok_or_else(|| anyhow::anyhow!("missing text"))?;

            let resp = tdlib::query(&state.td, serde_json::json!({
                "@type": "sendMessage",
                "chat_id": chat_id,
                "input_message_content": {
                    "@type": "inputMessageText",
                    "text": { "@type": "formattedText", "text": text }
                }
            })).await?;
            Ok(resp)
        }

        methods::SEARCH => {
            let chat_id = params["chat_id"].as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing chat_id"))?;
            let query = params["query"].as_str()
                .ok_or_else(|| anyhow::anyhow!("missing query"))?;
            let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(50);

            // Try local cache
            match state.cache.search_messages(chat_id, query, limit).await {
                Ok(msgs) if !msgs.is_empty() => {
                    return Ok(serde_json::to_value(msgs)?);
                }
                _ => {}
            }

            let resp = tdlib::query(&state.td, serde_json::json!({
                "@type": "searchChatMessages",
                "chat_id": chat_id,
                "query": query,
                "limit": limit
            })).await?;
            Ok(resp)
        }

        methods::FORWARD_MESSAGE => {
            let from = params["from_chat_id"].as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing from_chat_id"))?;
            let to = params["to_chat_id"].as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing to_chat_id"))?;
            let msg_id = params["message_id"].as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing message_id"))?;

            let resp = tdlib::query(&state.td, serde_json::json!({
                "@type": "forwardMessages",
                "chat_id": to,
                "from_chat_id": from,
                "message_ids": [msg_id]
            })).await?;
            Ok(resp)
        }

        methods::DELETE_MESSAGE => {
            let chat_id = params["chat_id"].as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing chat_id"))?;
            let msg_id = params["message_id"].as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing message_id"))?;

            let resp = tdlib::query(&state.td, serde_json::json!({
                "@type": "deleteMessages",
                "chat_id": chat_id,
                "message_ids": [msg_id],
                "revoke": true
            })).await?;
            Ok(resp)
        }

        methods::MARK_READ => {
            let chat_id = params["chat_id"].as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing chat_id"))?;

            tdlib::notify(&state.td, serde_json::json!({
                "@type": "viewMessages",
                "chat_id": chat_id,
                "message_ids": [],
                "force_read": true
            }));
            Ok(serde_json::json!({"status": "sent"}))
        }

        methods::DOWNLOAD_FILE => {
            let file_id = params["file_id"].as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing file_id"))?;

            let resp = tdlib::query(&state.td, serde_json::json!({
                "@type": "downloadFile",
                "file_id": file_id,
                "priority": 1,
                "synchronous": true
            })).await?;
            Ok(resp)
        }

        methods::LOGOUT => {
            tdlib::notify(&state.td, serde_json::json!({"@type": "logOut"}));
            Ok(serde_json::json!({"status": "sent"}))
        }

        methods::SHUTDOWN => {
            tracing::info!("Shutdown requested");
            std::process::exit(0);
        }

        // Auth methods
        methods::AUTH_PHONE => {
            let phone = params["phone"].as_str()
                .ok_or_else(|| anyhow::anyhow!("missing phone"))?;
            tdlib::notify(&state.td, serde_json::json!({
                "@type": "setAuthenticationPhoneNumber",
                "phone_number": phone,
                "settings": {
                    "@type": "phoneNumberAuthenticationSettings",
                    "is_current_phone_number": true
                }
            }));
            Ok(serde_json::json!({"status": "sent"}))
        }

        methods::AUTH_CODE => {
            let code = params["code"].as_str()
                .ok_or_else(|| anyhow::anyhow!("missing code"))?;
            tdlib::notify(&state.td, serde_json::json!({
                "@type": "checkAuthenticationCode",
                "code": code
            }));
            Ok(serde_json::json!({"status": "sent"}))
        }

        methods::AUTH_PASSWORD => {
            let pw = params["password"].as_str()
                .ok_or_else(|| anyhow::anyhow!("missing password"))?;
            tdlib::notify(&state.td, serde_json::json!({
                "@type": "checkAuthenticationPassword",
                "password": pw
            }));
            Ok(serde_json::json!({"status": "sent"}))
        }

        _ => Err(anyhow::anyhow!("unknown method: {}", method)),
    }
}
