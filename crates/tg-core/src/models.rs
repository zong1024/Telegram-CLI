//! Business models for the Telegram CLI.

use serde::{Deserialize, Serialize};

/// Preview of a chat for the sidebar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPreview {
    pub id: i64,
    pub title: String,
    pub kind: ChatKind,
    pub last_message_id: Option<i64>,
    pub unread_count: i32,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChatKind {
    Private,
    Group,
    Supergroup,
    Channel,
    Bot,
}

impl ChatKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Group => "group",
            Self::Supergroup => "supergroup",
            Self::Channel => "channel",
            Self::Bot => "bot",
        }
    }
}

/// A message view for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageView {
    pub chat_id: i64,
    pub message_id: i64,
    pub sender_id: Option<i64>,
    pub sender_name: String,
    pub text: String,
    pub date: i64,
    pub is_outgoing: bool,
    pub content_type: ContentType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContentType {
    Text,
    Photo,
    Video,
    Document,
    Sticker,
    Voice,
    Other,
}

impl ContentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Photo => "photo",
            Self::Video => "video",
            Self::Document => "document",
            Self::Sticker => "sticker",
            Self::Voice => "voice",
            Self::Other => "other",
        }
    }
}

/// Auth state machine for the login flow.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthState {
    WaitTdlibParameters,
    WaitEncryptionKey,
    WaitPhoneNumber,
    WaitCode,
    WaitPassword,
    Ready,
    LoggingOut,
    Closed,
    Unknown(String),
}

impl AuthState {
    pub fn from_tdlib(state_type: &str) -> Self {
        match state_type {
            "authorizationStateWaitTdlibParameters" => Self::WaitTdlibParameters,
            "authorizationStateWaitEncryptionKey" => Self::WaitEncryptionKey,
            "authorizationStateWaitPhoneNumber" => Self::WaitPhoneNumber,
            "authorizationStateWaitCode" => Self::WaitCode,
            "authorizationStateWaitPassword" => Self::WaitPassword,
            "authorizationStateReady" => Self::Ready,
            "authorizationStateLoggingOut" => Self::LoggingOut,
            "authorizationStateClosed" => Self::Closed,
            other => Self::Unknown(other.to_string()),
        }
    }
}

/// A download task tracked by the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    pub id: Option<i64>,
    pub chat_id: i64,
    pub message_id: i64,
    pub file_id: i64,
    pub local_path: Option<String>,
    pub status: DownloadStatus,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DownloadStatus {
    Pending,
    Downloading,
    Completed,
    Failed,
}

impl DownloadStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Downloading => "downloading",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}
