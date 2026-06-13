//! SQLite cache for messages and dialogs.
//!
//! Provides fast local reads for the TUI and CLI without hitting TDLib.

use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tracing::info;

pub struct Cache {
    pool: SqlitePool,
}

impl Cache {
    /// Open (or create) the SQLite database and run migrations.
    pub async fn new(path: &std::path::Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let url = format!("sqlite:{}?mode=rwc", path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect(&url)
            .await?;

        sqlx::query(SCHEMA).execute(&pool).await?;
        info!("SQLite cache opened at {}", path.display());

        Ok(Self { pool })
    }

    /// Upsert a dialog (chat).
    pub async fn upsert_dialog(
        &self,
        chat_id: i64,
        title: &str,
        last_msg_id: Option<i64>,
        unread_count: i32,
        raw: &JsonValue,
    ) -> Result<()> {
        let raw_str = serde_json::to_string(raw)?;
        let now = chrono_now();
        sqlx::query(
            r#"
            INSERT INTO dialogs (chat_id, title, last_msg_id, unread_count, raw_json, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(chat_id) DO UPDATE SET
                title = excluded.title,
                last_msg_id = COALESCE(excluded.last_msg_id, dialogs.last_msg_id),
                unread_count = excluded.unread_count,
                raw_json = excluded.raw_json,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(chat_id)
        .bind(title)
        .bind(last_msg_id)
        .bind(unread_count)
        .bind(raw_str)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Upsert a message.
    pub async fn upsert_message(
        &self,
        msg_id: i64,
        chat_id: i64,
        sender_id: Option<i64>,
        text: Option<&str>,
        date: i64,
        raw: &JsonValue,
    ) -> Result<()> {
        let raw_str = serde_json::to_string(raw)?;
        sqlx::query(
            r#"
            INSERT INTO messages (id, chat_id, sender_id, text, date, raw_json)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(id, chat_id) DO UPDATE SET
                sender_id = excluded.sender_id,
                text = excluded.text,
                raw_json = excluded.raw_json
            "#,
        )
        .bind(msg_id)
        .bind(chat_id)
        .bind(sender_id)
        .bind(text)
        .bind(date)
        .bind(raw_str)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get recent messages for a chat.
    pub async fn get_messages(&self, chat_id: i64, limit: i64) -> Result<Vec<CachedMessage>> {
        let rows = sqlx::query_as::<_, CachedMessage>(
            "SELECT id, chat_id, sender_id, text, date FROM messages WHERE chat_id = ? ORDER BY date DESC LIMIT ?",
        )
        .bind(chat_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Search messages by text content.
    pub async fn search_messages(
        &self,
        chat_id: i64,
        query: &str,
        limit: i64,
    ) -> Result<Vec<CachedMessage>> {
        let pattern = format!("%{query}%");
        let rows = sqlx::query_as::<_, CachedMessage>(
            "SELECT id, chat_id, sender_id, text, date FROM messages WHERE chat_id = ? AND text LIKE ? ORDER BY date DESC LIMIT ?",
        )
        .bind(chat_id)
        .bind(pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get all dialogs ordered by most recently updated.
    pub async fn get_dialogs(&self, limit: i64) -> Result<Vec<CachedDialog>> {
        let rows = sqlx::query_as::<_, CachedDialog>(
            "SELECT chat_id, title, last_msg_id, unread_count FROM dialogs ORDER BY updated_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct CachedMessage {
    pub id: i64,
    pub chat_id: i64,
    pub sender_id: Option<i64>,
    pub text: Option<String>,
    pub date: i64,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct CachedDialog {
    pub chat_id: i64,
    pub title: String,
    pub last_msg_id: Option<i64>,
    pub unread_count: i32,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS messages (
    id        INTEGER NOT NULL,
    chat_id   INTEGER NOT NULL,
    sender_id INTEGER,
    text      TEXT,
    date      INTEGER NOT NULL,
    raw_json  TEXT,
    PRIMARY KEY (id, chat_id)
);
CREATE INDEX IF NOT EXISTS idx_messages_chat_date ON messages(chat_id, date DESC);

CREATE TABLE IF NOT EXISTS dialogs (
    chat_id      INTEGER PRIMARY KEY,
    title        TEXT NOT NULL DEFAULT '',
    last_msg_id  INTEGER,
    unread_count INTEGER NOT NULL DEFAULT 0,
    raw_json     TEXT,
    updated_at   INTEGER NOT NULL DEFAULT 0
);
"#;

/// Current Unix timestamp in seconds.
fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
