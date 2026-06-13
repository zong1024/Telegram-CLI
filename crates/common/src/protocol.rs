use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// JSON-RPC-like protocol over Unix socket.
/// Messages are newline-delimited JSON.

// ── Client → Daemon ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: JsonValue,
}

// ── Daemon → Client ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "response")]
    Response(Response),

    #[serde(rename = "event")]
    Event(Event),

    #[serde(rename = "auth_state")]
    AuthState(AuthState),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub name: String,
    pub data: JsonValue,
}

/// Auth state pushed to clients so they can drive the login flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthState {
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

// ── Well-known method names ────────────────────────────────────────

pub mod methods {
    pub const GET_ME: &str = "get_me";
    pub const LIST_DIALOGS: &str = "list_dialogs";
    pub const GET_MESSAGES: &str = "get_messages";
    pub const SEND_MESSAGE: &str = "send_message";
    pub const FORWARD_MESSAGE: &str = "forward_message";
    pub const DELETE_MESSAGE: &str = "delete_message";
    pub const DOWNLOAD_FILE: &str = "download_file";
    pub const SEARCH: &str = "search";
    pub const MARK_READ: &str = "mark_read";
    pub const GET_STATUS: &str = "status";
    pub const LOGOUT: &str = "logout";
    pub const SHUTDOWN: &str = "shutdown";
}

// ── Well-known event names ─────────────────────────────────────────

pub mod events {
    pub const NEW_MESSAGE: &str = "new_message";
    pub const AUTH_UPDATE: &str = "auth_update";
}
