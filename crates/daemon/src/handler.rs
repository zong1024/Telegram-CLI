//! Request handler — maps incoming RPC requests to TDLib calls
//! and returns JSON responses.

use std::sync::Arc;
use tokio::sync::broadcast;
use serde_json::Value as JsonValue;
use anyhow::Result;
use tdlib::enums::Function;
use tdlib::functions as f;
use tracing::info;

use tg_common::config::TgConfig;
use tg_common::protocol::{methods, Request, Response, RpcError};

use crate::tdlib_client::TdClient;

pub struct AppState {
    pub config: TgConfig,
    pub td: TdClient,
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
            let resp = state
                .td
                .send_async(Function::GetMe(f::GetMe))
                .await;
            Ok(resp)
        }

        methods::LIST_DIALOGS => {
            let limit = params
                .get("limit")
                .and_then(|v| v.as_i64())
                .unwrap_or(20) as i32;
            let resp = state
                .td
                .send_async(Function::GetChats(f::GetChats {
                    chat_list: tdlib::enums::ChatList::Main,
                    limit,
                }))
                .await;
            Ok(resp)
        }

        methods::SEND_MESSAGE => {
            let chat_id = params["chat_id"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing chat_id"))?;
            let text = params["text"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing text"))?;
            let resp = state
                .td
                .send_async(Function::SendMessage(f::SendMessage {
                    chat_id,
                    message_thread_id: 0,
                    reply_to: None,
                    reply_markup: None,
                    options: None,
                    input_message_content:
                        tdlib::enums::InputMessageContent::InputMessageText(
                            tdlib::types::InputMessageText {
                                text: tdlib::types::FormattedText {
                                    text: text.to_string(),
                                    entities: Vec::new(),
                                },
                                clear_draft: false,
                                link_preview_options: None,
                            },
                        ),
                }))
                .await;
            Ok(resp)
        }

        methods::GET_MESSAGES => {
            let chat_id = params["chat_id"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing chat_id"))?;
            let limit = params
                .get("limit")
                .and_then(|v| v.as_i64())
                .unwrap_or(20) as i32;
            let resp = state
                .td
                .send_async(Function::GetChatHistory(f::GetChatHistory {
                    chat_id,
                    from_message_id: 0,
                    offset: 0,
                    limit,
                    only_local: false,
                }))
                .await;
            Ok(resp)
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
                .unwrap_or(20) as i32;
            let resp = state
                .td
                .send_async(Function::SearchChatMessages(f::SearchChatMessages {
                    chat_id,
                    query: query.to_string(),
                    sender_id: None,
                    from_message_id: 0,
                    offset: 0,
                    limit,
                    filter: tdlib::enums::SearchMessagesFilter::Empty,
                    message_thread_id: 0,
                    saved_messages_topic_id: 0,
                }))
                .await;
            Ok(resp)
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
            let resp = state
                .td
                .send_async(Function::ForwardMessages(f::ForwardMessages {
                    chat_id: to_chat,
                    message_thread_id: 0,
                    from_chat_id: from_chat,
                    message_ids: vec![msg_id],
                    send_copy: false,
                    remove_caption: false,
                }))
                .await;
            Ok(resp)
        }

        methods::DELETE_MESSAGE => {
            let chat_id = params["chat_id"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing chat_id"))?;
            let msg_id = params["message_id"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing message_id"))?;
            let resp = state
                .td
                .send_async(Function::DeleteMessages(f::DeleteMessages {
                    chat_id,
                    message_ids: vec![msg_id],
                    revoke: true,
                }))
                .await;
            Ok(resp)
        }

        methods::MARK_READ => {
            let chat_id = params["chat_id"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing chat_id"))?;
            let resp = state
                .td
                .send_async(Function::ViewMessages(f::ViewMessages {
                    chat_id,
                    message_ids: Vec::new(),
                    force_read: true,
                    source: None,
                }))
                .await;
            Ok(resp)
        }

        methods::DOWNLOAD_FILE => {
            let file_id = params["file_id"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("missing file_id"))? as i32;
            let resp = state
                .td
                .send_async(Function::DownloadFile(f::DownloadFile {
                    file_id,
                    priority: 1,
                    offset: 0,
                    limit: 0,
                    synchronous: true,
                }))
                .await;
            Ok(resp)
        }

        methods::LOGOUT => {
            let resp = state.td.send_async(Function::LogOut(f::LogOut)).await;
            Ok(resp)
        }

        methods::SHUTDOWN => {
            info!("Shutdown requested");
            std::process::exit(0);
        }

        _ => Err(anyhow::anyhow!("unknown method: {}", method)),
    }
}
