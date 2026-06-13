//! Request handler — maps incoming IPC requests to raw TDLib JSON queries.

use anyhow::Result;
use serde_json::Value as JsonValue;
use tokio::sync::broadcast;
use tracing::info;

use tg_common::config::TgConfig;
use tg_common::protocol::{methods, Request, Response, RpcError};

use crate::cache::Cache;
use crate::tdlib::TdClient;

pub struct AppState {
    pub config: TgConfig,
    pub td: TdClient,
    pub cache: Cache,
    pub event_tx: broadcast::Sender<JsonValue>,
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
                "authorized": state.td.is_authorized(),
                "socket": state.config.socket_path,
            }))
        }

        methods::GET_ME => {
            state
                .td
                .send(serde_json::json!({"@type": "getMe"}))
                .await;
            // Response arrives asynchronously; for now return acknowledgment
            Ok(serde_json::json!({"status": "sent", "method": "getMe"}))
        }

        methods::LIST_DIALOGS => {
            let limit = params
                .get("limit")
                .and_then(|v| v.as_i64())
                .unwrap_or(20);

            // Try cache first
            match state.cache.get_dialogs(limit).await {
                Ok(dialogs) if !dialogs.is_empty() => {
                    return Ok(serde_json::to_value(dialogs)?);
                }
                _ => {}
            }

            // Fall through to TDLib
            state
                .td
                .send(serde_json::json!({
                    "@type": "getChats",
                    "chat_list": {"@type": "chatListMain"},
                    "limit": limit
                }))
                .await;
            Ok(serde_json::json!({"status": "sent", "method": "getChats"}))
        }

        methods::GET_MESSAGES => {
            let chat_id = params["chat_id"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing chat_id"))?;
            let limit = params
                .get("limit")
                .and_then(|v| v.as_i64())
                .unwrap_or(20);

            // Try cache first
            match state.cache.get_messages(chat_id, limit).await {
                Ok(msgs) if !msgs.is_empty() => {
                    return Ok(serde_json::to_value(msgs)?);
                }
                _ => {}
            }

            state
                .td
                .send(serde_json::json!({
                    "@type": "getChatHistory",
                    "chat_id": chat_id,
                    "from_message_id": 0,
                    "offset": 0,
                    "limit": limit,
                    "only_local": false
                }))
                .await;
            Ok(serde_json::json!({"status": "sent", "method": "getChatHistory"}))
        }

        methods::SEND_MESSAGE => {
            let chat_id = params["chat_id"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing chat_id"))?;
            let text = params["text"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing text"))?;

            state
                .td
                .send(serde_json::json!({
                    "@type": "sendMessage",
                    "chat_id": chat_id,
                    "input_message_content": {
                        "@type": "inputMessageText",
                        "text": {
                            "@type": "formattedText",
                            "text": text
                        }
                    }
                }))
                .await;
            Ok(serde_json::json!({"status": "sent", "method": "sendMessage"}))
        }

        methods::SEARCH => {
            let chat_id = params["chat_id"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing chat_id"))?;
            let query = params["query"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing query"))?;
            let limit = params
                .get("limit")
                .and_then(|v| v.as_i64())
                .unwrap_or(20);

            // Try local cache search
            match state.cache.search_messages(chat_id, query, limit).await {
                Ok(msgs) if !msgs.is_empty() => {
                    return Ok(serde_json::to_value(msgs)?);
                }
                _ => {}
            }

            state
                .td
                .send(serde_json::json!({
                    "@type": "searchChatMessages",
                    "chat_id": chat_id,
                    "query": query,
                    "limit": limit
                }))
                .await;
            Ok(serde_json::json!({"status": "sent", "method": "searchChatMessages"}))
        }

        methods::FORWARD_MESSAGE => {
            let from_chat = params["from_chat_id"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing from_chat_id"))?;
            let to_chat = params["to_chat_id"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing to_chat_id"))?;
            let msg_id = params["message_id"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing message_id"))?;

            state
                .td
                .send(serde_json::json!({
                    "@type": "forwardMessages",
                    "chat_id": to_chat,
                    "from_chat_id": from_chat,
                    "message_ids": [msg_id]
                }))
                .await;
            Ok(serde_json::json!({"status": "sent"}))
        }

        methods::DELETE_MESSAGE => {
            let chat_id = params["chat_id"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing chat_id"))?;
            let msg_id = params["message_id"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing message_id"))?;

            state
                .td
                .send(serde_json::json!({
                    "@type": "deleteMessages",
                    "chat_id": chat_id,
                    "message_ids": [msg_id],
                    "revoke": true
                }))
                .await;
            Ok(serde_json::json!({"status": "sent"}))
        }

        methods::MARK_READ => {
            let chat_id = params["chat_id"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing chat_id"))?;

            state
                .td
                .send(serde_json::json!({
                    "@type": "viewMessages",
                    "chat_id": chat_id,
                    "message_ids": [],
                    "force_read": true
                }))
                .await;
            Ok(serde_json::json!({"status": "sent"}))
        }

        methods::DOWNLOAD_FILE => {
            let file_id = params["file_id"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing file_id"))?;

            state
                .td
                .send(serde_json::json!({
                    "@type": "downloadFile",
                    "file_id": file_id,
                    "priority": 1,
                    "synchronous": true
                }))
                .await;
            Ok(serde_json::json!({"status": "sent"}))
        }

        methods::LOGOUT => {
            state
                .td
                .send(serde_json::json!({"@type": "logOut"}))
                .await;
            Ok(serde_json::json!({"status": "sent"}))
        }

        methods::SHUTDOWN => {
            info!("Shutdown requested");
            std::process::exit(0);
        }

        // Auth interactive methods
        methods::AUTH_PHONE => {
            let phone = params["phone"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing phone"))?;
            state
                .td
                .send(serde_json::json!({
                    "@type": "setAuthenticationPhoneNumber",
                    "phone_number": phone,
                    "settings": {
                        "@type": "phoneNumberAuthenticationSettings",
                        "allow_flash_call": false,
                        "allow_missed_call": false,
                        "is_current_phone_number": true
                    }
                }))
                .await;
            Ok(serde_json::json!({"status": "sent"}))
        }

        methods::AUTH_CODE => {
            let code = params["code"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing code"))?;
            state
                .td
                .send(serde_json::json!({
                    "@type": "checkAuthenticationCode",
                    "code": code
                }))
                .await;
            Ok(serde_json::json!({"status": "sent"}))
        }

        methods::AUTH_PASSWORD => {
            let pw = params["password"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing password"))?;
            state
                .td
                .send(serde_json::json!({
                    "@type": "checkAuthenticationPassword",
                    "password": pw
                }))
                .await;
            Ok(serde_json::json!({"status": "sent"}))
        }

        _ => Err(anyhow::anyhow!("unknown method: {}", method)),
    }
}
